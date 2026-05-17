GROUP_TO_CREATE="o11y-389ds-rs"

if [ -z "$(getent group | grep ${GROUP_TO_CREATE})" ]; then 
   groupadd -r "${GROUP_TO_CREATE}";
fi
