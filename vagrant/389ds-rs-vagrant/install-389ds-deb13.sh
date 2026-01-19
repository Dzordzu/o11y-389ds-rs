#!/bin/bash
apt update -y && apt upgrade -y
apt install 389-ds vim -y

IS_READY=$(dsctl -l | wc -l)

if [[ "$IS_READY" -lt 1 ]]; then
   dscreate from-file /vagrant/dscreate.ini
   systemctl enable --now dirsrv@default
fi
