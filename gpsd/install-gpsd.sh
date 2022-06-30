apt install -y gpsd=3.22-4~bpo10+1

mkdir -p backup

#mkdir -p /etc/systemd/system/gpsd.socket.d
#cp socket.conf /etc/systemd/system/gpsd.socket.d/
#systemctl daemon-reload

mv /lib/systemd/system/gpsd.socket backup
cp gpsd.socket /lib/systemd/system/
systemctl daemon-reload

mv /etc/default/gpsd backup/
cp gpsd /etc/default/

service gpsd stop
service gpsd start
