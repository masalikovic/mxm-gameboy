# 0003. Build WebAssembly dengan `cargo` + `wasm-bindgen-cli`, tanpa bundler

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

`gb-web` harus dikompilasi ke WebAssembly dan dimuat oleh halaman HTML di GitHub Pages.
Rust menghasilkan file `.wasm` mentah; untuk memanggilnya dengan nyaman dari JavaScript
dibutuhkan "lem" (glue code) yang dihasilkan oleh `wasm-bindgen`.

Situasi ekosistem per Oktober 2026 yang ikut menentukan:

- Organisasi GitHub `rustwasm` diumumkan akan diarsipkan pada Juli 2025.
  `wasm-bindgen` dipindah ke organisasi baru dengan maintainer baru;
  `wasm-pack` awalnya masuk daftar arsip dan kini dirawat terpisah oleh satu maintainer.
- Artinya: makin banyak lapisan alat, makin besar risiko salah satunya berhenti dirawat.

## Pilihan yang dipertimbangkan

1. **`wasm-pack`** — satu perintah, populer di tutorial. Tetapi menambah lapisan
   yang status perawatannya sempat tidak pasti, dan menyembunyikan apa yang terjadi.
2. **Trunk** — dev server + bundler untuk aplikasi Rust/wasm. Nyaman, tetapi kita
   akan banyak menulis JS sendiri (Web Audio, tombol sentuh), jadi "sihir"-nya kurang berguna.
3. **`cargo build --target wasm32-unknown-unknown` lalu `wasm-bindgen-cli`**, versi dikunci;
   JavaScript ditulis sebagai ES module biasa, **tanpa bundler/npm**.

## Keputusan

Pilihan 3. Alur build:

```
cargo build -p gb-web --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/gb_web.wasm
```

Versi `wasm-bindgen-cli` **harus sama persis** dengan versi crate `wasm-bindgen` di
`Cargo.lock`; CI akan memeriksanya.

## Konsekuensi

- (+) Hanya dua alat, keduanya inti dari ekosistem. Setiap langkah terlihat — cocok untuk belajar.
- (+) Tidak perlu Node.js/npm di laptop maupun CI.
- (−) Tidak ada hot reload; setelah mengubah Rust, jalankan skrip build lalu refresh browser.
- (−) Optimasi ukuran (`wasm-opt`) harus ditambah sendiri nanti bila perlu.
- Pantau: jika versi `wasm-bindgen` di-upgrade, versi CLI ikut di-upgrade dalam PR yang sama.
