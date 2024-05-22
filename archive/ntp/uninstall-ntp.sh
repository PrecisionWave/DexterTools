apt remove ntp
#cd ntp-4.2.8p15
#make uninstall
#cd ..

# restore dhclient.conf
mv backup/dhclient.conf /etc/dhcp/

# restore dhcp hooks for ntp
mv backup/ntp /etc/dhcp/dhclient-exit-hooks.d/

# restore ntp.conf
mv backup/ntp.conf /etc/
