# 0014. Simpan permainan di browser: localStorage, kunci dari isi ROM

- Status: Diterima
- Tanggal: 2026-10-14

## Konteks

Cartridge berbaterai menjaga isi RAM-nya saat Game Boy dimatikan; begitulah
game menyimpan kemajuan. ADR-0013 meletakkan RAM di `Cartridge` dan memberi
adapter tiga pintu: `ram()`, `load_ram()`, dan `take_ram_dirty()`. Adapter
web perlu memutuskan di mana RAM disimpan, dengan kunci apa, dan kapan.

## Pilihan yang dipertimbangkan

**Tempat**

1. **localStorage**: API sinkron yang sederhana; cukup untuk RAM terbesar
   yang kita dukung (128 KiB → ±171 KiB teks base64; batas umum ±5 MB).
2. IndexedDB: bisa menyimpan byte langsung dan jauh lebih besar, tetapi API
   asinkron dan lebih rumit untuk data sekecil ini.
3. File unduhan (.sav): pengguna harus mengunggah ulang setiap kali.

**Kunci**

1. Judul dari header: dua ROM berbeda dengan judul sama saling menimpa.
2. SHA-256 isi ROM lewat `crypto.subtle`: tidak tersedia di `http://` alamat
   jaringan lokal (cara menguji dari laptop ke HP di M0).
3. **Judul + FNV-1a 32-bit isi ROM**: cukup unik untuk koleksi pribadi,
   jalan di mana saja, dan tetap terbaca manusia (`JERUK:6c02680c`).

**Kapan menyimpan**

1. Setiap frame: menulis ke penyimpanan 60× per detik.
2. **Saat RAM berubah, paling sering 1× per detik, ditambah saat tab
   disembunyikan atau ditutup.**

## Keputusan

localStorage, kunci `mxm-gameboy:simpan:<judul>:<fnv1a>`, disimpan saat RAM
berubah (≤ 1×/detik) dan saat `visibilitychange`/`pagehide`. Hanya untuk
cartridge berbaterai. Semua akses dibungkus `try/catch`: tanpa penyimpanan
(mode privat), game tetap jalan dan pengguna diberi tahu. Simpanan tidak
pernah meninggalkan perangkat (ADR-0006).

## Konsekuensi

- (+) Kode kecil di `web/save.js`; core tetap tidak tahu soal browser.
- (+) Pengguna bisa menghapus simpanan dari halaman.
- (−) Membersihkan data situs di browser ikut menghapus simpanan; belum ada
  ekspor/impor file .sav.
- (−) Perubahan RAM dalam detik terakhir sebelum browser dibunuh paksa bisa
  hilang (game asli juga punya risiko serupa saat baterai dicabut).
- Pantau: kalau kelak ada save state (snapshot seluruh mesin), ukurannya jauh
  lebih besar dan mungkin perlu IndexedDB.
