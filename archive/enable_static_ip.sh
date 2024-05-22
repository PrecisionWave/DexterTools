#!/bin/bash

IP_ADDR=${1:-192.168.1.111}
ETH_DEV=eth0
GW=${2:-192.168.1.1}
NM=${3:-255.255.255.0}
DN=${4:-192.168.1.1}


# netmask conversion
mask2cidr(){
  nbits=0
  IFS=.
  for dec in ${1} ; do
    case ${dec} in
      255) let nbits+=8;;
      254) let nbits+=7;;
      252) let nbits+=6;;
      248) let nbits+=5;;
      240) let nbits+=4;;
      224) let nbits+=3;;
      192) let nbits+=2;;
      128) let nbits+=1;;
      0);;
      *) echo "Error: ${dec} is not recognised"; exit 1
    esac
  done
  echo "${nbits}"
}


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

  # convert netmask format
  if [[ "${NM}" == *"."* ]]; then
    NM=$(mask2cidr ${NM})
  fi

  # add static settings for eth0
  echo "interface eth0"
  echo "static ip_address=${IP_ADDR}/${NM}"
  echo "static routers=${GW}"
  echo "static domain_name_servers=${DN}"
) > /etc/dhcpcd.conf

systemctl stop dhcpcd.service
ip addr flush dev ${ETH_DEV}
systemctl start dhcpcd.service

exit 0
