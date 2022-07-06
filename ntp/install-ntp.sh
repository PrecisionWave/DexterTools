#!/bin/bash

# get ppstest
apt -y install pps-tools

# install ntp deb packages (with KPPS support)
dpkg -i ntp-packages/*.deb

# put the package on hold, so that apt won't update the package anymore
# (packages from apt are usually not built with KPPS support)
echo "ntp hold" | dpkg --set-selections

# stop ntp service
systemctl stop ntp

#systemctl unmask ntp.service

mkdir -p backup

# replace dhclient.conf (removes 'sntp-servers' and 'ntp-servers')
if ! cmp --silent /etc/dhcp/dhclient.conf dhclient.conf; then
  mv /etc/dhcp/dhclient.conf backup/
  cp dhclient.conf /etc/dhcp/
fi

# remove dhcp hooks for ntp
if [ -f /etc/dhcp/dhclient-exit-hooks.d/ntp ]; then
  mv /etc/dhcp/dhclient-exit-hooks.d/ntp backup/
fi

# replace
if ! cmp --silent /etc/ntp.conf ntp.conf; then
  mv /etc/ntp.conf backup/
  cp ntp.conf /etc/
fi

# start ntp service
systemctl start ntp
