# 0009. Cache busting: folder dist/ dan versi build di URL

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Setelah PR #6 (gaya MXM) di-deploy, HP pemilik masih menampilkan tampilan lama.
GitHub Pages mengirim header `Cache-Control: max-age=600`, jadi browser boleh
memakai `style.css`, `main.js` dan `.wasm` lama selama 10 menit tanpa bertanya
ke server. Kita tidak bisa mengubah header itu di GitHub Pages.

Akibat terburuk: `main.js` baru bertemu `.wasm` lama (atau sebaliknya) dan
halaman gagal dimuat.

## Pilihan yang dipertimbangkan

1. **Menunggu / menyuruh pengguna menghapus cache.** Tidak bisa diandalkan.
2. **Nama file berisi hash** (`style.3f2a.css`). Standar di bundler, tetapi
   perlu alat tambahan; kita sengaja tanpa bundler (ADR-0003).
3. **Query versi** (`style.css?v=<versi>`). Browser menganggap URL dengan query
   berbeda sebagai file berbeda. Cukup diganti dengan `sed` saat build.

## Keputusan

Pilihan 3, dengan pemisahan sumber dan hasil:

- `web/` adalah **sumber** halaman (di-commit). Di dalamnya, setiap URL file
  memakai penanda `?v=__BUILD__`.
- `tools/build-web.sh` / `.ps1` merakit `dist/`: salin `web/`, tambahkan
  hasil wasm-bindgen di `dist/pkg/`, lalu ganti `__BUILD__` dengan versi build.
  `dist/` tidak di-commit; CI mengunggah `dist/` ke GitHub Pages.
- Versi build = hash commit (12 karakter). Kalau ada perubahan yang belum
  di-commit, ditambah waktu Unix, supaya build lokal pun selalu berganti URL.
- Penanda dipakai di `index.html` (CSS, JS) dan `main.js` (lem JS dan `.wasm`).

Bagian alur build di ADR-0003 tetap berlaku, kecuali folder output
wasm-bindgen yang kini `dist/pkg`, bukan `web/pkg`.

## Konsekuensi

- (+) Setiap deploy langsung terlihat di HP; `.js` dan `.wasm` selalu sepasang.
- (+) Tanpa alat tambahan: cukup `cp` dan `sed`.
- (−) `index.html` sendiri tetap bisa di-cache sampai 10 menit. Dalam praktik,
  browser menanyakan ulang halaman utama saat dimuat ulang; file lain mengikuti.
- (−) Membuka `web/index.html` langsung tidak menjalankan emulator; selalu
  build dulu, lalu buka `dist/`.
- Aturan baru: **file baru di `web/` yang dirujuk halaman wajib memakai
  `?v=__BUILD__`**, kalau tidak ia bisa tertinggal di cache.
