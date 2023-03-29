#!/bin/bash

# GPS disciplined OCXO controller for PrecisionWave Dexter
#
# 22/06/01 A. Zutter
#
# see application note AN-1002 from Analog Devices
# https://www.analog.com/media/en/technical-documentation/application-notes/AN-1002.pdf

# ADC clock frequency 163.84MHz
# Ref Clk Oscillators dexter onboard: VCOCXO Connor Winfield DOCAT020V-030.72M; Highperformance VCOCXO Taitien NI-10M-2450

verbose=${1:-0} # 1: enables console output, 0: silent mode
production_test=${2:-0} # 1: enables faster tuning for vbi baseboard production tests, 0: standard mode

gps_pps_accuracy_ns=11 # GPS PPS pulse accuracy 11ns on RXM RM GPS module

# check and terminate if there is already another GPSDO instance is running
for pid in $(pidof -x gpsdo.sh); do
    if [ $pid != $$ ]; then
        echo "[$(date)] : gpsdo.sh : Process is already running with PID $pid"
        exit 1
    fi
done

# search all devices needed for the GPSDO
dsp=0
while true; do
        if test -f /sys/bus/iio/devices/iio:device$dsp/name; then
                myvalue=$(cat /sys/bus/iio/devices/iio:device$dsp/name)
        else
                myvalue=0
        fi
        if [ $myvalue == 'dexter_dsp_tx' ] || [ $dsp -eq 100 ]
        then
                break
        fi
        ((dsp++))
	if (( $dsp == 100 )); then
	        echo "ERROR: DSP core not found"
        	exit 1
	fi
done
if (( verbose != 0 )); then
	echo "DSP core IIO device found on nr: $dsp"
fi

dac_ocxo=0
while true; do
	if test -f /sys/bus/iio/devices/iio:device$dac_ocxo/name; then
	        myvalue=$(cat /sys/bus/iio/devices/iio:device$dac_ocxo/name)
	else
		myvalue=0
	fi
        if [ $myvalue == 'ltc2606' ] || (( $dac_ocxo == 20 )) # 16Bit DAC
        then
                break
        fi
        ((dac_ocxo++))
done
if (( verbose != 0 )); then
	if (( $dac_ocxo == 20 )); then
                echo "OCXO tuning DAC IIO device not found"
	else
	        echo "OCXO tuning DAC IIO device found on nr: $dac_ocxo"
	fi
fi

lmk04805=0
while true; do
        if test -f /sys/bus/iio/devices/iio:device$lmk04805/name; then
                myvalue=$(cat /sys/bus/iio/devices/iio:device$lmk04805/name)
        else
                myvalue=0
        fi
        if [ $myvalue == 'lmk04805' ]
        then
                break
        fi
        ((lmk04805++))
	if (( $lmk04805 == 100 )); then
        	echo "ERROR: LMK04805 IIO device not found"
	        exit 1
	fi
done
if (( verbose != 0 )); then
        echo "LMK04805 IIO device found on nr: $lmk04805"
fi

# init variables
nomfreq_old=0

