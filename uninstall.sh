#!/bin/bash

APP_NAME="galahad2lcd"
INSTALL_DIR="/usr/local/bin"
CONFIG_FILE="/etc/default/${APP_NAME}"
SERVICE_FILE="/etc/systemd/system/${APP_NAME}.service"

if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}[!] Please run as root (use sudo).${NC}"
  exit 1
fi

systemctl disable ${APP_NAME}
systemctl stop ${APP_NAME}

rm $CONFIG_FILE
rm $SERVICE_FILE
rm "${INSTALL_DIR}/${APP_NAME}"

systemctl daemon-reload
