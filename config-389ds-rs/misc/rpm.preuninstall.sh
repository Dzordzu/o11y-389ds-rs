IS_UPGRADED="$1"
SYSTEMD_UNIT="exporter-389ds-rs.service"

case "$IS_UPGRADED" in
  0) # This is a yum remove.
     cp /etc/o11y-389ds-rs/default.toml /etc/o11y-389ds-rs/default.toml.$(date '+%Y-%m-%d').rpmsave
  ;;
  1) # This is a yum upgrade.
     exit 0;
  ;;
esac
