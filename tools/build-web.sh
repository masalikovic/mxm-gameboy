#!/usr/bin/env bash
# Build halaman web: gb-web (Rust) → .wasm → lem JavaScript, lalu rakit dist/.
# web/ adalah SUMBER (di-commit); dist/ adalah HASIL (tidak di-commit, ADR-0009).
# Dipakai di CI dan di Linux/macOS. Versi Windows: tools/build-web.ps1.
# Langkah-langkahnya sengaja terlihat satu per satu (ADR-0003).
set -euo pipefail

cd "$(dirname "$0")/.."

# 1. Versi wasm-bindgen-cli harus sama persis dengan crate wasm-bindgen di
#    Cargo.lock. Kalau beda, file lem JS tidak cocok dengan .wasm dan halaman
#    gagal dimuat dengan error yang membingungkan. Lebih baik berhenti di sini.
expected="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen-cli belum terpasang. Pasang dengan:" >&2
  echo "  cargo install wasm-bindgen-cli --version ${expected} --locked" >&2
  exit 1
fi
installed="$(wasm-bindgen --version | awk '{ print $2 }')"
if [[ "${installed}" != "${expected}" ]]; then
  echo "Versi wasm-bindgen-cli ${installed} tidak sama dengan Cargo.lock (${expected})." >&2
  echo "  cargo install wasm-bindgen-cli --version ${expected} --locked --force" >&2
  exit 1
fi

# 2. Compile adapter browser ke WebAssembly.
cargo build -p gb-web --release --target wasm32-unknown-unknown

# 3. Rakit dist/ dari web/ (sumber halaman), mulai dari folder kosong.
rm -rf dist
cp -R web dist

# 4. Buat lem JavaScript (ES module) yang memanggil fungsi #[wasm_bindgen].
wasm-bindgen --target web --no-typescript --out-dir dist/pkg \
  target/wasm32-unknown-unknown/release/gb_web.wasm

# 4b. ROM homebrew kita (kalau sudah dibuild tools/build-homebrew.sh) ikut
#     disajikan sebagai demo (ADR-0006: ROM homebrew boleh dideploy).
#     Tidak ada ROM pihak ketiga yang pernah disalin ke sini.
mkdir -p dist/roms
for rom in homebrew/*/build/*.gb; do
  [[ -f "${rom}" ]] && cp "${rom}" dist/roms/
done

# 5. Cache busting (ADR-0009): ganti __BUILD__ dengan versi build, supaya
#    URL file berubah setiap build dan browser HP tidak memakai file lama.
#    Versi = hash commit; kalau ada perubahan yang belum di-commit, ditambah waktu.
build="$(git rev-parse --short=12 HEAD 2>/dev/null || echo lokal)"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
  build="${build}-$(date +%s)"
fi
sed -i.bak "s/__BUILD__/${build}/g" dist/index.html dist/main.js dist/input.js dist/save.js dist/audio.js dist/debugger.js dist/sw.js
# 5b. Service worker (ADR-0017) perlu tahu ROM demo mana yang ikut disimpan
#     untuk offline: daftar nama berkas di dist/roms sebagai array JSON.
roms="$(cd dist/roms && ls -1 ./*.gb 2>/dev/null | sed 's|^\./||; s/.*/"&"/' | paste -sd, -)"
sed -i.bak "s/__ROMS__/[${roms}]/" dist/sw.js
rm -f dist/*.bak

echo "Selesai: dist/ siap (build ${build}). Coba lokal: cd dist && python3 -m http.server 8000"
