#!/usr/bin/env bash
# Build setiap program C di homebrew/<nama>/main.c menjadi
# homebrew/<nama>/build/<nama>.gb (folder build tidak di-commit, ADR-0006),
# lalu jalankan dengan gb-headless. Setiap program wajib mencetak "Passed".
#
# Lokasi GBDK: variabel GBDK_HOME, atau .gbdk/gbdk (tools/fetch-gbdk.sh).
# Windows (Git Bash): GBDK_HOME=/c/gbdk ./tools/build-homebrew.sh
set -euo pipefail
cd "$(dirname "$0")/.."

gbdk="${GBDK_HOME:-.gbdk/gbdk}"
lcc="${gbdk}/bin/lcc"
[[ -x "${lcc}" || -x "${lcc}.exe" ]] || {
  echo "GBDK tidak ditemukan di ${gbdk}. Jalankan tools/fetch-gbdk.sh atau set GBDK_HOME." >&2
  exit 1
}

cargo build --release -p gb-headless
headless="target/release/gb-headless"

problems=0
for dir in homebrew/*/; do
  name="$(basename "${dir}")"
  mkdir -p "${dir}build"
  rom="${dir}build/${name}.gb"
  echo "== ${name}: build"
  # Flag tambahan per program (opsional), mis. tipe MBC dan RAM cartridge:
  # homebrew/<nama>/lcc.flags berisi "-Wm-yt0x03 -Wm-ya1".
  flags=()
  [[ -f "${dir}lcc.flags" ]] && read -r -a flags < "${dir}lcc.flags"
  "${lcc}" "${flags[@]}" -o "${rom}" "${dir}main.c"
  echo "== ${name}: jalankan"
  if "${headless}" "${rom}"; then
    echo "== ${name}: ok"
  else
    echo "== ${name}: GAGAL" >&2
    problems=$((problems + 1))
  fi
  # Golden test (opsional), satu tes per baris: "<frames> <hash> [tombol]".
  # Tombol (mis. "right,a") ditahan selama semua frame, supaya input ikut diuji.
  #   screen.hash → hash layar setelah N frame;
  #   sound.hash  → hash suara (sampel 16-bit, 48 kHz) selama N frame.
  for kind in screen sound; do
    file="${dir}${kind}.hash"
    [[ -f "${file}" ]] || continue
    case_no=0
    while read -r frames expected buttons; do
      case_no=$((case_no + 1))
      extra=()
      [[ -n "${buttons}" ]] && extra+=(--hold "${buttons}")
      if [[ "${kind}" == screen ]]; then
        out="${dir}build/${name}-${case_no}.pgm"
        extra+=(--screenshot "${out}")
        what="layar"
      else
        out="${dir}build/${name}-${case_no}.wav"
        extra+=(--audio --wav "${out}")
        what="suara"
      fi
      actual="$("${headless}" "${rom}" --frames "${frames}" "${extra[@]}")"
      label="${frames} frame${buttons:+, tahan ${buttons}}"
      if [[ "${actual}" == "${expected}" ]]; then
        echo "== ${name}: ${what} cocok (${label}, ${actual})"
      else
        echo "== ${name}: ${what^^} BERBEDA (${label}): ${actual}, seharusnya ${expected} (lihat ${out})" >&2
        problems=$((problems + 1))
      fi
    done < <(grep -v '^#' "${file}")
  done
done

if (( problems > 0 )); then
  echo "${problems} pemeriksaan homebrew gagal." >&2
  exit 1
fi
echo "Semua program homebrew Passed."
