#!/bin/bash

if ! command -v cargo > /dev/null; then
  echo "[-] Cargo is required:"
  echo "Debian/Ubuntu: sudo apt install cargo"
  echo "Redhat/Fedora: sudo dnf install cargo"
  echo "Arch/Manjaro:  sudo pacman -S cargo"
  exit 1
fi

if [ -z "$1" ]; then
    echo -e "\033[0;31m[!] Error: You must provide a video file path.\033[0m"
    echo -e "Usage: sudo ./install.sh <path_to_video_or_gif>"
    exit 1
fi

INPUT_FILE=$(realpath "$1")

if [ ! -f "$INPUT_FILE" ]; then
    echo -e "\033[0;31m[!] Error: File '$INPUT_FILE' does not exist.\033[0m"
    exit 1
fi

APP_NAME="galahad2lcd"
INSTALL_DIR="/usr/local/bin"
CONFIG_FILE="/etc/default/${APP_NAME}"
SERVICE_FILE="/etc/systemd/system/${APP_NAME}.service"

DEFAULT_ARGS="--input ${INPUT_FILE} --rotate 0"

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}--- Starting Installation for ${APP_NAME} ---${NC}"

if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}[!] Please run as root (use sudo).${NC}"
  exit 1
fi

echo -e "${GREEN}[+] Building Release Binary...${NC}"
if cargo build --release; then
    echo -e "${GREEN}[+] Build successful.${NC}"
else
    echo -e "${RED}[!] Build failed. Exiting.${NC}"
    exit 1
fi

if systemctl is-active --quiet ${APP_NAME}; then
    echo -e "${GREEN}[+] Stopping existing service...${NC}"
    systemctl stop ${APP_NAME}
fi

echo -e "${GREEN}[+] Installing binary to ${INSTALL_DIR}...${NC}"
if [ -f "target/release/${APP_NAME}" ]; then
    cp "target/release/${APP_NAME}" "${INSTALL_DIR}/${APP_NAME}"
    chmod +x "${INSTALL_DIR}/${APP_NAME}"
else
    echo -e "${RED}[!] Could not find binary at target/release/${APP_NAME}.${NC}"
    echo -e "${RED}[!] Ensure your Cargo.toml 'name' field is '${APP_NAME}'.${NC}"
    exit 1
fi

echo -e "${GREEN}[+] Writing config file to ${CONFIG_FILE}...${NC}"
echo "MYAPP_ARGS=\"${DEFAULT_ARGS}\"" > "$CONFIG_FILE"
chmod 644 "$CONFIG_FILE"

echo -e "${GREEN}[+] Creating Systemd Service file with 3s delay...${NC}"
cat <<EOF > "${SERVICE_FILE}"
[Unit]
Description=${APP_NAME} Service
After=network.target

[Service]
Type=simple
EnvironmentFile=${CONFIG_FILE}
ExecStartPre=/bin/sleep 8
ExecStart=${INSTALL_DIR}/${APP_NAME} daemon \$MYAPP_ARGS
Restart=on-failure
User=root
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

echo -e "${GREEN}[+] Reloading daemon and enabling service...${NC}"
systemctl daemon-reload
systemctl enable ${APP_NAME}
systemctl start ${APP_NAME}

echo -e "${GREEN}--- Installation Complete ---${NC}"
echo -e "[+] Service is starting (with a 3s delay)..."
echo -e "[!] Video set to: ${INPUT_FILE}"
echo -e "${GREEN}[+] Galahad2LCD Installed! :D${NC}"
echo -e "${GREEN}To Update The Video/GIF: ${NC}"

galahad2lcd set-args -h
