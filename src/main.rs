use anyhow::{Context, Result};
use byteorder::{BigEndian, WriteBytesExt};
use clap::{Parser, Subcommand};
use ffmpeg_next as ffmpeg;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::dictionary::Owned as Dictionary;
use image::{ImageBuffer, Rgba};
use rusb::{DeviceHandle, GlobalContext};
use std::fs;
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// Config Settings
const CONFIG_PATH: &str = "/etc/default/galahad2lcd";
const SERVICE_NAME: &str = "galahad2lcd";

// Hardware Constants
const VENDOR_ID: u16 = 0x0416;
const PRODUCT_ID: u16 = 0x7395;
const ENDPOINT_OUT: u8 = 0x02;
const INTERFACE_NUM: u8 = 1;
const SCREEN_WIDTH: u32 = 480;
const SCREEN_HEIGHT: u32 = 480;

// USB Protocol Constants
const REPORT_ID_VIDEO: u8 = 0x02;
const CMD_SEND_H264: u8 = 0x0D;
const MAX_PAYLOAD_VIDEO: usize = 501;
const PKT_SIZE_VIDEO: usize = 512;
const HEADER_SIZE: usize = 11;

// Clap Cli Options ---
#[derive(Parser)]
#[command(name = "galahad2lcd")]
#[command(about = "Driver for Lian Li Galahad II LCD", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Daemon {
        /// Path to the video/gif file
        #[arg(short, long)]
        input: String,

        /// Rotation in degrees (0, 90, 180, 270)
        #[arg(short, long, default_value_t = 0)]
        rotate: i32,
    },

    /// Updates the /etc/default/galahad2lcd config and restarts the service
    SetArgs {
        /// Path to the video/gif file
        #[arg(short, long)]
        input: String,

        /// Rotation in degrees (0, 90, 180, 270)
        #[arg(short, long, default_value_t = 0)]
        rotate: i32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon { input, rotate } => {
            run_daemon(input, rotate)
        }
        
        Commands::SetArgs { input, rotate } => {
            println!("[+] Updating configuration...");
            
            let abs_path = std::fs::canonicalize(&input)
                .with_context(|| format!("[-] Could not find file: {}", input))?;
            let abs_path_str = abs_path.to_string_lossy();

            // Format the content for /etc/default/galahad2lcd
            let config_content = format!(
                "MYAPP_ARGS=\"--input {} --rotate {}\"", 
                abs_path_str, rotate
            );

            if let Err(e) = fs::write(CONFIG_PATH, config_content) {
                eprintln!("[!] Failed to write config to {}: {}", CONFIG_PATH, e);
                eprintln!("[!] (Hint: Did you run with 'sudo'?)");
                std::process::exit(1);
            }
            println!("[+] Configuration saved to {}", CONFIG_PATH);

            println!("[+] Restarting {} service...", SERVICE_NAME);
            let status = Command::new("systemctl")
                .arg("restart")
                .arg(SERVICE_NAME)
                .status()
                .context("Failed to execute systemctl")?;

            if status.success() {
                println!("[+] Success! Service restarted with new settings.");
            } else {
                eprintln!("[!] Service restart failed.");
            }
            
            Ok(())
        }
    }
}

fn run_daemon(input_path: String, rotation: i32) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("[!] Stopping driver...");
    })?;

    ffmpeg::init()?;

    let cached_file = "/tmp/galahad_cache.h264";
    
    println!("[!] Transcoding input to H.264 (Rotation: {}°)...", rotation);
    let playback_fps = transcode_to_h264(&input_path, cached_file, rotation)?;
    println!("[+] Video FPS Detected: {:.2}", playback_fps);

    println!("[!] Pre-load H.264 packets into RAM...");
    let video_packets = preload_packets(cached_file)?;
    println!("[!] Buffered {} frames", video_packets.len());

    println!("[!] Connecting to Lian Li device...");
    let mut handle = open_device(VENDOR_ID, PRODUCT_ID)?;
    prepare_usb_device(&mut handle)?;

    stream_buffered_packets(&video_packets, &mut handle, running, playback_fps)?;

    Ok(())
}

fn preload_packets(path: &str) -> Result<Vec<Vec<u8>>> {
    let mut ictx = ffmpeg::format::input(&path)?;
    let input_stream = ictx.streams().best(ffmpeg::media::Type::Video)
        .ok_or(anyhow::anyhow!("[-] No video stream found in file"))?;
    let stream_index = input_stream.index();

    let mut buffered_packets = Vec::new();

    for (stream, packet) in ictx.packets() {
        if stream.index() == stream_index {
            if let Some(data) = packet.data() {
                buffered_packets.push(data.to_vec());
            }
        }
    }

    Ok(buffered_packets)
}

