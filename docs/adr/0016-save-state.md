# 0016. Save state: format byte sendiri, berversi, dimuat semua-atau-tidak

- Status: Diterima
- Tanggal: 2026-10-14

## Konteks

M8 menambah save state (`visi.md` F8): potret seluruh mesin — CPU, RAM,
VRAM, timer, PPU, APU, joypad, MBC — yang bisa dimuat kembali sampai ke
siklus yang sama. Berbeda dengan simpan permainan (ADR-0014), yang hanya
berisi RAM cartridge yang dikelola game sendiri.

Core `no_std` dan tidak memakai crate luar. Field komponen bersifat privat.
Yang perlu diputuskan: bentuk byte-nya, cara menolak state yang salah, dan
cara membuktikan tidak ada field yang terlupa.

## Pilihan yang dipertimbangkan

**Format**

1. `serde` + format biner: ringkas ditulis, tetapi menambah dependensi ke
   core dan menyembunyikan urutan field di balik derive.
2. **`Writer`/`Reader` kecil buatan sendiri**: setiap komponen menulis dan
   membaca field-nya sendiri, dalam urutan yang terlihat di kodenya.

**Membuktikan kelengkapan**

1. Bolak-balik saja (simpan → muat → jalan → bandingkan): field yang lupa
   disimpan hanya ketahuan kalau efeknya sempat mengubah field lain.
2. **Bolak-balik + perbandingan struktural**: setelah dimuat, mesin baru
   dibandingkan dengan aslinya *field demi field* (`PartialEq`), di banyak
   titik simpan dengan jarak berubah-ubah.

## Keputusan

- **Format sendiri**: `"MXMS"`, versi (u16), sidik ROM (FNV-1a 32-bit isi
  ROM), lalu CPU, bus, timer, PPU, APU, joypad, cartridge. Angka
  little-endian; deretan byte didahului panjangnya.
- **Versi**: naik setiap urutan/isi berubah; versi lain ditolak, bukan
  dibaca salah. **Sidik ROM**: state dari game lain ditolak.
- **Muat semua-atau-tidak**: state dibaca ke salinan mesin; hanya kalau
  seluruhnya valid (termasuk tidak ada sisa byte) salinan menggantikan
  mesin.
- **Tidak ikut disimpan, dengan sengaja**: ROM (sudah ada di perangkat),
  buffer/laju sampel dan filter suara (urusan adapter), `line_colors` PPU
  (dibangun ulang tiap baris). Setelah dimuat, RAM cartridge ditandai
  berubah supaya simpan permainan ikut memperbaruinya.
- **Bukti**: tes struktural di 3000 titik simpan (jarak 1–11 instruksi)
  dengan ROM yang memakai EI, interrupt timer, noise, window, MBC1+RAM;
  plus bolak-balik pada semua ROM uji dan homebrew di CI.
- Web: satu slot per ROM di localStorage (kunci simpan permainan + `:state`).

## Konsekuensi

- (+) Core tetap tanpa dependensi; urutan field terlihat dan bisa direview.
- (+) Field baru yang lupa ditambahkan ke `save`/`load` langsung membuat
  tes struktural gagal, dengan nama komponennya.
- (−) Menambah field = menulis dua baris lagi dan menaikkan `VERSION`;
  state lama tidak bisa dimuat (belum ada migrasi).
- (−) Ukuran state ±48 KB (lebih besar dengan RAM cartridge besar);
  disimpan sebagai base64 di localStorage.
