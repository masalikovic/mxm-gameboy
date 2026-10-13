#!/usr/bin/env bash
# Menjalankan setiap ROM di tools/test-roms.tsv dengan gb-headless dan
# membandingkan hasilnya dengan kolom "hasil-yang-diharapkan".
# Jalankan tools/fetch-test-roms.sh dulu. Windows: jalankan dari Git Bash.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p gb-headless
headless="target/release/gb-headless"

problems=0
printf '%-28s %-20s %-8s %s\n' "ROM" "diharapkan" "hasil" "status"
while IFS=$'\t' read -r _sha name _remote expected; do
  set +e
  "${headless}" "test-roms/${name}" > "test-roms/${name}.log" 2>&1
  code=$?
  set -e
  case "${code}" in
    0) actual="pass" ;;
    1) actual="fail" ;;
    *) actual="error" ;;
  esac
  if [[ "${expected}" == "pass" && "${actual}" == "pass" ]]; then
    status="ok"
  elif [[ "${expected}" == fail:* && "${actual}" != "pass" ]]; then
    status="ok (gagal sesuai harapan)"
  elif [[ "${expected}" == fail:* ]]; then
    status="BERUBAH: kini lulus, ubah manifest menjadi pass"
    problems=$((problems + 1))
  else
    status="GAGAL (log: test-roms/${name}.log)"
    problems=$((problems + 1))
  fi
  printf '%-28s %-20s %-8s %s\n' "${name}" "${expected}" "${actual}" "${status}"
done < <(grep -v '^#' tools/test-roms.tsv)

if (( problems > 0 )); then
  echo "${problems} ROM tidak sesuai harapan." >&2
  exit 1
fi
echo "Semua ROM sesuai harapan."
