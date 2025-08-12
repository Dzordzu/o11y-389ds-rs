dnf install haproxy vim -y
cp /vagrant/ldap-haproxy.cfg /etc/haproxy/conf.d/ldap.cfg
systemctl enable --now haproxy
systemctl restart haproxy
