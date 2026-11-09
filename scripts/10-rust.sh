#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
T=x86_64-unknown-linux-musl
cargo build --release --target $T -p stw -p rusted -p stratumview
mkdir -p build/bin
for b in stw rusted stratumview; do cp target/$T/release/$b build/bin/; done
for l in pkg unpkg stwc; do ln -sf stw build/bin/$l; done
