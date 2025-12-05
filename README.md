# GalahadII_LCD_Linux
A simple PyUSB script to stream a gif to the Lian-Li Galahad II LCD on Linux - because Linux deserves pretty things too.
This script was created and tested on CachyOS - with some help from a Windows VM, USBPcap, and DNSpyEx for reversing L-Connect.

## Usage

First identify/confirm that the GAII device is detected.
Make note of the ID value - 0416:7395 , I have set these as the script default but may require changing!

```
-$ lsusb | grep -i 'LianLi-GA'
Bus 001 Device 023: ID 0416:7395 Winbond Electronics Corp. LianLi-GA_II-LCD_v1.6
```

Next - modify the permissions of the USB device, this is optional **however you will need to run the python script with sudo if not done**

```
-$ sudo nano /etc/udev/rules.d/99-galahad.rules
```

Add the following, make sure to change your vendor and product id based on the earlier lsusb output

```
SUBSYSTEM=="usb", ATTR{idVendor}=="0416", ATTR{idProduct}=="7395", MODE="0666"
```

Then update the rules with udevadm

```
-$ sudo udevadm control --reload-rules
-$ sudo udevadm trigger
```

You'll likely need to pip install pyav

```
-$ sudo pacman -S python-av
OR
-$ pip install av --break-system-packages
```

Then run the script with your chosen gif, use -h to list args for rotating the GIF, changing VID/PID and frame interval speed.

```
-$ python3 galahadII_LCD.py -i frieren.gif
[+] Device Initialized
[!] Opening frieren.gif...
[!] Saving 40 frames to rotated.gif...
[+] Done!
[!] Converting rotated.gif -> video.h264...
[!] Detected FPS: 10
[+] Conversion Complete.
[+] Streaming 'rotated.gif'...
```
## Notes

If interrupted you only have a few seconds to restart the same or a new stream, else when running again the LCD will restart and kill the script.

I recommended that one gif be chosen at a time and this enabled as a service to run on a delay after startup.

```
-$ sudo nano /etc/systemd/system/lconlcd.service
```

```
[Unit]
Description=LianLiGalahadIILCD

[Service]
WorkingDirectory=/opt/GalahadII_LCD_Linux/
ExecStart=/usr/bin/python3 /opt/GalahadII_LCD_Linux/galahadII_LCD.py -i frieren.gif

[Install]
WantedBy=multi-user.target
```

Then perform a daemon-reload and enable the service

```
-$ sudo systemctl daemon-reload
-$ sudo systemctl enable --now lconlcd
```

This is far from perfect :) - the script basically just throws the H264 data at the LCD without the polling and other comms normally made by L-Connect.
Still - better than staring at the damn Lian-Li logo all day.
