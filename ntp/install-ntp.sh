# get ntp
apt install ntp

# stop ntp service
service ntp stop

if false; then
# get & build ntp with ATOM clk source support
#apt remove ntp
apt install libcap-dev
wget http://www.eecis.udel.edu/~ntp/ntp_spool/ntp4/ntp-4.2/ntp-4.2.8p15.tar.gz
tar xf ntp-4.2.8p15.tar.gz
rm ntp-4.2.8p15.tar.gz
cd ntp-4.2.8p15
./configure --enable-ATOM --enable-linuxcaps
make -j 2
#make install
# cp binaries
cd ..
fi

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
service ntp start
