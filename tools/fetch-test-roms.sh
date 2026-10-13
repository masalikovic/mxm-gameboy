#!/usr/bin/env bash
# Mengunduh ROM uji di tools/test-roms.tsv ke test-roms/ dan memverifikasi
# SHA-256-nya (ADR-0005). File yang sudah ada dan cocok tidak diunduh ulang.
# Windows: jalankan dari Git Bash.
set -euo pipefail
cd "$(dirname "$0")/.."

source_url="https://raw.githubusercontent.com/retrio/gb-test-roms"
commit="$(sed -n 's/^# Commit : \([0-9a-f]\{40\}\).*/\1/p' tools/test-roms.tsv)"
mkdir -p test-roms

grep -v '^#' tools/test-roms.tsv | while IFS=$'\t' read -r sha name remote _expected; do
  target="test-roms/${name}"
  if [[ -f "${target}" ]] && echo "${sha}  ${target}" | sha256sum --check --status; then
    continue
  fi
  # Spasi dan koma di nama file sumber harus di-encode untuk URL.
  encoded="$(printf '%s' "${remote}" | sed 's/ /%20/g; s/,/%2C/g; s/(/%28/g; s/)/%29/g')"
  echo "Mengunduh ${name}"
  curl -sSfL --retry 3 -o "${target}" "${source_url}/${commit}/${encoded}"
  if ! echo "${sha}  ${target}" | sha256sum --check --status; then
    echo "SHA-256 ${name} TIDAK cocok. File dihapus; jangan dipakai." >&2
    rm -f "${target}"
    exit 1
  fi
done
echo "ROM uji siap di test-roms/ (checksum cocok)."
