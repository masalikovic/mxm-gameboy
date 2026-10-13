# 0006. Lisensi kode & kebijakan ROM

- Status: Diterima
- Tanggal: 2026-10-13

> Catatan: ini keputusan teknis proyek, bukan nasihat hukum.

## Konteks

Repo ini public. Ada tiga jenis materi yang perlu diatur:

1. Kode kita sendiri (Rust, C, JS, dokumen).
2. ROM uji buatan orang lain.
3. Materi milik Nintendo (boot ROM, game komersial).

Hasil pengecekan lisensi ROM uji (Oktober 2026):

| ROM uji | Lisensi | Keterangan |
|---|---|---|
| mooneye test suite (Gekkio) | MIT | Ada build siap pakai dari pembuatnya |
| dmg-acid2 (Matt Currie) | MIT | ROM siap pakai di rilis v1.0 |
| blargg (`cpu_instrs`, dll.) | **Tidak ditemukan pernyataan lisensi** | Beredar luas di mirror komunitas, tetapi izin redistribusinya tidak tertulis |
| Koleksi `c-sp/game-boy-test-roms` | Repo: MIT; tiap suite mengikuti lisensi aslinya | Praktis sebagai satu sumber unduhan berversi |

## Pilihan yang dipertimbangkan

Untuk kode kita:
1. MIT saja. 2. Apache-2.0 saja. 3. **MIT OR Apache-2.0** (dual license, konvensi ekosistem Rust).

Untuk ROM uji:
1. Commit semua ke repo — praktis, tetapi untuk blargg kita mendistribusikan ulang tanpa izin tertulis.
2. Commit yang MIT saja, unduh blargg — dua mekanisme berbeda, membingungkan.
3. **Tidak commit satu pun; semua diunduh saat tes** dari sumber berversi, dicek SHA-256.

## Keputusan

- Kode kita: **MIT OR Apache-2.0**.
- ROM uji: **pilihan 3**. Folder unduhan masuk `.gitignore`. ROM uji tidak pernah
  ikut dideploy ke GitHub Pages.
- **Tidak ada boot ROM Nintendo.** Emulator mulai langsung dalam keadaan "setelah boot"
  (nilai register & I/O yang terdokumentasi publik, seolah boot ROM baru saja selesai).
- **Tidak ada ROM komersial** di repo, issue, CI, atau contoh. Pengguna boleh memuat
  file ROM miliknya sendiri di browser; file itu tidak pernah diunggah ke mana pun.
- Kode sumber homebrew kita (C/GBDK) di-commit; ROM hasil build-nya dibuat ulang
  di CI dan boleh ikut dideploy ke GitHub Pages.

## Konsekuensi

- (+) Repo bebas dari materi yang status hukumnya meragukan.
- (+) Dual license memudahkan orang lain memakai kode kita (pola yang sama dengan Rust sendiri).
- (−) Tes ROM butuh unduhan pertama; diatasi dengan cache di GitHub Actions.
- (−) Tanpa boot ROM, animasi logo saat start tidak ada. Bisa diganti animasi buatan sendiri kelak.
