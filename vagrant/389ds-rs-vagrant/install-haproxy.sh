#!/bin/bash
dnf install haproxy vim policycoreutils-python-utils -y
cp /vagrant/ldap-haproxy.cfg /etc/haproxy/haproxy.cfg
setsebool -P haproxy_connect_any 1
systemctl enable --now haproxy
systemctl restart haproxy
