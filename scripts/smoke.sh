#!/bin/bash
# boot the ISO headless; pass if the marker shows on the serial console
B=$(dirname "$0")/../build; rm -f $B/serial.log
qemu-system-x86_64 -m 1024 -cdrom $B/stratum-os.iso -boot d -display none -serial file:$B/serial.log -no-reboot & Q=$!
for i in $(seq 1 120); do grep -q STRATUM-BOOT-OK $B/serial.log 2>/dev/null && { echo "SMOKE OK"; kill $Q; exit 0; }; sleep 2; done
kill $Q; tail -40 $B/serial.log; echo "SMOKE FAIL"; exit 1
