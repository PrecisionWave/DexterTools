#!/bin/bash
#
# Enable a static IP for eth0 (or another interface) on Ubuntu-based setups.
# Note that the wanted IP address should be specified as the first argument;
# otherwise, it defaults to 192.168.0.101. Also, the interface can be specified
# as the second argument if the default (eth0) isn't wanted.
#
# Example usage:
# enable_static_ip.sh [10.66.99.101] [eth1]
#
# WARNING: Do not use this script if there is a custom network configuration
# set up in /etc/network/interfaces as it will be overwritten.

set -e

IP_ADDR=${1:-192.168.1.111}
#ETH_DEV=${2:-eth0}
ETH_DEV=eth0
GW=${2:-192.168.1.1}
NM=${3:-255.255.255.0}
DN=${4:-192.168.1.1}

if [[ ${UID} -ne 0 ]]; then
        echo "This script must be run as root!"
        exit 1
fi

echo "Enabling the static IP address ${IP_ADDR} on ${ETH_DEV}"

# disable NetworkManager (assumes the config file hasn't been touched much)
sed -i 's/^managed=false/managed=true/' /etc/NetworkManager/NetworkManager.conf

# set up loopback and add static IP config for ${ETH_DEV} (defaults to eth0)
cat <<-EOF > /etc/network/interfaces
        auto lo
        iface lo inet loopback

        allow-hotplug ${ETH_DEV}
        iface ${ETH_DEV} inet static
        address ${IP_ADDR}
        netmask ${NM}
        gateway ${GW}
        dns-nameservers ${DN}
EOF

service network-manager restart
