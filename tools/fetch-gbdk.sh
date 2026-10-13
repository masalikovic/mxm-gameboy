#!/usr/bin/env bash
# Mengunduh GBDK-2020 (compiler C untuk Game Boy, ADR-0004) ke .gbdk/ dan
# memverifikasi SHA-256-nya. Versi dikunci di sini; ganti hanya lewat PR.
# Untuk Linux/CI. Windows: lihat docs/setup-windows.md langkah 8.
set -euo pipefail
cd "$(dirname "$0")/.."

version="4.5.0"
sha256="d7857a5f6d135ee4c249043ca26aad9f2ec8ab5d4106d97720d404114f42605c"
url="https://github.com/gbdk-2020/gbdk-2020/releases/download/${version}/gbdk-linux64.tar.gz"

if [[ -x .gbdk/gbdk/bin/lcc && "$(cat .gbdk/VERSION 2>/dev/null)" == "${version}" ]]; then
  echo "GBDK ${version} sudah ada di .gbdk/"
  exit 0
fi

rm -rf .gbdk
mkdir -p .gbdk
echo "Mengunduh GBDK ${version}"
curl -sSfL --retry 3 -o .gbdk/gbdk.tar.gz "${url}"
if ! echo "${sha256}  .gbdk/gbdk.tar.gz" | sha256sum --check --status; then
  echo "SHA-256 arsip GBDK TIDAK cocok. Arsip dihapus." >&2
  rm -rf .gbdk
  exit 1
fi
tar -xzf .gbdk/gbdk.tar.gz -C .gbdk
rm .gbdk/gbdk.tar.gz
echo "${version}" > .gbdk/VERSION
echo "GBDK ${version} siap di .gbdk/gbdk/"
