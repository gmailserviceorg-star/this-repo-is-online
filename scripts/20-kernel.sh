#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."; ROOT=$PWD; B=$ROOT/build; mkdir -p $B/k; cd $B/k
V=${KERNEL_VERSION:-$(curl -fsSL https://www.kernel.org/releases.json | jq -r '.releases[]|select(.moniker=="longterm")|.version' | sort -V | tail -1)}
echo "kernel $V"
[ -f linux-$V.tar.xz ] || curl -fsSLO https://cdn.kernel.org/pub/linux/kernel/v${V%%.*}.x/linux-$V.tar.xz
curl -fsSL https://cdn.kernel.org/pub/linux/kernel/v${V%%.*}.x/sha256sums.asc | grep " linux-$V.tar.xz\$" | sha256sum -c -
rm -rf linux-$V; tar xf linux-$V.tar.xz; cd linux-$V
make tinyconfig
scripts/kconfig/merge_config.sh -m .config $ROOT/kernel/core.config $ROOT/drivers/drivers.config $ROOT/network/network.config
make olddefconfig
sh $ROOT/kernel/check-config.sh .config
make -j"$(nproc)" bzImage modules
rm -rf $B/modroot; make INSTALL_MOD_PATH=$B/modroot INSTALL_MOD_STRIP=1 modules_install
cp arch/x86/boot/bzImage $B/vmlinuz; echo $V > $B/kver
cp .config $B/kernel.config
