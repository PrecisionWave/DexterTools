apt install -y gpsd=3.22-4~bpo10+1

mkdir -p backup

#mkdir -p /etc/systemd/system/gpsd.socket.d
#cp socket.conf /etc/systemd/system/gpsd.socket.d/
#systemctl daemon-reload

# replace socket file to enable connections to GPSD from other network nodes
if ! cmp --silent /lib/systemd/system/gpsd.socket gpsd.socket; then
  mv /lib/systemd/system/gpsd.socket backup
  cp gpsd.socket /lib/systemd/system/
  systemctl daemon-reload
fi

# replace GPSD config file (autoboot service, use kernel PPS device)
if ! cmp --silent /etc/default/gpsd gpsd; then
  mv /etc/default/gpsd backup/
  cp gpsd /etc/default/
fi

service gpsd stop
service gpsd start
