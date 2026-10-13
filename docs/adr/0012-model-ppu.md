# 0012. Model PPU: digambar per baris, framebuffer tingkat kegelapan

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

M4 membangun PPU, chip yang menggambar layar. Di hardware asli, PPU
mengeluarkan satu piksel per siklus selama mode 3, dan panjang mode 3
berubah-ubah (172–289 siklus) tergantung scroll, window, dan sprite. Program
bisa mengubah register di tengah baris untuk efek khusus.

Kita perlu memutuskan seberapa teliti meniru ini, dan apa bentuk keluaran
PPU untuk adapter (browser, CLI, kelak cyberdeck).

## Pilihan yang dipertimbangkan

**Waktu menggambar**

1. **Per piksel (pixel FIFO)**: paling akurat, termasuk efek tengah baris.
   Jauh lebih rumit dan lebih lambat.
2. **Per baris**: satu baris digambar sekaligus saat masuk H-blank, memakai
   nilai register saat itu. Mode tetap berganti tepat waktu.
3. **Per frame**: seluruh layar digambar saat V-blank. Efek per baris
   (misalnya scroll yang berubah tiap baris lewat interrupt STAT) hilang.

**Bentuk keluaran**

1. Warna RGBA per piksel dari core.
2. **Tingkat kegelapan 0–3 per piksel** (setelah palet `BGP`); warna nyata
   urusan adapter.

## Keputusan

- Waktu: **pilihan 2 (per baris)**. Mode 2 = 80 siklus, mode 3 = 172 siklus
  (selalu), H-blank = sisanya hingga 456. 154 baris per frame.
- Keluaran: **tingkat kegelapan 0–3**, 160×144 byte, di heap (`Box<[u8]>`).
  Adapter memetakan ke palet MXM (ADR-0008) atau palet lain.
- Interrupt: V-blank saat `LY` menjadi 144; STAT pada **tepi naik** gabungan
  sumber yang dipilih (mode 0/1/2, `LY == LYC`).
- OAM DMA (0xFF46) menyalin 160 byte seketika.
- `GameBoy::run_frame()` menjalankan CPU sampai frame selesai; adapter
  memanggilnya sekali per refresh layar.
- M4 hanya background; window dan sprite menyusul di M5.

## Konsekuensi

- (+) Mode, `LY`, dan interrupt tepat waktu pada tingkat baris: cukup untuk
  sebagian besar game dan untuk 11/11 ROM Blargg.
- (+) Core tidak tahu warna, layar, atau ukuran canvas. Adapter bebas.
- (+) Golden test (hash framebuffer) bisa dibuat dari program C kita sendiri.
- (−) Efek yang mengubah register di **tengah** baris tidak muncul.
- (−) Panjang mode 3 tidak bergantung sprite/scroll; ROM uji timing PPU
  yang teliti (mis. mooneye) akan gagal.
- (−) VRAM/OAM tidak dikunci selama mode 2/3, jadi program yang salah tulis
  di waktu terlarang tetap "berhasil" di emulator kita.
- Pantau: kalau game nyata butuh efek tengah baris, ADR baru bisa
  menggantikan ini dengan pixel FIFO tanpa mengubah API framebuffer.