fn stream_buffered_packets(
    packets: &[Vec<u8>],
    handle: &mut DeviceHandle<GlobalContext>,
    running: Arc<AtomicBool>,
    fps: f64,
) -> Result<()> {
    let safe_fps = if fps <= 0.0 || fps > 120.0 { 30.0 } else { fps };
    let target_frame_time = Duration::from_secs_f64(1.0 / safe_fps);
    
    println!("[+] Streaming from RAM at {:.2} FPS (Interval: {:?})", safe_fps, target_frame_time);

    while running.load(Ordering::SeqCst) {
        for frame_data in packets {
            if !running.load(Ordering::SeqCst) { break; }

            let start = std::time::Instant::now();

            if let Err(e) = send_packet_to_usb(handle, frame_data) {
                eprintln!("[-] USB Error: {:?}", e);
            }

            let elapsed = start.elapsed();
            if target_frame_time > elapsed {
                std::thread::sleep(target_frame_time - elapsed);
            }
        }
    }
    Ok(())
}

fn transcode_to_h264(input_path: &str, output_path: &str, rotation: i32) -> Result<f64> {
    let mut ictx = ffmpeg::format::input(&input_path)?;
    let input_stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(anyhow::anyhow!("No video stream found"))?;
    
    let video_stream_index = input_stream.index();

    let fps_rational = input_stream.avg_frame_rate();
    let fps = if fps_rational.denominator() == 0 {
        let r_fps = input_stream.rate();
        if r_fps.denominator() == 0 { 30.0 } else { r_fps.numerator() as f64 / r_fps.denominator() as f64 }
    } else {
        fps_rational.numerator() as f64 / fps_rational.denominator() as f64
    };

    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let mut octx = ffmpeg::format::output(&output_path)?;
    let codec = ffmpeg::encoder::find_by_name("libx264")
        .ok_or(anyhow::anyhow!("libx264 not found"))?;
    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec);
    let mut encoder = encoder_ctx.encoder().video()?;

    encoder.set_height(SCREEN_HEIGHT);
    encoder.set_width(SCREEN_WIDTH);
    encoder.set_format(Pixel::YUV420P);
    encoder.set_bit_rate(2_000_000);
    encoder.set_time_base((1, 1000));
    encoder.set_max_b_frames(0);
    
    let gop_size = fps.round() as u32;
    encoder.set_gop(gop_size);

    let mut opts = Dictionary::new();
    opts.set("preset", "veryfast");
    opts.set("profile", "baseline");
    opts.set("x264-params", &format!(
        "nal-hrd=cbr:vbv-maxrate=2000:vbv-bufsize=2000:annexb=1:open-gop=0:scenecut=0:keyint={}:min-keyint={}",
        gop_size, gop_size
    ));

    let mut encoder = encoder.open_as_with(codec, opts)?;
    let mut ost = octx.add_stream(codec)?;
    ost.set_parameters(&encoder);
    octx.write_header()?;

    let mut decoded_frame = ffmpeg::util::frame::Video::empty();
    let mut encoded_packet = ffmpeg::Packet::empty();
    
    let mut pts_counter = 0;
    let frame_delay_units = (1000.0 / fps) as i64; 

    let mut to_rgba_scaler: Option<Scaler> = None;
    let mut to_yuv_scaler: Option<Scaler> = None;

    for (stream, pkt) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&pkt)?;
            
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                if to_rgba_scaler.is_none() || 
                   to_rgba_scaler.as_ref().unwrap().input().width != decoded_frame.width() ||
                   to_rgba_scaler.as_ref().unwrap().input().height != decoded_frame.height() 
                {
                    to_rgba_scaler = Some(Scaler::get(
                        decoded_frame.format(),
                        decoded_frame.width(),
                        decoded_frame.height(),
                        Pixel::RGBA,
                        decoded_frame.width(),
                        decoded_frame.height(),
                        Flags::BILINEAR,
                    )?);
                }

                let mut rgba_frame = ffmpeg::util::frame::Video::empty();
                to_rgba_scaler.as_mut().unwrap().run(&decoded_frame, &mut rgba_frame)?;

                let raw_data = rgba_frame.data(0);
                let stride = rgba_frame.stride(0);
                let width = rgba_frame.width();
                let height = rgba_frame.height();
                
                let mut tight_buffer = Vec::with_capacity((width * height * 4) as usize);
                for y in 0..height as usize {
                    let start = y * stride;
                    let end = start + (width as usize * 4);
                    tight_buffer.extend_from_slice(&raw_data[start..end]);
                }

                let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = 
                    ImageBuffer::from_raw(width, height, tight_buffer)
                    .ok_or(anyhow::anyhow!("[-] Failed to create image buffer"))?;

                let rotated_buffer = if rotation == 90 {
                    image::imageops::rotate90(&img_buffer)
                } else if rotation == -90 || rotation == 270 {
                    image::imageops::rotate270(&img_buffer)
                } else if rotation == 180 {
                    image::imageops::rotate180(&img_buffer)
                } else {
                    img_buffer
                };

                let (rot_w, rot_h) = (rotated_buffer.width(), rotated_buffer.height());

                if to_yuv_scaler.is_none() || 
                   to_yuv_scaler.as_ref().unwrap().input().width != rot_w ||
                   to_yuv_scaler.as_ref().unwrap().input().height != rot_h 
                {
                     to_yuv_scaler = Some(Scaler::get(
                        Pixel::RGBA,
                        rot_w,
                        rot_h,
                        Pixel::YUV420P,
                        SCREEN_WIDTH,
                        SCREEN_HEIGHT,
                        Flags::BILINEAR,
                    )?);
                }

                let mut input_frame_rotated = ffmpeg::util::frame::Video::new(Pixel::RGBA, rot_w, rot_h);
                let dest_stride = input_frame_rotated.stride(0);
                let dest_data = input_frame_rotated.data_mut(0);
                let src_data = rotated_buffer.as_raw();
                let src_stride = (rot_w * 4) as usize;

                for y in 0..rot_h as usize {
                    let src_start = y * src_stride;
                    let src_end = src_start + src_stride;
                    let dest_start = y * dest_stride;
                    dest_data[dest_start..dest_start+src_stride].copy_from_slice(&src_data[src_start..src_end]);
                }

                let mut final_frame = ffmpeg::util::frame::Video::empty();
                to_yuv_scaler.as_mut().unwrap().run(&input_frame_rotated, &mut final_frame)?;

                final_frame.set_pts(Some(pts_counter));
                pts_counter += frame_delay_units;

                encoder.send_frame(&final_frame)?;
                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    encoded_packet.set_stream(0);
                    encoded_packet.write_interleaved(&mut octx)?;
                }
            }
        }
    }

    encoder.send_eof()?;
    while encoder.receive_packet(&mut encoded_packet).is_ok() {
        encoded_packet.set_stream(0);
        encoded_packet.write_interleaved(&mut octx)?;
    }

    octx.write_trailer()?;

    Ok(fps)
}

