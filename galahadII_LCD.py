import usb.core
import usb.util
import time
import struct
import os
import av
import argparse

from PIL import Image, ImageSequence

# --- Constants ---
INTERFACE_NUM = 1
EP_OUT = 0x02

# H264 Stream
REPORT_ID_VIDEO = 0x02
CMD_SEND_H264 = 0x0D

# Struct sizes
HEADER_SIZE = 11
PKT_SIZE_VIDEO = 512
PKT_SIZE_CTRL = 1024
MAX_PAYLOAD_VIDEO = 501

class GalahadII_Vision:
    def __init__(self, args):
        self.dev = usb.core.find(idVendor=args.vid, idProduct=args.pid)
        if self.dev is None:
            raise ValueError("Device not found.")

        if self.dev.is_kernel_driver_active(INTERFACE_NUM):
            try:
                self.dev.detach_kernel_driver(INTERFACE_NUM)
            except:
                pass
        try:
            usb.util.claim_interface(self.dev, INTERFACE_NUM)
        except:
            pass
        print(f"[+] Device Initialized")
        
    def send_h264_frame(self, frame_data):
        total_size = len(frame_data)
        bytes_sent = 0
        idx_val = 0

        while bytes_sent < total_size:
            remaining = total_size - bytes_sent
            chunk_len = min(remaining, MAX_PAYLOAD_VIDEO)
            
            header = struct.pack('>BB I 3s H', 
                                 REPORT_ID_VIDEO, 
                                 CMD_SEND_H264, 
                                 total_size, 
                                 idx_val.to_bytes(3, byteorder='big'), 
                                 chunk_len)
            
            packet = bytearray(PKT_SIZE_VIDEO)
            packet[0:HEADER_SIZE] = header
            packet[HEADER_SIZE : HEADER_SIZE + chunk_len] = frame_data[bytes_sent : bytes_sent + chunk_len]

            # Fire and forget. No reading crap back.
            self.dev.write(EP_OUT, packet)

            idx_val += 1
            bytes_sent += chunk_len


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


def rotate_gif(input_path, output_path, rotate_amount):
    
    rotate_amount = rotate_amount * -1

    print(f"[!] Opening {input_path}...")
    
    with Image.open(input_path) as im:
        frames = []
        
        for frame in ImageSequence.Iterator(im):

            rotated_frame = frame.copy().convert('RGBA').rotate(rotate_amount, expand=True)
            frames.append(rotated_frame)

        print(f"[!] Saving {len(frames)} frames to {output_path}...")

        frames[0].save(
            output_path,
            save_all=True,
            append_images=frames[1:],
            optimize=False,
            duration=im.info.get('duration', 100),
            loop=im.info.get('loop', 0),
            disposal=2
        )
        print("[+] Done!")

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
    
    stream = output_container.add_stream('libx264', rate=10) # fps=10
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
    scale = graph.add("scale", "480:480:flags=lanczos")
    fps = graph.add("fps", "fps=10")
    
    # IMPORTANT format filter to convert GIF (RGB/Paletted) to YUV420P
    # Corrected pixel format :D - hopefully
    fmt = graph.add("format", "yuv420p")
    
    sink = graph.add("buffersink")

    buffer.link_to(scale)
    scale.link_to(fps)
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


def main():
    parser = argparse.ArgumentParser(description ='Lian Li Galahad II LCD Video')
    parser.add_argument('-i','--input', 
        type=str, 
        help='Input gif to use',
        required=True
    )
    parser.add_argument('-r','--rotate', 
        type=int, 
        help='Rotate GIF, counter-clockwise default: 0/No rotation',
        default=0, 
        required=False
    )
    parser.add_argument('-v','--vid', 
        type=int, 
        help='Vendor ID, default: 0x0416',
        default=0x0416, 
        required=False
    )
    parser.add_argument('-p','--pid', 
        type=int, 
        help='Product ID, default: 0x7395',
        default=0x7395, 
        required=False
    )
    parser.add_argument('-s','--speed', 
        type=float, 
        help='Speed Adjustment via frame intervals, higher is slower: 0.5 - 2 | default: 1',
        default=1,
        required=False
    )
    args = parser.parse_args()

    device = GalahadII_Vision(args)

    if args.rotate != 0:
        rotate_gif(args.input, 'rotated.gif', args.rotate)
        args.input = 'rotated.gif'

    source_fps = convert_gif_to_h264(args.input, 'video.h264')

    try:
        print(f"[+] Streaming '{args.input}'...")
        frame_interval = args.speed / source_fps
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
        if os.path.isfile('rotated.gif'):
            os.remove('rotated.gif')
        if os.path.isfile('video.h264'):
            os.remove('video.h264')

    except Exception as e:
        print(f"\n[-] Error: {e}")
        if os.path.isfile('rotated.gif'):
            os.remove('rotated.gif')
        if os.path.isfile('video.h264'):
            os.remove('video.h264')

if __name__ == "__main__":
    main()
    
