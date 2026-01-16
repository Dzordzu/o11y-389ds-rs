IS_UPGRADED="$1"
SYSTEMD_UNIT="haproxy-389ds-rs.service"

case "$IS_UPGRADED" in
  0) # This is a yum remove.
     if [ $(systemctl list-unit-files ${SYSTEMD_UNIT} &> /dev/null; echo $?) -eq 0 ]; then
         systemctl disable ${SYSTEMD_UNIT};
         systemctl stop ${SYSTEMD_UNIT};
     fi;

  ;;
  1) # This is a yum upgrade.
     systemctl is-active --quiet ${SYSTEMD_UNIT} && systemctl restart ${SYSTEMD_UNIT};
     exit 0;
  ;;
esac
