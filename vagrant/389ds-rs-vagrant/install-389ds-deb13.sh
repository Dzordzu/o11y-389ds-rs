apt update -y && apt upgrade -y
apt install 389-ds vim -y

dscreate from-file /vagrant/dscreate.ini
systemctl enable --now dirsrv@default
