#!/bin/bash

cargo build --target armv7-unknown-linux-gnueabihf
sudo cp target/armv7-unknown-linux-gnueabihf/debug/firmware-update ~/digris/PrecisionWave/disk-image/root-debian-testing/root/

if [[ "$1" == "deploy" ]]
then
    /usr/arm-linux-gnueabihf/bin/strip target/armv7-unknown-linux-gnueabihf/debug/firmware-update -o ./firmware-update
    scp firmware-update-filelist.txt firmware-update dexter:
fi
