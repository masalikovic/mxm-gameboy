# 0011. Waktu dan interrupt: komponen dimajukan setelah setiap instruksi

- Status: Diterima
- Tanggal: 2026-10-13

## Konteks

Di Game Boy asli, CPU, timer, layar (PPU), dan suara bekerja **bersamaan**,
didorong satu jam 4,19 MHz. Di emulator, mereka harus bergiliran. Mulai M3
ada komponen kedua (timer) yang bisa **menyela** CPU lewat interrupt, jadi
cara bergiliran ini perlu diputuskan sebelum PPU (M4) dan suara (M7) datang.

## Pilihan yang dipertimbangkan

1. **Per T-cycle**: setiap siklus, semua komponen maju satu langkah, termasuk
   di tengah instruksi CPU. Paling akurat, tetapi CPU harus dipecah menjadi
   langkah per siklus, kodenya jauh lebih rumit, dan lebih lambat.
2. **Per instruksi ("catch-up")**: CPU menjalankan satu instruksi utuh,
   lalu komponen lain dimajukan sebanyak siklus instruksi itu.
3. **Per frame**: CPU jalan satu frame penuh, baru komponen lain menyusul.
   Terlalu kasar: interrupt timer bisa terlambat ribuan siklus.

## Keputusan

Pilihan 2, dengan aturan:

- `GameBoy::step` = `cpu.step(bus)` lalu `bus.tick(siklus)`.
- Komponen yang ingin menyela CPU **menyalakan bit di IF** (0xFF0F). CPU tidak
  tahu komponen mana yang ada; ia hanya membaca IF dan IE lewat `Bus`.
- Di awal setiap `Cpu::step`: kalau `HALT` dan tidak ada interrupt yang
  menunggu (`IF & IE`), CPU diam 4 siklus. Kalau IME menyala dan ada yang
  menunggu, CPU melompat ke penangan (20 siklus), bit terendah lebih dulu.
- Timer meniru hardware: penghitung 16-bit internal; `DIV` = 8 bit atasnya;
  `TIMA` naik pada *falling edge* bit yang dipilih `TAC`.

## Konsekuensi

- (+) CPU tetap sederhana dan tetap teruji per instruksi (ADR-0010).
- (+) CPU tidak bergantung pada timer, PPU, atau suara; semuanya lewat bus
  dan IF. Komponen baru tinggal menambah `tick` dan bit IF.
- (+) 11/11 ROM Blargg `cpu_instrs` lulus dengan model ini.
- (−) Kejadian di **tengah** instruksi tidak ditiru. Contoh: CPU membaca
  `DIV` sebelum timer dimajukan untuk instruksi itu sendiri, dan jeda 4
  siklus isi ulang TIMA tidak ada. ROM uji timing halus (mooneye) akan gagal.
- (−) "HALT bug" belum ditiru. `STOP` diperlakukan seperti `HALT`.
- Pantau: kalau PPU (M4) atau game nyata butuh ketelitian lebih, ADR baru
  bisa menggantikan ini dengan langkah per M-cycle (4 T-cycle) untuk akses
  memori, tanpa mengubah antarmuka `Bus`.
