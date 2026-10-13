# 0010. Bentuk CPU: trait Bus, decode lewat pola bit, siklus dalam T-cycle

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

M1 mulai menulis CPU SM83. Sebelum instruksi pertama, tiga hal perlu
diputuskan karena akan dipakai oleh ratusan instruksi berikutnya:

1. Bagaimana CPU mengakses memori, padahal cartridge, PPU, dan timer belum ada?
2. Bagaimana 256 opcode di-decode tanpa menjadi kode yang tidak terbaca?
3. Satuan waktu apa yang dikembalikan CPU ke komponen lain?

## Pilihan yang dipertimbangkan

**Akses memori**

1. CPU memiliki array memori sendiri. Sederhana, tetapi cartridge dan I/O
   nanti harus dijejalkan ke dalam CPU.
2. **CPU menerima `&mut impl Bus`** di setiap `step`. CPU tidak tahu apa yang
   ada di balik alamat; tes memakai `FlatBus` 64 KiB.

**Decode opcode**

1. Satu `match` dengan 256 cabang. Mudah dicari, tetapi `LD r,r'` saja
   butuh 63 cabang yang hampir sama.
2. Tabel 256 pointer fungsi. Cepat, tetapi alurnya sulit diikuti pemula.
3. **`match` per kelompok + pola bit** (`yyy` = tujuan, `zzz` = sumber).
   Satu cabang menangani satu kelompok instruksi; register dipilih dengan
   `read_r` / `write_r` (urutan `B C D E H L (HL) A`).

**Satuan waktu**

1. M-cycle (1 M = 4 T). Angka lebih kecil, tetapi PPU bekerja per T-cycle.
2. **T-cycle** (4,19 MHz), angka yang sama dengan tabel opcode di Pan Docs.

## Keputusan

- Akses memori: pilihan 2, `trait Bus { read, write }` di `gb-core/src/bus.rs`.
- Decode: pilihan 3. Tabel pointer fungsi bisa dipertimbangkan nanti kalau
  pengukuran kinerja (M4) menunjukkan `match` menjadi hambatan.
- Waktu: T-cycle. `Cpu::step` mengembalikan `Result<u8, CpuError>`.
- Opcode yang belum ditulis mengembalikan `CpuError::Unimplemented`, dan PC
  dikembalikan ke alamat opcode. Tidak ada `panic!` di core.
- CPU mulai dari keadaan register **setelah** boot ROM DMG (PC = 0x0100),
  karena boot ROM Nintendo tidak kita pakai (ADR-0006).

## Konsekuensi

- (+) CPU bisa diuji penuh sebelum komponen lain ada.
- (+) Satu cabang `match` = satu baris di tabel opcode, mudah dicocokkan.
- (+) Adapter tidak pernah crash karena opcode yang belum ada; ia menerima error.
- (−) `&mut impl Bus` membuat CPU generik; kode mesin dibuat per jenis bus.
  Untuk satu bus asli ini tidak masalah.
- Pantau: setiap tes instruksi wajib memeriksa **flag dan jumlah siklus**,
  bukan hanya hasil register. Kasus batas (mis. `0x8 + 0x7` vs `0x8 + 0x8`
  untuk half carry) wajib ada; tanpa itu bug "lebih-kecil vs lebih-kecil-sama"
  lolos, seperti yang terbukti saat PR pertama M1 dibuat.
