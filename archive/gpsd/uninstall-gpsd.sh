apt remove gpsd

#rm /etc/systemd/system/gpsd.socket.d/socket.conf
#systemctl daemon-reload

mv backup/gpsd.socket /lib/systemd/system/
systemctl daemon-reload

mv backup/gpsd /etc/default/
