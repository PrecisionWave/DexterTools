#!/bin/bash
# sysmon readout for DEXTER
failed=0
vcc3v3=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in2_input)
vcc3v3=$(echo "$vcc3v3 *(18+36)/36/1000" | bc -l)
vcc5v4=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in1_input)
vcc5v4=$(echo "$vcc5v4 *(51+36)/36/1000" | bc -l)
vfan=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in3_input)
vfan=$(echo "$vfan *(560+22)/22/1000" | bc -l)
vccmainin=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in0_input)
vccmainin=$(echo "$vccmainin *(560+22)/22/1000" | bc -l)
vcc3v3pll=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in4_input)
vcc3v3pll=$(echo "$vcc3v3pll *(18+36)/36/1000" | bc -l)
vcc2v5io=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in5_input)
vcc2v5io=$(echo "$vcc2v5io *(4.7+36)/36/1000" | bc -l)
vccocxo=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/in6_input)
vccocxo=$(echo "$vccocxo *(51+36)/36/1000" | bc -l)
tbaseboard=$(cat /sys/bus/i2c/devices/1-002f/hwmon/hwmon0/temp1_input)
tbaseboard=$(echo "$tbaseboard/1000" | bc -l)
scale=$(cat /sys/bus/iio/devices/iio:device0/in_temp0_scale)
offset=$(cat /sys/bus/iio/devices/iio:device0/in_temp0_offset)
raw=$(cat /sys/bus/iio/devices/iio:device0/in_temp0_raw)
tfpga=$(echo "($raw + $offset) * $scale /1000" | bc -l)
printf "FPGA temperature = %.1f°C\n" $tfpga
printf "Baseboard temperature = %.1f°C\n" $tbaseboard
printf "VCC_MAIN_IN = %.2fV\n" $vccmainin
printf "VCC5V4 = %.2fV\n" $vcc5v4
printf "VCC3V3 = %.2fV\n" $vcc3v3
printf "VCC3V3PLL = %.2fV\n" $vcc3v3pll
printf "VCC_2V5_IO = %.2fV\n" $vcc2v5io
printf "V_FAN = %.2fV\n" $vfan
printf "VCC_OCXO = %.2fV\n" $vccocxo
VMINFACT=0.85
VMAXFACT=1.15
verror=0
if (( `echo $vccmainin'<='10 | bc`)); then
	verror=1
	printf "VCC_MAIN_IN FAILURE\n"
fi
if (( `echo $vcc5v4'<='5.4*$VMINFACT | bc`)) || (( `echo $vcc5v4'>='5.4*$VMAXFACT | bc`)); then
	verror=1
	printf "VCC5V4 FAILURE\n"
fi
if (( `echo $vcc3v3'<='3.3*$VMINFACT | bc`)) || (( `echo $vcc3v3'>='3.3*$VMAXFACT | bc`)); then
	verror=1
	printf "VCC3V3 FAILURE\n"
fi
if (( `echo $vcc3v3pll'<='3.3*$VMINFACT | bc`)) || (( `echo $vcc3v3pll'>='3.3*$VMAXFACT | bc`)); then
	verror=1
	printf "VCC_3V3_PLL FAILURE\n"
fi
if (( `echo $vcc2v5io'<='2.5*$VMINFACT | bc`)) || (( `echo $vcc2v5io'>='2.5*$VMAXFACT | bc`)); then
	verror=1
	printf "VCC_2V5_IO FAILURE\n"
fi
if [ "$verror" -eq "1" ]; then
	printf "Voltage Test: FAILED\n"
	failed=1
else
	printf "Voltage Test: OK\n"
fi

terror=0
if (( `echo $tfpga'>'85 | bc`)); then
	echo "FPGA Temperature too high"
	terror=1
fi
if (( `echo $tbaseboard'>'60 | bc`)); then
        echo "Baseboard Temperature too high"
	terror=1
fi
if [ "$terror" -eq "1" ]; then
        printf "Temperatures: ^TOO HIGH\n"
        failed=1
else
        printf "Temperatures: OK\n"
fi
if [[ $failed -ne 0 ]]; then
        exit 1
fi
exit 0
