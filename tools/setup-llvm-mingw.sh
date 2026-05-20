#!/usr/bin/env bash
# setup-llvm-mingw.sh — baixa toolchain llvm-mingw para cross-compilar Vex
# do WSL2 Ubuntu para Windows (.exe).
#
# Mantenedor da toolchain: Martin Storsjö (https://github.com/mstorsjo/llvm-mingw)
# UCRT é usada (não MSVCRT) por ser o C runtime moderno do Windows 10+.

set -euo pipefail

VERSION="${VERSION:-20251123}"   # data do release; atualize periodicamente
TARGET_DIR="${TARGET_DIR:-tools/llvm-mingw}"
ARCH="$(uname -m)"

UBUNTU_VER="20.04"
TARBALL="llvm-mingw-${VERSION}-ucrt-ubuntu-${UBUNTU_VER}-${ARCH}.tar.xz"
URL="https://github.com/mstorsjo/llvm-mingw/releases/download/${VERSION}/${TARBALL}"

if [ -d "${TARGET_DIR}" ]; then
    echo "llvm-mingw já instalado em ${TARGET_DIR}. Apague para reinstalar."
    exit 0
fi

mkdir -p "$(dirname "${TARGET_DIR}")"
echo "Baixando ${URL} ..."
curl -fSL "${URL}" -o "/tmp/${TARBALL}"

echo "Extraindo ..."
mkdir -p "${TARGET_DIR}"
tar -xf "/tmp/${TARBALL}" --strip-components=1 -C "${TARGET_DIR}"
rm "/tmp/${TARBALL}"

echo
echo "Pronto. Adicione ao seu shell:"
echo "  export PATH=\"\$PWD/${TARGET_DIR}/bin:\$PATH\""
echo
echo "Para Rust cross-compile:"
echo "  rustup target add x86_64-pc-windows-gnu"
echo "  cargo build --target x86_64-pc-windows-gnu"
