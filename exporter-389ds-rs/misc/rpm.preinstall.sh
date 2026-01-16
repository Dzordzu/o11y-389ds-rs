ACCOUNT_TO_CREATE="exporter-38d9s-rs"
GROUP_TO_CREATE="o11y-389ds-rs"

if [ -z "$(getent passwd | grep ${ACCOUNT_TO_CREATE})" ]; then 
   useradd -r ${ACCOUNT_TO_CREATE};
fi

if [ -z "$(getent group | grep ${GROUP_TO_CREATE})" ]; then 
   groupadd -r "${GROUP_TO_CREATE}";
fi

usermod -aG "${GROUP_TO_CREATE}" "${ACCOUNT_TO_CREATE}"
