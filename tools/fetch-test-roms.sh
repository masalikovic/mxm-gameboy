#!/usr/bin/env bash
# Mengunduh ROM uji di tools/test-roms.tsv ke test-roms/ dan memverifikasi
# SHA-256-nya (ADR-0005). File yang sudah ada dan cocok tidak diunduh ulang.
# Windows: jalankan dari Git Bash.
#
# Dua sumber, keduanya dikunci di kepala tools/test-roms.tsv:
#   - path biasa  → satu file dari cermin retrio/gb-test-roms (commit dikunci);
#   - "c-sp:path" → satu file dari bundel zip c-sp/game-boy-test-roms (versi
#                   dan SHA-256 zip dikunci). Zip hanya diunduh kalau ada file
#                   dari bundel yang belum ada atau tidak cocok.
set -euo pipefail
cd "$(dirname "$0")/.."

manifest="tools/test-roms.tsv"
source_url="https://raw.githubusercontent.com/retrio/gb-test-roms"
commit="$(sed -n 's/^# Commit : \([0-9a-f]\{40\}\).*/\1/p' "${manifest}")"
bundle_url="$(sed -n 's/^# Bundel : \(https:[^ ]*\).*/\1/p' "${manifest}")"
bundle_sha="$(sed -n 's/^# Bundel : https:[^ ]* sha256 \([0-9a-f]\{64\}\).*/\1/p' "${manifest}")"
bundle="test-roms/.bundel.zip"
mkdir -p test-roms

# Hapus file yang checksum-nya salah supaya tidak pernah dipakai.
verify_or_delete() {
  local sha="$1" target="$2" name="$3"
  if ! echo "${sha}  ${target}" | sha256sum --check --status; then
    echo "SHA-256 ${name} TIDAK cocok. File dihapus; jangan dipakai." >&2
    rm -f "${target}"
    exit 1
  fi
}

fetch_bundle() {
  if [[ -f "${bundle}" ]] && echo "${bundle_sha}  ${bundle}" | sha256sum --check --status; then
    return
  fi
  echo "Mengunduh bundel ${bundle_url}"
  curl -sSfL --retry 3 -o "${bundle}" "${bundle_url}"
  verify_or_delete "${bundle_sha}" "${bundle}" "bundel"
}

grep -v '^#' "${manifest}" | while IFS=$'\t' read -r sha name remote _expected; do
  target="test-roms/${name}"
  if [[ -f "${target}" ]] && echo "${sha}  ${target}" | sha256sum --check --status; then
    continue
  fi
  echo "Mengambil ${name}"
  if [[ "${remote}" == c-sp:* ]]; then
    fetch_bundle
    unzip -p "${bundle}" "${remote#c-sp:}" > "${target}"
  else
    # Spasi dan koma di nama file sumber harus di-encode untuk URL.
    encoded="$(printf '%s' "${remote}" | sed 's/ /%20/g; s/,/%2C/g; s/(/%28/g; s/)/%29/g')"
    curl -sSfL --retry 3 -o "${target}" "${source_url}/${commit}/${encoded}"
  fi
  verify_or_delete "${sha}" "${target}" "${name}"
done
echo "ROM uji siap di test-roms/ (checksum cocok)."
