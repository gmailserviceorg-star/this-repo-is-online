#!/bin/bash
# Alpine minirootfs (musl+busybox) + packages + Stratum overlay -> squashfs(zstd)
set -euo pipefail
cd "$(dirname "$0")/.."; ROOT=$PWD; B=$ROOT/build; R=$B/rootfs; BIN=$B/bin
cleanup(){ for m in proc dev sys; do sudo umount -l $R/$m 2>/dev/null || true; done; }; trap cleanup EXIT
sudo rm -rf $R; mkdir -p $R
U=https://dl-cdn.alpinelinux.org/alpine/latest-stable
F=$(curl -fsSL $U/releases/x86_64/latest-releases.yaml | grep -o 'alpine-minirootfs-[0-9.]*-x86_64.tar.gz' | head -1)
AV=$(echo $F | grep -o '[0-9]*\.[0-9]*' | head -1)
[ -f $B/$F ] || curl -fsSLo $B/$F $U/releases/x86_64/$F
sudo tar -xzf $B/$F -C $R
printf 'https://dl-cdn.alpinelinux.org/alpine/v%s/main\nhttps://dl-cdn.alpinelinux.org/alpine/v%s/community\n' $AV $AV | sudo tee $R/etc/apk/repositories >/dev/null
sudo cp /etc/resolv.conf $R/etc/resolv.conf
for m in proc dev sys; do sudo mkdir -p $R/$m; sudo mount --bind /$m $R/$m; done
PK=$(cat programs/packages/*.txt | grep -v '^#' | tr '\n' ' ')
: > $B/skipped.txt
sudo chroot $R /sbin/apk add --no-cache $PK || {
 echo "bulk install failed; installing one by one"
 for p in $PK; do sudo chroot $R /sbin/apk add --no-cache $p || echo $p >> $B/skipped.txt; done; }
# Stratum overlay + layout
for d in network drivers services programs libs; do sudo cp -a $d/rootfs/. $R/; done
sudo mkdir -p $R/{System/home/uservice,System/etc/rusted,System/libexec,Apps,Users,Extras,boot,mnt/iso,mnt/ovl,mnt/sfs,var/lib/stratum,var/log,var/lib/stw}
sudo mkdir -p "$R/System/Offline pages/Tutors/www" "$R/System/Offline pages/legal"
sudo mkdir -p $R/usr/share/stratum; convert -size 1920x1080 gradient:"#0b1220-#24457f" $B/wallpaper.png 2>/dev/null && sudo cp $B/wallpaper.png $R/usr/share/stratum/ || true
sudo cp programs/tutorials/*.html "$R/System/Offline pages/Tutors/www/"
sudo cp programs/legal/*.md "$R/System/Offline pages/legal/"
sudo install -m755 $BIN/stw $R/usr/bin/stw; sudo install -m755 $BIN/stratumview $R/usr/bin/stratumview
for l in pkg unpkg stwc; do sudo ln -sf stw $R/usr/bin/$l; done
sudo find $R/etc/service $R/System/libexec $R/usr/bin -maxdepth 2 -type f \( -name run -o -name 'stratum-*' -o -name stage* \) -exec chmod 755 {} +
sudo chmod 755 $R/etc/xdg/labwc/autostart 2>/dev/null || true
# keys: CI secret STRATUM_SIGNING_KEY (hex seed) or ephemeral
KEY=$B/dev.key
if [ -n "${STRATUM_SIGNING_KEY:-}" ]; then echo "$STRATUM_SIGNING_KEY" > $KEY; else $BIN/stw keygen $KEY >/dev/null; fi
$BIN/stw pub $KEY | sudo tee $R/System/etc/rusted/trusted.pub >/dev/null
# Rusted.stw built with our own stwc
RUSTED_BIN=$BIN/rusted STW_KEY=$KEY $BIN/stwc build services/rusted/stw
sudo cp services/rusted/stw/dist/Rusted.stw $R/System/home/uservice/Rusted.stw
# strata -> labwc menu
$BIN/stw strata labwc programs/rootfs/System/etc/menus.strata | sudo tee $R/etc/xdg/labwc/menu.xml >/dev/null
# extras (on-demand components) built from apk into .pkg
mkdir -p $B/extras
mkextra(){ n=$1; shift
 sudo rm -rf $R/wr; sudo mkdir -p $R/wr
 if sudo chroot $R /sbin/apk --root /wr --initdb add --no-cache --keys-dir /etc/apk/keys --repositories-file /etc/apk/repositories "$@"; then
  sudo rm -rf $R/wr/var $R/wr/etc/apk $R/wr/lib/apk $R/wr/usr/share/doc $R/wr/usr/share/man
  sudo $BIN/pkg create $B/extras/$n.pkg $R/wr
 else echo "extra $n failed" >> $B/skipped.txt; fi
 sudo rm -rf $R/wr; }
mkextra wine wine xwayland
mkextra firmware linux-firmware-i915 linux-firmware-iwlwifi linux-firmware-rtl_nic linux-firmware-rtw88
# modules + trimming
sudo mkdir -p $R/lib/modules; sudo cp -a $B/modroot/lib/modules/. $R/lib/modules/
sudo depmod -b $R "$(cat $B/kver)"
sudo rm -rf $R/usr/share/{man,doc,info,gtk-doc,help} $R/var/cache/apk/* $R/root/.cache
# boot manifest (signed)
M=$B/boot.manifest; : > $M
for f in /System/home/uservice/Rusted.stw /System/etc/rusted/rusted.toml /System/etc/webview.toml /System/etc/rusted/trusted.pub; do echo "$(sha256sum $R$f | cut -d' ' -f1)  $f" >> $M; done
echo "$(sha256sum $B/vmlinuz | cut -d' ' -f1)  /mnt/iso/boot/vmlinuz" >> $M
echo "$(sha256sum $B/initramfs.img | cut -d' ' -f1)  /mnt/iso/boot/initramfs.img" >> $M
sudo cp $M $R/System/etc/rusted/boot.manifest
$BIN/stw sign-file $KEY $M | sudo tee $R/System/etc/rusted/boot.manifest.sig >/dev/null
cleanup
sudo rm -f $B/rootfs.sfs
sudo mksquashfs $R $B/rootfs.sfs -comp zstd -Xcompression-level 19 -b 256K -noappend -no-progress -e proc dev sys
sudo chown $USER $B/rootfs.sfs
ls -l $B/rootfs.sfs $B/extras $B/skipped.txt
