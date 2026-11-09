#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."; B=$PWD/build; I=$B/initramfs; rm -rf $I
mkdir -p $I/{bin,proc,sys,dev,iso,sfs,ovl,newroot,data}
cp /bin/busybox $I/bin/busybox; install -m755 kernel/initramfs/init $I/init
(cd $I && find . | cpio -o -H newc --quiet | zstd -19 -T0 -q -o $B/initramfs.img -f)
