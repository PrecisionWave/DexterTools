#!/bin/bash

ETH_DEV=eth0


# read current config but filter static settings for eth0
buf=()
tmp=()
eth0=false
while read -r line; do
  if ${eth0}; then
    if [ "${line}" = "" ]; then
      tmp+=("${line}")
      continue
    elif [ "${line%% *}" = "static" ]; then
      tmp=()
      continue
    else
      eth0=false
      buf=("${buf[@]}" "${tmp[@]}")
      tmp=()
    fi
  fi
  if [ "${line}" = "interface eth0" ]; then
    eth0=true
    tmp+=("${line}")
  else
    buf+=("${line}")
  fi
done < /etc/dhcpcd.conf


# write new config
(
  # write filtered version of current config
  for line in "${buf[@]}"; do
    echo "${line}"
  done
) > /etc/dhcpcd.conf

systemctl stop dhcpcd.service
ip addr flush dev ${ETH_DEV}
systemctl start dhcpcd.service

exit 0
