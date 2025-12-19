# GalahadII_LCD_Linux
A Rust compiled service for streaming an MP4/GIF to the Lian-Li Galahad II LCD on Unix systems - Linux deserves pretty things too.
This script was created and tested on CachyOS - with some help from a Windows VM, USBPcap, and DNSpyEx for reversing L-Connect.

This is a rewrite of the previous Python project to act as a more elegant solution.

## Prerequisites

There are several libraries that may be required depending on your Linux Distro of choice. I have tried to compile some common ones below.
I can't cater to all distros here sadly, but I am sure you can get it working on Red Star OS - I believe in you.

### Ubuntu

The rustc version at time of writing necessitates rustup to be installed instead of cargo

```
sudo apt install -y rustup pkg-config libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev clang build-essential && \
    rustup default stable 
```

### Debian

```
sudo apt install -y cargo pkg-config libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev
```

### Fedora

```
sudo dnf install cargo libavutil-free-devel libavformat-free-devel libavfilter-free-devel libavdevice-free-devel
```

### Arch

```
sudo pacman -S cargo pkgconf ffmpeg clang
```

## Building

First identify & confirm that the GAII device is detected.
Make note of the Product and Vendor ID values, e.g: 0416:7395
I have set these as the script default **but may require changing in src/main.rs** !

```
❯ lsusb | grep -i 'LianLi-GA'

Bus 001 Device 023: ID 0416:7395 Winbond Electronics Corp. LianLi-GA_II-LCD_v1.6
```

Ensure you have cargo installed, then run the install script with your chosen gif.

```
❯ sudo bash install.sh /home/harkenzo/Pictures/frieren.gif

--- Starting Installation for galahad2lcd ---
[+] Building Release Binary...
---SNIP---
[+] Build successful.
[+] Installing binary to /usr/local/bin...
[+] Writing config file to /etc/default/galahad2lcd
[+] Creating Systemd Service file with 3s delay
[+] Reloading daemon and enabling service...
Created symlink '/etc/systemd/system/multi-user.target.wants/galahad2lcd.service' → '/etc/systemd/system/galahad2lcd.service'.
--- Installation Complete ---
[+] Service is starting (with a 3s delay)...
[!] Video set to: /home/harkenzo/Pictures/frieren.gif
[+] Galahad2LCD Installed! :D
```

To Update The Video/GIF: 

```
Usage: sudo galahad2lcd set-args [OPTIONS] --input <INPUT> -r 0

  -i, --input <INPUT>    Path to the video/gif file
  -r, --rotate <ROTATE>  Rotation in degrees (0, 90, 180, 270) [default: 0]
  -h, --help             Print help
```

This is far from perfect :) - the service basically just throws the H264 data at the LCD without the polling and other comms normally made by L-Connect.
Still - better than staring at the damn Lian-Li logo all day.
