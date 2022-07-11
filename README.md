# DexterTools
This repository contains routines for the installation and configuration of services that are needed by the Dexter Platform.
It is mandatory that this repository is cloned to the path '/root/tools/'.
```
git clone https://github.com/PrecisionWave/DexterTools.git /root/tools/
```

## gpsd
This service continuously receives the GPS NMEA strings from the GPS module and distributes them to all clients connected to the service. Further, the service receives and distributes the PPS pulses that are fed to the system over GPIO and delivered by the kernel module 'pps-gpio'. The NMEA strings are received from the serial device '/dev/ttyPS1' and the PPS signal is received from the device '/dev/pps0'.

The time stamps within the NMEA strings and the PPS pulses are shared to other applications by the use of shared memory sections. The NTP service can make use of this shared data to synchronize the internal clock to the highly precise GPS time source. This feature is the main use of this service on the Dexter platform.

The shared memory sections can be listed using the following command:
```
ipcs -m
```

## gpsdo
This service manages the tunable clock source 'Voltage Controlled Oven Controlled Crystal Oscillator' (VCOCXO) based on the highly precise PPS pulse from the GPS module.

It is basically a software 'phase locked loop' (PLL), which is continuously measuring the error of the VCOCXO in relation to the PPS signal, then converting the error signal to a regulation signal which is further being filtered before applying it to the ADC that controls the oscillator.

Once the clock matches the PPS signal accurately enough, gpsdo-locked is signaled to the Dexter DSP over iio.
Naturally, the GPS module should have reached fix state to enable proper operation of this service - this might take a while when booting the system. Further, it might take some time to reach gpsdo-locked state, as the PLL filter is quite narrow.

## ntp
This service synchronizes the system time.

It uses a mix of sources to optimize system time accuracy. If the Dexter Platform is connected to the internet, it might make use of online sources to get a coarse but quick synchronization. The information from the GPS module which is shared by the gpsd service is also being used for synchronization, thus no internet connection is really needed for time synchronization. The most precise source is the PPS signal which is delivered by the kernel module 'pps-gpio', however this source is always combined with one of the other sources mentioned, as the absolute time is missing in the PPS signal.

Accurate time synchronization needs some time to establish. To check the current status, try executing the following script:
```
/root/tools/ntp/testing/ntp-query.sh
```
