# 0013. Model cartridge: satu enum MBC di dalam gb-core

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Game Boy hanya bisa "melihat" 32 KiB ROM (0000–7FFF) dan 8 KiB RAM
cartridge (A000–BFFF) sekaligus. Game yang lebih besar membawa chip MBC
(*memory bank controller*) di dalam cartridge: tulisan ke area ROM bukan
mengubah ROM, tetapi perintah ke MBC untuk menukar "bank" yang terlihat.
Tipe MBC dibaca dari header cartridge (alamat 0x147).

Sampai M5, `SystemBus` memetakan ROM apa adanya dan mengabaikan tulisan.
M6 menambah MBC1, MBC2, MBC3 dan MBC5 (`visi.md` F6), lalu simpan permainan.
Yang perlu diputuskan: bagaimana tipe MBC yang berbeda-beda diwakili, dan apa
yang terjadi pada ROM yang tidak kita kenal.

## Pilihan yang dipertimbangkan

**Bentuk MBC**

1. **Trait `Mbc` + `Box<dyn Mbc>`**: mudah ditambah dari luar crate, tetapi
   setiap baca ROM lewat panggilan dinamis, dan keadaan MBC tersembunyi di
   balik trait (lebih sulit disimpan untuk save state kelak).
2. **Satu `enum Mbc` dengan varian per chip**: semua tipe terlihat di satu
   tempat; `match` dicek lengkap oleh compiler; data biasa, mudah disalin.
   Menambah MBC berarti mengubah crate ini.
3. **Generic `SystemBus<M: Mbc>`**: tercepat, tetapi tipe `GameBoy` jadi
   berbeda per MBC, padahal adapter baru tahu tipenya setelah ROM dibaca.

**ROM yang tidak dikenal**

1. Jalankan saja tanpa MBC: game mungkin berjalan aneh tanpa penjelasan.
2. **Tolak dengan error yang bisa dibaca**: `GameBoy::new` mengembalikan
   `Result`; adapter menampilkan "tipe cartridge 0x.. belum didukung".

## Keputusan

- **Pilihan 2 (enum)** dan **tolak dengan error**.
- Modul baru `gb-core/src/cartridge.rs`: `Header` (judul, tipe, ukuran ROM
  dan RAM, baterai, checksum), `Cartridge` (ROM, RAM, MBC), dan `Mbc`.
- Jumlah bank ROM dihitung dari **ukuran file** (dibulatkan ke pangkat dua),
  bukan dari header: lebih tahan terhadap header homebrew yang salah, dan
  nomor bank selalu di-*mask* ke bank yang benar-benar ada.
- RAM cartridge milik `Cartridge`. Adapter bisa membaca dan memasangnya
  kembali (simpan permainan, M6·2); core tidak tahu soal file atau browser.
- Jam MBC3 (RTC) **tidak berjalan**: registernya bisa ditulis dan dibaca,
  tetapi waktu tidak maju. Core harus deterministik (ADR-0011); waktu nyata
  kelak masuk dari adapter, bukan dari core.
- MBC1 "multicart" (beberapa game dalam satu cartridge) tidak didukung.

## Konsekuensi

- (+) Satu `match` per alamat; mudah dibaca dan dites, tanpa `dyn`.
- (+) ROM yang tidak didukung gagal dengan pesan jelas, bukan perilaku aneh.
- (−) `GameBoy::new` berubah menjadi `Result`; semua adapter ikut diubah.
- (−) Game yang bergantung pada jam (mis. siang/malam) melihat waktu beku.
- Pantau: ROM uji Mooneye MBC1/2/5 di `tools/test-roms.tsv` adalah bukti
  utama; perubahan aturan bank harus tetap meluluskannya.
