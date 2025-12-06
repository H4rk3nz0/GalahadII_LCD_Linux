import json
import os
from pathlib import Path

class GalahadConfig:
    # Device Info
    vendor_id = 0x0416
    product_id = 0x7395
    # USB
    interface_num = 1
    ep_out = 0x02
    # H264 Stream
    report_to_video = 0x02
    cmd_send_h264 = 0x0D
    # Struct sizes
    header_size = 11
    pkt_size_video = 512
    pkt_size_ctrl = 1024
    max_payload_video = 501

    app_directory_name = "galahadlcd"
    cfg_dir = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / app_directory_name
    cfg_file = cfg_dir / "config.json"
    pid_file_name = app_directory_name + ".pid"
    pid_file = cfg_dir / pid_file_name

    stream_script = "stream_gif.py"

    def __init__(self):
        # Files
        self.cfg_dir.mkdir(parents=True, exist_ok=True)

    def load(self):
        if not self.cfg_file.is_file():
            print("No valid config file")
            return
        with open(self.cfg_file, "r") as config:
            user_settings = json.load(config)
            self.current_video = user_settings["current_video"]
            self.product_id = user_settings["product_id"]
            self.vendor_id = user_settings["vendor_id"]
            print(f"Loaded settings from {self.cfg_file}")

    def write_config(self):
        with open(self.cfg_file, "w") as f:
            user_settings = {}
            user_settings["current_video"] = self.current_video
            user_settings["product_id"] = self.product_id
            user_settings["vendor_id"] = self.vendor_id
            json.dump(user_settings, f, indent=4)
            print(f"Settings written to {self.cfg_file}")

def main():
    config = GalahadConfig()
    print(config.current_video)

if __name__ == "__main__":
    main()
