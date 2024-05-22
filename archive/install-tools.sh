#!/bin/bash
SCR_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "${SCR_PATH}" != "/root/tools" ]; then
  echo "ERROR: install-tools.sh must be located in /root/tools/ !"
  exit 1
fi

cd ${SCR_PATH}/gpsd
./install-gpsd.sh

cd ${SCR_PATH}/ntp
./install-ntp.sh

cd ${SCR_PATH}/gpsdo
./install-gpsdo.sh
