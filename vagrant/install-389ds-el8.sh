dnf install epel-release -y
dnf config-manager --set-enabled powertools
dnf copr enable @389ds/389-directory-server -y
dnf install 389-ds-base cockpit-389-ds vim -y

dscreate from-file /vagrant/dscreate.ini
systemctl enable --now dirsrv@default