while true; do
	nomfreq=$(cat /sys/bus/iio/devices/iio:device$lmk04805/out_altvoltage7_DAC_CLK_frequency)
	vco_mode=$(cat /sys/bus/iio/devices/iio:device$lmk04805/VCO_MODE)
	nomfreq=$(($nomfreq / 6))
	if (( $nomfreq_old != $nomfreq )); then
		# load last saved calibration value if exist
		if test -f "ocxo.cal"; then
			dac_start_value=$(cat ocxo.cal)
		else
			dac_start_value=32767 	# might be overwritten by last saved value in ocxo.cal file
		fi
	        max_dac_value=65535 	# 2^number of DAC bits
		sync_threshold=0.1 	# resync point Hz
		unsync_threshold=2 	# must be >sync_threshold
	        f_mean_len=10
	        t_mean_len=10
		f_gain=0.03
		t_gain=0.1
		fine_gain=0.2
		if (( $production_test == 1 )); then
       	                sync_threshold=1      # resync point Hz
                        unsync_threshold=15     # 5 must be >sync_threshold
               	        f_gain=0.03; #0.01
       	                t_gain=0.15
                        fine_gain=1; #0.2
			dac_start_value=2048
		fi

		# load coefficients depending on min or max version VCOCXO type
		if (( $vco_mode == 6 )); then # Min version, PLL1 disabled, PLL2 has VCOCXO as reference
		        K_ocxo=9260   		# DOCAT020V-030.72M tune sensitivity ppb/Volt
			max_dac_voltage=3.3 	# DAC reference voltage
		else
			K_ocxo=250		# NI-10M-2450, 250ppb/V
			max_dac_voltage=4 	# DAC reference voltage
		fi

		# init variables
		pps_cnt=0
		pps_direction=0 	# 0: SMA PPS connector is output, 1: input
		pd=0 			# digital phase detector GPS 1pps pulse to in FPGA regenerated 1pps pulse with nomfreq sampling clock
		pd_samples_old=0
		cte1=0			# cumulated time error
		dac_value_old=0

		f_mean_done=0		# frequency mean
		f_mean_sum=0
		f_mean_i=1

		t_mean_done=0		# time error mean
		t_mean_sum=0
		t_mean_i=1

		search_state=0		# count cycles (0.5s) when pps counter not changes, toggle from internal to external pps after three cycles
		pps_lost=0		# when 1, no PPS pulses are detected anymore and it starts searching either from internal GPS PPS or 
					# external PPS input pulses are available
		skip_next=0

		coarse_resync_cnt=0
		store_i=0
		store_period=600	# number of 0.5s cycles a storage of the DAC value occurs

		resyncs=0
		sync_state=0

		min=0			# time error minimum
		max=0			# time error maximum

		# Calculate constants
		K_dac=$(echo "$max_dac_voltage / $max_dac_value" |bc -l) 	# DAC gain V/bit
		K_Hz=$(echo "$K_dac * $nomfreq * $K_ocxo / 1000000000" |bc -l) 	# OCXO+DAC tuning bit/Hz
		K_s=$(echo "$K_ocxo / 1000000000 * $K_dac" |bc -l) 		# OCXO+DAC tuning sec/bit
		K_ns=$(echo "$K_s * 1000000000" |bc -l)
		T=$(echo "1 / $nomfreq" |bc -l) 				# phase detector sampling time
		T_ns=$(echo "1000000000 / $nomfreq" |bc -l)
		sync_state=0

		if (( verbose != 0 )); then
			printf "Nom Freq=%d, T=%.2f ns, DAC start value = %d, K_ocxo = %d ppb/V, K_dac = %.6f V/bit, K total = %.6f Hz/bit = %.2f ns/bit\n" $nomfreq $T_ns $dac_start_value $K_ocxo $K_dac $K_Hz $K_ns
		fi

		# init DAC with center voltage
		cfe=$dac_start_value
		echo $dac_start_value > /sys/bus/iio/devices/iio:device$dac_ocxo/out_voltage0_raw
		sleep 1

		echo $nomfreq >/sys/bus/iio/devices/iio:device$dsp/pps_reference_frequency
		echo $(($nomfreq+1)) >/sys/bus/iio/devices/iio:device$dsp/pps_reference_frequency     # reset reference frequency
		# set reference frequency value, a change on this register generates a resync of the error counter
		sleep 1
	fi
	nomfreq_old=$nomfreq

	# if no pps pulse found in last cycle, try with other direction
	pps_cnt_old=$pps_cnt
	pps_cnt=$(cat /sys/bus/iio/devices/iio:device$dsp/pps_cnt)
	if (( $pps_cnt_old == $pps_cnt )); then
		if (( $search_state == 3 )); then
			if (( $pps_direction == 0 )); then
				pps_direction=1 	# SMA PPS connector is input
				if (( verbose != 0 )); then
				        echo "SMA PPS connector direction set to input"
				fi
			else
				pps_direction=0 	# SMA PPS connector is output
                        	if (( verbose != 0 )); then
                                	echo "SMA PPS connector direction set to output"
	                        fi
			fi
			echo $(($nomfreq+1)) >/sys/bus/iio/devices/iio:device$dsp/pps_reference_frequency 	# reset reference frequency
			# set reference frequency value, a change on this register generates a resync of the error counter
			# set PPS connector direction
			echo $pps_direction >/sys/bus/iio/devices/iio:device$dsp/pps_direction_out_n_in
			echo 1 >/sys/bus/iio/devices/iio:device$dsp/pps_loss_of_signal
			search_state=0
			pps_lost=1
			sleep 0.5
		else
			search_state=$(($search_state+1))
		fi
	elif (( $pps_lost == 1 )); then
		search_state=0
		pps_lost=0
	elif (( $pps_lost == 0 )); then
		echo $nomfreq >/sys/bus/iio/devices/iio:device$dsp/pps_reference_frequency
		echo 0 >/sys/bus/iio/devices/iio:device$dsp/pps_loss_of_signal
		search_state=0
		pps_lost=0
		# pps pulse found in last cycle, run PLL correction
		pd_samples=$(cat /sys/bus/iio/devices/iio:device$dsp/pps_clk_error) 	# read frequency error
		if (( $pd_samples >= $(( $nomfreq / 2 )) )); then # pd readout is unsigned, convert to signed
			pd_samples=$(( $pd_samples - $nomfreq ))
		fi
		freq_error=$(( pd_samples - pd_samples_old )) # ADC clock frequency error in Hz
		pd_samples_old=$pd_samples
		if (( $skip_next > 0 )) || (( $freq_error > 10000 )) || (( $freq_error < -10000 )); then
			if (( $skip_next > 0 )); then
	                        skip_next=$((skip_next - 1))
			else
				coarse_resync_cnt=$((coarse_resync_cnt + 1))
				if (( $coarse_resync_cnt > 4 )); then
					nomfreq_old=0
					coarse_resync_cnt=0
				fi
			fi
			freq_error=0
			pd_samples=0
			pd_samples_old=0
                else
			coarse_resync_cnt=0
		fi

		# correct frequency
		f_mean_array[f_mean_i]=$freq_error
		f_mean_i_old=$f_mean_i
		f_mean_i=$(($f_mean_i+1))
		if (( $f_mean_i > $f_mean_len )); then
			f_mean_i=1
			f_mean_done=1
		fi
		f_array_element=${f_mean_array[f_mean_i]}
		if (( $f_mean_done == 0 )); then
			f_mean_sum=$(echo "$f_mean_sum + $freq_error" |bc -l)
			f_mean=$(echo "$f_mean_sum / $f_mean_i_old" |bc -l)
		else
			f_mean_sum=$(echo "$f_mean_sum + $freq_error - $f_array_element" |bc -l)
			f_mean=$(echo "$f_mean_sum / ($f_mean_len - 1)" |bc -l)
		fi
		f_tune=$(echo "-1 * $f_mean * $f_gain / $K_Hz" |bc -l)

		# correct phase, calculate cumulated time error cte
		abs_f_mean=$f_mean
		if (( $(echo "$abs_f_mean < 0" |bc -l) )); then
			abs_f_mean=$(echo "-1 * $abs_f_mean" |bc -l)
		fi
		pd=$(echo "$pd_samples * $T" |bc -l)

		# if the frequency error rise above the unsync threshold, the GPSDO will go to the unsynced state
		# and do a coarse frequency only correction.
		# if the frequency error falls below the sync threshold, the GPSDO will start correct the PPS time error
		if (( $(echo "$abs_f_mean > $unsync_threshold" |bc -l) )); then
			sync_state=0 # not synced
		elif (( $(echo "$abs_f_mean < $sync_threshold" |bc -l) )) && (( $pd_samples != 0 )) && (( $sync_state == 0 )); then
			sync_state=1 # start resync
		fi

		if (( $sync_state == 1 )); then # do resyc
			sync_state=2
			skip_next=2
			echo $(($nomfreq+1)) >/sys/bus/iio/devices/iio:device$dsp/pps_reference_frequency       # reset reference frequency
			if (( verbose != 0 )); then
				echo "resync!"
			fi
			resyncs=$((resyncs+1))
		elif (( $sync_state == 2 )); then # synced state, adjust PPS time error and do not correct frequency error
			f_tune=0
	                t_mean_array[t_mean_i]=$pd
        	        t_mean_i_old=$t_mean_i
                	t_mean_i=$(($t_mean_i+1))
	                if (( $t_mean_i > $t_mean_len )); then
        	                t_mean_i=1
                	        t_mean_done=1
	                fi
        	        t_array_element=${t_mean_array[t_mean_i]}
	                if (( $t_mean_done == 0 )); then
	                	t_mean_sum=$(echo "$t_mean_sum + $pd" |bc -l)
        	                t_mean=$(echo "$t_mean_sum / $t_mean_i_old" |bc -l)
                	else
	                	t_mean_sum=$(echo "$t_mean_sum + $pd - $t_array_element" |bc -l)
                        	t_mean=$(echo "$t_mean_sum / ($t_mean_len - 1)" |bc -l)
	                fi
			if (( $(echo "$pd < 0" |bc -l) )); then
				cte1=$(echo "$cte1 + $fine_gain" |bc -l)
			elif (( $(echo "$pd > 0" |bc -l) )); then
				cte1=$(echo "$cte1 - $fine_gain" |bc -l)
			fi
			cte=$(echo "$cte1 - $t_mean * $t_gain / $K_s" |bc -l)
			echo 1 >/sys/bus/iio/devices/iio:device$dsp/gpsdo_locked
		else # unsynced sync_state==0, just correct frequency error and not PPS time error
			t_mean_done=0
			t_mean_sum=0
			t_mean_i=1
			t_mean=0
			cte1=0
			cte=0
			echo 0 >/sys/bus/iio/devices/iio:device$dsp/gpsdo_locked
		fi

		# calculate cumulated frequency error cfe
		cfe=$(echo "$cfe + $f_tune" |bc -l)
		if (( $(echo "$cfe > $max_dac_value" |bc -l) )); then
			cfe=$max_dac_value
		elif (( $(echo "$cfe < 0" |bc -l) )); then
			cfe=0
		fi

		# add phase and frequency correction to get the DAC tune value
		tune_val=$(echo "$cfe + $cte" |bc -l)
		if (( $(echo "$tune_val > $max_dac_value" |bc -l) )); then
			tune_val=$max_dac_value
		elif (( $(echo "$tune_val < 0" |bc -l) )); then
			tune_val=0
		fi
		dac_value=$(printf "%.*f" 0 $tune_val) # round to integer

		t_ns=$(echo "1000000000 * $pd" |bc -l) # regenerated PPS time error in ns
		t_ns_int=$(printf "%.*f" 0 $t_ns) # round to integer
		echo $t_ns_int >/sys/bus/iio/devices/iio:device$dsp/pps_clk_error_ns
		echo $freq_error >/sys/bus/iio/devices/iio:device$dsp/pps_clk_error_hz

		# send it to DAC
		if (( $dac_value_old != $dac_value )); then
			dac_value_old=$dac_value
			echo $dac_value > /sys/bus/iio/devices/iio:device$dac_ocxo/out_voltage0_raw
		fi

		# store value to file
		if (( $store_i == $store_period )); then
			if (( $dac_start_value != $dac_value )); then
				echo $dac_value >ocxo.cal
			fi
			store_i=0
		fi
		store_i=$(($store_i+1))

		# print values
		if (( verbose != 0 )); then
			t_mean_ns=$(echo "1000000000 * $t_mean" |bc -l)
			f_mean_ppb=$(echo "1000000000 * $f_mean / $nomfreq" |bc -l)
			if (( $(echo "$t_mean_ns < $min" |bc -l) )); then
				min=$t_mean_ns
			elif (( $(echo "$t_mean_ns > $max" |bc -l) )); then
				max=$t_mean_ns
			fi
                        printf "f err = %d Hz, f mean err = %.3fHz = %.3fppb, t err = %.0fns, t mean err = %.3fns min %.1f max %.1f, f tune = %.3f, cte = %.3f, DAC value = %d, resyncs = %d\n" $freq_error $f_mean $f_mean_ppb $t_ns $t_mean_ns $min $max $f_tune $cte $dac_value $resyncs
                fi
	fi
	sleep 0.5
done
