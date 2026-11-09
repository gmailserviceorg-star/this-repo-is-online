#!/bin/bash
# usage: launchers.sh linux|windows|mac   -> out-launcher/{Linux,Windows,Macs}
set -euo pipefail
cd "$(dirname "$0")/.."; O=$PWD/out-launcher; mkdir -p $O
case $1 in
linux)
 sudo apt-get update -qq; sudo apt-get install -y -qq gcc-multilib dpkg
 mkdir -p $O/Linux
 for x in 32:i686-unknown-linux-gnu:i386 64:x86_64-unknown-linux-gnu:amd64; do IFS=: read n t a <<<"$x"
  rustup target add $t; cargo build --release -p bootset --target $t
  d=$(mktemp -d); mkdir -p $d/usr/bin $d/DEBIAN; cp target/$t/release/bootset $d/usr/bin/stratum-bootset
  printf 'Package: stratum-bootset\nVersion: 1.0\nArchitecture: %s\nMaintainer: Stratum\nDescription: Stratum OS USB writer (CLI)\n' $a > $d/DEBIAN/control
  dpkg-deb --build $d $O/Linux/bootset$n.deb; done;;
windows)
 sudo apt-get update -qq; sudo apt-get install -y -qq gcc-mingw-w64
 mkdir -p $O/Windows
 for x in 32:i686-pc-windows-gnu 64:x86_64-pc-windows-gnu; do IFS=: read n t <<<"$x"
  rustup target add $t; cargo build --release -p bootset --target $t; cp target/$t/release/bootset.exe $O/Windows/bootset$n.exe; done;;
mac)
 mkdir -p $O/Macs
 rustup target add x86_64-apple-darwin aarch64-apple-darwin
 for t in x86_64-apple-darwin aarch64-apple-darwin; do cargo build --release -p bootset --target $t; done
 lipo -create -output bootset-universal target/x86_64-apple-darwin/release/bootset target/aarch64-apple-darwin/release/bootset
 app(){ a=$O/Macs/$1.app; mkdir -p $a/Contents/MacOS
  cat > $a/Contents/Info.plist <<P
<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>Stratum Setup</string><key>CFBundleExecutable</key><string>launch</string><key>CFBundleIdentifier</key><string>org.stratum.$1</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>
P
  printf '#!/bin/sh\nD="$(cd "$(dirname "$0")" && pwd)"\n%s\n' "$2" > $a/Contents/MacOS/launch; chmod +x $a/Contents/MacOS/launch; }
 app bootset64 'osascript -e "tell application \"Terminal\" to do script \"sudo \\\"$D/bootset\\\"\"" -e "tell application \"Terminal\" to activate"'
 cp bootset-universal $O/Macs/bootset64.app/Contents/MacOS/bootset
 app bootset32 'osascript -e "display dialog \"32-bit macOS is not supported by modern toolchains. Use bootset64.app.\" buttons {\"OK\"}"';;
esac
