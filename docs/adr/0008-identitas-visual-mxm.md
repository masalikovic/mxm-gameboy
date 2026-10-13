# 0008. Identitas visual: palet MXM

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Halaman M0 memakai palet hijau Game Boy DMG. Pemilik ingin emulator ini terasa
sebagai bagian dari keluarga MXM, yang identitasnya sudah tetap: ink `#0E0E0E`,
cream `#F5EBD7`, orange `#FF8A3D`, dan abu-abu tenang, dengan UI yang tidak ramai.

Game Boy asli hanya punya **4 tingkat warna** per piksel (nilai 0–3). Warna
yang tampil sepenuhnya urusan frontend; `gb-core` hanya mengeluarkan angka 0–3.

## Pilihan yang dipertimbangkan

1. **Tetap hijau DMG.** Otentik, tetapi tidak terasa seperti MXM.
2. **Bingkai MXM, layar tetap hijau.** Kompromi, tetapi dua identitas bertabrakan.
3. **Bingkai dan layar MXM.** Layar memakai 4 tingkat dari palet MXM.

## Keputusan

Pilihan 3. Empat tingkat layar, gelap → terang:

| Nilai piksel | Warna | Asal |
|---|---|---|
| 0 | `#0E0E0E` | ink |
| 1 | `#7A3F17` | orange digelapkan, jembatan ink → orange |
| 2 | `#FF8A3D` | orange |
| 3 | `#F5EBD7` | cream |

Warna-warna ini didefinisikan sekali sebagai variabel CSS di `web/style.css`
(`--lcd-0` … `--lcd-3`).

## Konsekuensi

- (+) Emulator langsung dikenali sebagai bagian dari MXM.
- (+) `gb-core` tidak berubah: palet murni urusan adapter (ADR-0002 tetap utuh).
- (−) Game terlihat berbeda dari Game Boy asli. Bisa diatasi di M4/M5 dengan
  pilihan palet ("MXM" atau "DMG klasik"), karena hanya tabel 4 warna yang berganti.
- Pantau: kontras tingkat 1 dan 2 harus cukup terbaca saat PPU mulai
  menggambar (M4); kalau tidak, tingkat 1 disesuaikan lewat ADR ini.
