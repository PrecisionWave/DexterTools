cp gpsdo.service /lib/systemd/system/

systemctl daemon-reload
systemctl enable gpsdo
systemctl start gpsdo
