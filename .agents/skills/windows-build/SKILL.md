---
name: windows-build
description: Instructions for cross-compiling ALCOM Windows executables (ALCOM.exe) and Inno Setup installer (alcom-setup.exe) using distrobox debian-win container.
---

# Cross-Compiling ALCOM for Windows on Linux (Bazzite / Distrobox)

This guide documents how to build the Windows standalone binary (`ALCOM.exe`) and Inno Setup installer (`alcom-setup.exe`) inside a dedicated `debian-win` Distrobox container.

---

## 1. Container Setup (`debian-win`)

A dedicated container is used for Windows cross-compilation so that `wine32` and `i386` multiarch packages do not interfere with Linux AppImage packaging (`dpkg-query`).

### Create Container
```bash
distrobox create --name debian-win --image docker.io/library/debian:latest -Y
```

### Install Toolchain & Dependencies
```bash
distrobox enter debian-win -- bash -c "
  sudo dpkg --add-architecture i386 && \
  sudo apt-get update && \
  sudo apt-get install -y clang lld nsis nodejs npm pkg-config libssl-dev wine wine32 wine64 llvm && \
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
  \$HOME/.cargo/bin/rustup target add x86_64-pc-windows-msvc && \
  \$HOME/.cargo/bin/cargo install cargo-xwin --locked && \
  sudo ln -sf /usr/bin/clang-cl-19 /usr/local/bin/clang-cl && \
  sudo ln -sf /usr/bin/lld-link-19 /usr/local/bin/lld-link && \
  sudo ln -sf /usr/bin/llvm-ar-19 /usr/local/bin/llvm-ar && \
  sudo ln -sf /usr/bin/llvm-lib-19 /usr/local/bin/llvm-lib
"
```

---

## 2. Web Frontend Build

Before building Rust release binaries, ensure Vite static assets are built:
```bash
distrobox enter debian-win -- bash -c "npm --prefix vrc-get-gui run build"
```

---

## 3. Compiling the Windows Binary (`ALCOM.exe`)

> **CRITICAL**: Always include `--features custom-protocol` when building release binaries. Without this feature, static web assets will fail to embed, resulting in a blank/gray window on launch.

```bash
distrobox enter debian-win -- bash -c "
  export PATH=\$HOME/.cargo/bin:\$PATH && \
  cargo xwin build -p vrc-get-gui --target x86_64-pc-windows-msvc --release --features custom-protocol
"
```
**Output Binary**: `target/x86_64-pc-windows-msvc/release/ALCOM.exe`

---

## 4. Bundling the Windows Setup Installer (`alcom-setup.exe`)

Uses `cargo xtask bundle-alcom` which invokes Inno Setup (`ISCC.exe`) via `wine32`:

```bash
distrobox enter debian-win -- bash -c "
  export PATH=\$HOME/.cargo/bin:\$PATH && \
  cargo xtask bundle-alcom --target x86_64-pc-windows-msvc --release --bundles setup-exe
"
```
**Output Installer**: `target/x86_64-pc-windows-msvc/release/bundle/setup/alcom-setup.exe`

---

## 5. Uploading to GitHub Release

```bash
cp target/x86_64-pc-windows-msvc/release/ALCOM.exe ./alcom-1.1.9-beta.0-x86_64-pc-windows-msvc.exe
cp target/x86_64-pc-windows-msvc/release/bundle/setup/alcom-setup.exe ./alcom-setup-1.1.9-beta.0-x86_64.exe

gh release upload feat-kill-unity-1.1.9-beta.0 \
  ./alcom-1.1.9-beta.0-x86_64-pc-windows-msvc.exe \
  ./alcom-setup-1.1.9-beta.0-x86_64.exe \
  --repo Bluscream/vrc-get --clobber

rm -f ./alcom-1.1.9-beta.0-x86_64-pc-windows-msvc.exe ./alcom-setup-1.1.9-beta.0-x86_64.exe
```
