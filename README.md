# Stratum OS (early, UNTESTED)
Alpine/musl userland + custom minimal Linux LTS kernel, GRUB (BIOS+UEFI), runit, labwc (Wayland), Wine/Darling as optional extras.
**Nothing here has been compiled or booted yet.** Expect first-build errors: paste them back and they get fixed.

## Build
- Local (Linux / Windows WSL2 Ubuntu): `./build.sh`  -> `build/stratum-os.iso`
- GitHub: push; Actions builds the ISO (artifact `stratum-os-iso`). Tag `v1` to publish a release.
- Test in QEMU: `bash scripts/smoke.sh` or `qemu-system-x86_64 -m 2G -cdrom build/stratum-os.iso`

## Parts (each separate)
| dir | what |
|---|---|
| `kernel/` | core kernel config, GRUB configs (live + A/B installed), initramfs init |
| `drivers/` | driver kernel config (USB/input/GPU/sound), modprobe policy, `stratum-usb`, firmware list |
| `network/` | network kernel config, dhcpcd/iwd/nftables, `stratum-net` (honest Wi-Fi) |
| `services/` | runit services, stage1, **Rusted** (Rust daemon + its `.stw` project) |
| `programs/` | desktop config, menus, setup flow, managers, apps list, tutorials, StratumView, bootset launcher |
| `libs/` | `library-make` + example (`.lib` files) |
| `runtime/` | `.pkg` format, `.stw` + `stwc`, Brix/Lash/Strata (`stw-pkg`, `stw-lang`, `stw`), schemas, examples |

## .lib files
`libs` file (C-like) -> `library-make` -> `build/NAME/makefile` + `Linker.ld` (GNU ld version script: only `export`ed symbols are visible) -> real linker (cc/ld) -> `dist/NAME.lib`. Installed under `/System/lib` (`LD_LIBRARY_PATH` set). Link: `cc app.c -L/System/lib -l:NAME.lib`. Tested on Linux with gcc.
## .pkg / .stw
`pkg create|ls|cat|put|patch|rm|compact`, `unpkg FILE.pkg` (extracts next to the file), `stwc build [--unsigned] DIR` (needs `MN`, a `.compiler`, and `compileinstruction/NAME.int`), `stw install|verify|run`. `.pkg`: 64-byte header + zstd(19) 1 MiB blocks + zstd index; edits append blocks and a new index, then rewrite the header (crash-safe); `pkg patch` rewrites only touched blocks. `.stw` = `.pkg` with `Softwaremanifest.xml`, `Termsofservice.md`, `PrivacyPolicy.md`, `Automated.xml`, `App.pkg`, `META/SIGNATURE` (ed25519 over per-file SHA-256 list; signer must be in `/System/etc/rusted/trusted.pub`). Examples: `runtime/examples/*`, `services/rusted/stw`. Tutorials: `/System/Offline pages/Tutors/www/{Brix,Lash,Strata}.html`.

## Honest limits (not done / different from the spec)
- Compositor is **labwc** (C, wlroots), not Smithay/Rust. No translucency/blur, no custom cursor theme, login is **tuigreet** (text), setup is **text-mode**.
- StratumView: engine **selection/health chain only** (`stratumview status|select|open`); no unified embed API, no WebView2Loader shim. Servo/CEF are not built (litehtml fallback = w3m in a terminal).
- Rusted: boot manifest verify, integrity watch + overlay rollback (best-effort), ro-lock of /System and /boot, signed unlock, audit log, Landlock+namespaces sandbox (`rusted run PROFILE -- cmd`). **No seccomp, no cgroup limits.** GRUB->kernel signature chain is not implemented (initramfs checks the rootfs SHA-256 given on the kernel command line; unsigned GRUB can be edited).
- `oops=warn` is not a real kernel parameter; panics cannot be made impossible: panic=5 + watchdogs + A/B slots only.
- Darling and CEF are **not built**; Wine/Xwayland/firmware `.pkg` are built from Alpine packages.
- Launchers are **CLI** (write ISO to a device). `bootset32.app` is a stub (no 32-bit macOS). Mac `.app` needs signing/notarization.
- Many apps are stubs (see `programs/apps/apps.tsv`). Right-click menus: desktop only. Clipboard/DnD broker: spice-vdagent only.
- Size targets (ISO <=250 MB, idle RAM <=256 MB) are unverified; see `build/size-report.txt`. Trim `programs/packages/*.txt` if too big.
- Signing key: set secret `STRATUM_SIGNING_KEY` (64 hex chars) or each build uses an ephemeral key.