fn send_packet_to_usb(handle: &mut DeviceHandle<GlobalContext>, frame_data: &[u8]) -> Result<()> {
    let total_size = frame_data.len();
    let mut bytes_sent = 0;
    let mut idx_val: u32 = 0;

    while bytes_sent < total_size {
        let remaining = total_size - bytes_sent;
        let chunk_len = std::cmp::min(remaining, MAX_PAYLOAD_VIDEO);

        let mut header = Vec::with_capacity(HEADER_SIZE);
        header.push(REPORT_ID_VIDEO);
        header.push(CMD_SEND_H264);
        header.write_u32::<BigEndian>(total_size as u32)?;

        let idx_bytes = idx_val.to_be_bytes();
        header.write_all(&idx_bytes[1..4])?;

        header.write_u16::<BigEndian>(chunk_len as u16)?;

        let mut usb_packet = vec![0u8; PKT_SIZE_VIDEO];
        usb_packet[0..HEADER_SIZE].copy_from_slice(&header);
        usb_packet[HEADER_SIZE..HEADER_SIZE + chunk_len]
            .copy_from_slice(&frame_data[bytes_sent..bytes_sent + chunk_len]);

        let _ = handle
            .write_bulk(ENDPOINT_OUT, &usb_packet, Duration::from_millis(1000))
            .ok();

        idx_val = idx_val.wrapping_add(1);
        bytes_sent += chunk_len;
    }
    Ok(())
}

fn prepare_usb_device(handle: &mut DeviceHandle<GlobalContext>) -> Result<()> {
    if let Ok(active) = handle.kernel_driver_active(INTERFACE_NUM) {
        if active {
            let _ = handle.detach_kernel_driver(INTERFACE_NUM);
        }
    }
    handle.claim_interface(INTERFACE_NUM)?;
    Ok(())
}

fn open_device(vid: u16, pid: u16) -> Result<DeviceHandle<GlobalContext>> {
    for device in rusb::devices()?.iter() {
        let descriptor = device.device_descriptor()?;
        if descriptor.vendor_id() == vid && descriptor.product_id() == pid {
            return device.open().context("[-] Device not found");
        }
    }
    Err(anyhow::anyhow!("[-] Device not found"))
}