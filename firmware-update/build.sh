#!/bin/bash

cargo build --target armv7-unknown-linux-gnueabihf
/usr/arm-linux-gnueabihf/bin/strip target/armv7-unknown-linux-gnueabihf/debug/firmware-update -o ./firmware-update
sudo cp firmware-update firmware-update-filelist.txt firmware-update-rc.py ~/digris/PrecisionWave/disk-image/root-debian-testing/root/

if [[ "$1" == "deploy" ]]
then
    scp firmware-update-filelist.txt firmware-update firmware-update-rc.py dexter:
fi
