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
mv /etc/dhcp/dhclient.conf backup/
cp dhclient.conf /etc/dhcp/

# remove dhcp hooks for ntp
mv /etc/dhcp/dhclient-exit-hooks.d/ntp backup/

# replace
mv /etc/ntp.conf backup/
cp ntp.conf /etc/

# start ntp service
systemctl start ntp
