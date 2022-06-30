cp gpsdo.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable gpsdo
systemctl start gpsdo
