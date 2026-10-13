# 0004. GBDK-2020 untuk ROM homebrew dalam C

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Kita ingin (a) berlatih embedded C untuk CPU 8-bit, dan (b) punya ROM uji buatan
sendiri yang legal, kecil, dan bisa kita ubah sesuka hati untuk menguji emulator.

## Pilihan yang dipertimbangkan

1. **RGBDS (assembly)** — kontrol penuh, standar di komunitas, tetapi ini latihan
   assembly, bukan C.
2. **GBDK-2020 (C, berbasis SDCC)** — toolchain C yang aktif dirawat, punya library
   untuk grafis/input/suara, tersedia untuk Windows dan Linux.
3. **Toolchain C buatan sendiri dengan SDCC mentah** — sangat mendidik, tetapi
   terlalu banyak pekerjaan sebelum bisa menampilkan satu huruf.

## Keputusan

Pilihan 2. Versi dikunci (saat ini rilis terbaru adalah **4.5.0**; versi pasti
dicatat di skrip CI dan diperiksa ulang saat M3 dimulai).

- Kode sumber di `homebrew/<nama-program>/`.
- Di CI, GBDK diunduh dari rilis resminya (Linux), lalu ROM di-build dan langsung
  dijalankan oleh `gb-headless`.
- Di laptop Windows, GBDK dipasang dari arsip zip rilisnya (panduan ditambahkan saat M3).

## Konsekuensi

- (+) Latihan C nyata: memori terbatas, tipe 8/16-bit, register perangkat keras yang dipetakan ke memori.
- (+) ROM homebrew menjadi tes end-to-end: C → ROM → emulator kita → hasil yang bisa dicek.
- (−) SDCC menghasilkan kode yang kurang optimal dibanding assembly; tidak masalah untuk tujuan kita.
- (−) Satu toolchain tambahan di CI (diunduh, bukan dibangun — cepat). Baru dipakai mulai M3.
- Catatan: GBDK menyisipkan header cartridge standar (termasuk data logo yang diperiksa
  perangkat asli). Emulator kita tidak memerlukan pemeriksaan logo, karena kita
  tidak memakai boot ROM (ADR-0006).
