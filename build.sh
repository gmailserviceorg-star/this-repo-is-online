#!/bin/bash
# Local build (Linux, or Windows via WSL2 Ubuntu). Needs sudo + network. Output: build/stratum-os.iso
set -euo pipefail
cd "$(dirname "$0")"
need="curl jq xz bc flex bison cpio zstd gcc make mksquashfs xorriso grub-mkrescue mformat busybox convert grub-mkfont musl-gcc"
miss=""; for t in $need; do command -v $t >/dev/null || miss="$miss $t"; done
if [ -n "$miss" ]; then echo "missing tools:$miss"; echo "Ubuntu/WSL: sudo apt install -y build-essential bc flex bison libssl-dev libelf-dev libncurses-dev cpio zstd xz-utils jq curl squashfs-tools xorriso mtools grub-pc-bin grub-efi-amd64-bin grub-common busybox-static imagemagick fonts-dejavu-core musl-tools gcc-multilib qemu-system-x86 kmod"; exit 1; fi
command -v cargo >/dev/null || { echo "install Rust: curl https://sh.rustup.rs | sh"; exit 1; }
rustup target add x86_64-unknown-linux-musl
bash scripts/10-rust.sh
bash scripts/20-kernel.sh
bash scripts/40-initramfs.sh
bash scripts/30-rootfs.sh
[ "${SKIP_LAUNCHERS:-0}" = 1 ] || { bash scripts/launchers.sh linux && bash scripts/launchers.sh windows; mkdir -p build/launchers; cp -a out-launcher/. build/launchers/; } || echo "launchers failed (continuing without)"
bash scripts/50-iso.sh
echo "DONE: build/stratum-os.iso   (test: bash scripts/smoke.sh)"
