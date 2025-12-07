import usb.core
import usb.util
import time
import struct
import os
import av

from galahad_config import GalahadConfig

class GalahadII_Vision:
    def __init__(self, config: GalahadConfig):
        self.dev = usb.core.find(idVendor=config.vendor_id, idProduct=config.product_id)
        self.config = config
        if self.dev is None:
            raise ValueError("Device not found.")

        if self.dev.is_kernel_driver_active(config.interface_num):
            try:
                self.dev.detach_kernel_driver(config.interface_num)
            except:
                pass
        try:
            usb.util.claim_interface(self.dev, config.interface_num)
        except:
            pass
        print(f"[+] Device Initialized")
        
    def send_h264_frame(self, frame_data):
        total_size = len(frame_data)
        bytes_sent = 0
        idx_val = 0

        while bytes_sent < total_size:
            remaining = total_size - bytes_sent
            chunk_len = min(remaining, self.config.max_payload_video)
            
            header = struct.pack('>BB I 3s H', 
                                 self.config.report_to_video, 
                                 self.config.cmd_send_h264, 
                                 total_size, 
                                 idx_val.to_bytes(3, byteorder='big'), 
                                 chunk_len)
            
            packet = bytearray(self.config.pkt_size_video)
            packet[0:self.config.header_size] = header
            packet[self.config.header_size : self.config.header_size + chunk_len] = frame_data[bytes_sent : bytes_sent + chunk_len]

            # Fire and forget. No reading crap back.
            self.dev.write(self.config.ep_out, packet)

            idx_val += 1
            bytes_sent += chunk_len

def convert_gif_to_h264(input_path, output_path):
    print(f"[!] Converting {input_path} -> {output_path}...")
    
    input_container = av.open(input_path)
    input_stream = input_container.streams.video[0]

    source_fps = input_stream.average_rate
    print(f'[!] Detected FPS: {source_fps}')
    if source_fps is None or float(source_fps) == 0:
        print("[!] Warning: Source FPS likely wrong/not detected. Defaulting to 24")
        source_fps = 24

    output_container = av.open(output_path, mode='w', format='h264')
    
    stream = output_container.add_stream('libx264', rate=10)
    stream.width = 480
    stream.height = 480
    stream.pix_fmt = 'yuv420p'
    
    stream.bit_rate = 2000000      # 2 Mbps (Arbitrary safe value for valid CBR)

    stream.options = {
        'profile': 'baseline',
        'preset': 'veryfast',
        'x264-params': 'keyint=30:min-keyint=30:nal-hrd=cbr' 
    }

    graph = av.filter.Graph()

    buffer = graph.add_buffer(template=input_stream)

    scale = graph.add("scale", "w=480:h=480:force_original_aspect_ratio=decrease")
    pad   = graph.add("pad",   "w=480:h=480:x=(ow-iw)/2:y=(oh-ih)/2:color=black")
    fps = graph.add("fps", "fps=10")
    
    # IMPORTANT format filter to convert GIF (RGB/Paletted) to YUV420P
    # Corrected pixel format :D - hopefully
    fmt = graph.add("format", "yuv420p")
    
    sink = graph.add("buffersink")

    buffer.link_to(scale)
    scale.link_to(pad)
    pad.link_to(fps)
    fps.link_to(fmt)
    fmt.link_to(sink)
    
    graph.configure()

    # Process frames 
    for frame in input_container.decode(input_stream):
        graph.push(frame)

        try:
            while True:
                filtered_frame = graph.pull()
                for packet in stream.encode(filtered_frame):
                    output_container.mux(packet)
        except (av.error.BlockingIOError, av.error.EOFError):
            continue

    # Flush
    graph.push(None)

    try:
        while True:
            filtered_frame = graph.pull()
            for packet in stream.encode(filtered_frame):
                output_container.mux(packet)
    except (av.error.BlockingIOError, av.error.EOFError):
        pass

    # Flush
    for packet in stream.encode():
        output_container.mux(packet)

    input_container.close()
    output_container.close()
    print("[+] Conversion Complete.")
    return source_fps

def yield_ffmpeg_packets(filepath):
    """
    Parses video to raw H.264
    """
    container = av.open(filepath)
    stream = container.streams.video[0]
    
    # Handle PyAV version differences for BitStreamFilter
    try:
        # Newer PyAV
        bsf = av.BitStreamFilterContext('h264_mp4toannexb', stream)
    except AttributeError:
        try:
            # Standard PyAV
            bsf = av.BitStreamFilter('h264_mp4toannexb', stream)
        except AttributeError:
            # Older/Alternative PyAV
            from av.bitstream import BitStreamFilter
            bsf = BitStreamFilter('h264_mp4toannexb', stream)

    for packet in container.demux(stream):
        for res_packet in bsf.filter(packet):
            yield bytes(res_packet)

def main():
    config = GalahadConfig()
    config.load()
    device = GalahadII_Vision(config=config)
    source_fps = convert_gif_to_h264(config.current_video, 'video.h264')

    try:
        print(f"[+] Streaming '{config.current_video}'...")
        frame_interval = 1 / source_fps
        while True:
            for frame_bytes in yield_ffmpeg_packets('video.h264'):

                start = time.time()

                device.send_h264_frame(frame_bytes)

                elapsed = time.time() - start
                sleep_time = frame_interval - elapsed
                if sleep_time > 0:
                    time.sleep(sleep_time)

    except KeyboardInterrupt:
        print("\n[!] Stopped.")
        if os.path.isfile('video.h264'):
            os.remove('video.h264')

    except Exception as e:
        print(f"\n[-] Error: {e}")
        if os.path.isfile('video.h264'):
            os.remove('video.h264')

if __name__ == "__main__":
    main()
