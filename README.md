# GalahadII LCD Linux
A simple PyUSB script to stream a gif to the Lian-Li Galahad II LCD on Linux - because Linux deserves pretty things too.
This script was created and tested on CachyOS (and Gentoo) - with some help from a Windows VM, USBPcap, and DNSpyEx for reversing L-Connect.  

**NOTE** If this script isn't working, I recommend enumerating the values for the `REPORT_ID_VIDEO` variable until you find a number that works.

## Fork Notes

Please check out the original repository: https://github.com/H4rk3nz0/GalahadII_LCD_Linux
H4rk3nzo did all the hard work, I just hacked together changes and updates to fit my needs.  
  
The original project wasn't working for my hardware revision (`LianLi-GA_II-LCD_v1.4`) so I updated the script to work on my stuff but eventually I ended up with somthing quite different.  

Here's some notable changes:
    - Updated the `REPORT_ID_VIDEO` variable from `0x03` to `0x02` to get the script working with my hardware
    - Added a virtual environment
    - Added a `requirements.txt` file
    - Maintained the aspect ratio of the gif's
    - Removed the rotate feature (I personally didn't need it but I probably should bring that back...)
    - Chopped up the scripts and reorganized the files based on my preferences
    - Renamed the script and removed the need for an input flag
    - Heavily edited the `README` file while also keeping a lot of it the same

## Setup

### USB Permissions  

First identify/confirm that the GAII device is detected.
Make note of the ID value - 0416:7395 , these are set as the script default but may require changing!

```
-$ lsusb | grep -i 'LianLi-GA'
Bus 001 Device 023: ID 0416:7395 Winbond Electronics Corp. LianLi-GA_II-LCD_v1.6
```

Next - Create a `udev` rule for the USB device and set the permissions, changing your vendor and product id based on the earlier `lsusb` output

```
-$ echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="0416", ATTR{idProduct}=="7395", MODE="0666"' > 99-galahad.rules
-$ sudo mv ./99-galahad.rules /etc/udev/rules.d/
```

Then update the rules with `udevadm`

```
-$ sudo udevadm control --reload-rules
-$ sudo udevadm trigger
```

### Environment  

Create a virtual environment for the gif streaming script to run in.
I set up the script rather sloppily and I'm not a Python developer so currently the script requires a virtual environment living within a `venv` folder.  

```
-$ python3 -m venv venv
-$ source venv/bin/activate
```
  
Install the dependencies:
```
-$ pip install -r requirements.txt
```
  
From here you can call `deactivate` and not worry about the virtual environment again.
  
Finally, ensure the `splashstream` script has execute permissions:
```
-$ chmod +x splashstream
```

## Usage

Run the script with your chosen gif, use -h to list args for stopping the GIF, and changing VID/PID.

```
-$ ./splashstream frieren.gif
Settings written to /home/topsoil/.config/splashstream/config.json
Previous video process killed
Writing pid file: 1459626
```

## Systemd Service

Create a new service file called `lconlcd.service` within `/etc/systemd/system/` with the following contents:

```
[Unit]
Description=LianLiGalahadIILCD

 # Update these directory as needed
[Service]
WorkingDirectory=/opt/GalahadII_LCD_Linux/
ExecStart=/usr/bin/python3 /opt/GalahadII_LCD_Linux/splashstream frieren.gif

[Install]
WantedBy=multi-user.target
```

Then perform a daemon-reload and enable the service

```
-$ sudo systemctl daemon-reload
-$ sudo systemctl enable --now lconlcd
```

## Notes

### Notes from the original author
If interrupted you only have a few seconds to restart the same or a new stream, else when running again the LCD will restart and kill the script.

I recommended that one gif be chosen at a time and this enabled as a service to run on a delay after startup.

This is far from perfect :) - the script basically just throws the H264 data at the LCD without the polling and other comms normally made by L-Connect.
Still - better than staring at the damn Lian-Li logo all day.

### My notes (tww0003)  
  
I haven't encountered the issues with interrupting the script. In my experience, the LCD screen will just freeze on the last frame displayed. I also didn't experience any issues swapping between gifs at any point like the original author mentions.
  
I only know enough Python to be dangerous and the way this script works should be proof of that.  
The `splashstream` script parses whatever arguments you pass in, writes a config file, then spans a subprocess that streams the gif to the lcd screen so that the command can exit and the gif can continue playing. The subprocess is spawned via `venv/bin/python` so it's important that the virtual environment is created for the script to work correctly. This is a bad design decision and I know that. The PID of that process is written to a file, which is later read and killed when either the `-s` flag is supplied or before a new gif is streamed.  
  
It would be nice to package this up so that it can be added to variable package managers, but I'm unsure if I'll continue to have the motivation to do so.