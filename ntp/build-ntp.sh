#!/bin/bash

# first uncomment the deb-src line in sources.list
buf=()
ppa_updated=false
deb_src=false
while read -r line; do
  if [ "${line:0:8}" = "#deb-src" ]; then
    buf+=("${line:1}")
    ppa_updated=true
    deb_src=true
  elif [ "${line:0:7}" = "deb-src" ]; then
    deb_src=true
  else
    buf+=("${line}")
  fi
done < /etc/apt/sources.list

if ${ppa_updated}; then
  (
    for line in "${buf[@]}"; do
      echo "${line}"
    done
  ) > /etc/apt/sources.list
elif ! ${deb_src}; then
  echo "ERROR: no deb-src found in /etc/apt/sources.list !"
  echo "> Please add source manually .."
  exit 1
fi

# get latest package lists
apt update

# get all dependencies for building ntp
apt -y build-dep ntp

# get tools for building deb packages
apt -y install ubuntu-dev-tools

# get source of ntp
mkdir ntp-src
cd ntp-src
apt source ntp

# build ntp deb packages
cd ntp-*
dpkg-buildpackage -uc -us -nc -j $(nproc)
cd ..
mv *.deb ../ntp-packages/

echo "\nntp build DONE! Please run install-ntp.sh now .."
