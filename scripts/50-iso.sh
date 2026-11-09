#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."; ROOT=$PWD; B=$ROOT/build; I=$B/iso
rm -rf $I; mkdir -p $I/boot/grub/theme $I/Extras $I/Macs $I/Windows $I/Linux
cp $B/vmlinuz $B/initramfs.img $B/rootfs.sfs $I/boot/
H=$(sha256sum $B/rootfs.sfs | cut -d' ' -f1)
sed "s/@SFS_SHA256@/$H/" kernel/grub/grub.cfg.in > $I/boot/grub/grub.cfg
cp kernel/grub/theme.txt $I/boot/grub/theme/
F=$(fc-list : file | grep -i 'DejaVuSans.ttf' | head -1 | cut -d: -f1 || true)
[ -n "$F" ] && grub-mkfont -s 18 -n "Stratum Sans 16" -o $I/boot/grub/theme/stratum.pf2 "$F" || echo "no font: theme disabled"
convert -size 1280x720 gradient:'#0b1220-#24457f' -gravity north -fill '#e8f0ff' -pointsize 64 -annotate +0+120 'STRATUM OS' $I/boot/grub/theme/bg.png 2>/dev/null || true
for m in /boot/memtest86+x64.bin /boot/memtest86+.bin; do [ -f $m ] && { cp $m $I/boot/memtest.bin; break; }; done
cp $B/extras/*.pkg $I/Extras/ 2>/dev/null || true
cp programs/extras/README.txt $I/Extras/
[ -d $B/launchers ] && cp -a $B/launchers/. $I/ || echo "launchers missing"
grub-mkrescue -o $B/stratum-os.iso $I -- -volid STRATUM 2>&1 | tail -3
{ echo "== size report (MB) =="; du -m $B/vmlinuz $B/initramfs.img $B/rootfs.sfs $B/extras/*.pkg $B/stratum-os.iso; } | tee $B/size-report.txt
