//! CPU Sharp SM83, otak Game Boy.
//!
//! Satu panggilan [`Cpu::step`] menjalankan satu instruksi:
//!
//! 1. **Fetch**: baca byte di alamat PC, lalu PC maju.
//! 2. **Decode**: cari tahu instruksi apa dari pola bit byte itu.
//! 3. **Execute**: kerjakan, ubah register, flag, atau memori.
//!
//! Hasilnya adalah jumlah **T-cycle** yang dipakai instruksi itu (4,19 juta
//! per detik). Komponen lain (timer, PPU) kelak dimajukan sebanyak angka ini,
//! sehingga semuanya tetap sinkron (ADR-0010).
//!
//! ## Pola bit opcode
//!
//! Banyak opcode SM83 tersusun teratur. Untuk byte `0b_xx_yyy_zzz`:
//!
//! ```text
//!  x   = bit 7–6 : kelompok besar
//!  yyy = bit 5–3 : sering = register tujuan
//!  zzz = bit 2–0 : sering = register sumber
//! ```
//!
//! dan angka register 0–7 berarti `B C D E H L (HL) A`. Contoh: `0x41` =
//! `0b01_000_001` → kelompok 1 (LD), tujuan 0 (B), sumber 1 (C) → `LD B,C`.
//! Karena itu satu baris kode bisa menangani puluhan opcode sekaligus.

pub mod registers;

use crate::bus::Bus;
pub use registers::{Flag, Registers};

/// Alasan [`Cpu::step`] tidak bisa menjalankan instruksi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuError {
    /// Opcode ini belum ditulis di emulator (M1 sedang berjalan).
    /// PC dikembalikan ke alamat opcode, jadi keadaan CPU tidak setengah jalan.
    Unimplemented {
        /// Byte opcode yang belum dikenal.
        opcode: u8,
        /// Alamat tempat opcode itu dibaca.
        address: u16,
    },
}

/// CPU SM83.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpu {
    /// Semua register.
    pub regs: Registers,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// CPU dalam keadaan **setelah boot ROM** selesai.
    ///
    /// Game Boy asli menjalankan boot ROM Nintendo (logo bergulir) lalu
    /// melompat ke `0x0100`. Kita tidak memakai boot ROM itu (ADR-0006), jadi
    /// CPU langsung dimulai dengan nilai register yang ditinggalkan boot ROM
    /// pada DMG, sesuai dokumentasi publik (Pan Docs, "Power Up Sequence").
    #[must_use]
    pub fn new() -> Self {
        // F privat (4 bit bawahnya harus 0), jadi register diisi satu per satu.
        let mut regs = Registers::default();
        regs.a = 0x01;
        regs.set_f(0xB0);
        regs.b = 0x00;
        regs.c = 0x13;
        regs.d = 0x00;
        regs.e = 0xD8;
        regs.h = 0x01;
        regs.l = 0x4D;
        regs.sp = 0xFFFE;
        regs.pc = 0x0100;
        Self { regs }
    }

    /// Menjalankan satu instruksi dan mengembalikan jumlah T-cycle-nya.
    ///
    /// # Errors
    ///
    /// [`CpuError::Unimplemented`] kalau opcode belum ditulis. Register tidak
    /// berubah dalam kasus itu.
    pub fn step(&mut self, bus: &mut impl Bus) -> Result<u8, CpuError> {
        let address = self.regs.pc;
        let opcode = self.fetch(bus);

        // Dua bit teratas opcode (x) memilih satu dari empat kelompok besar.
        let cycles = match opcode >> 6 {
            0 => self.execute_block0(bus, opcode),
            1 => self.execute_block1(bus, opcode),
            // Kelompok 2 seluruhnya ALU A,r: 8 operasi × 8 sumber. yyy memilih
            // operasinya (ADD ADC SUB SBC AND XOR OR CP), zzz sumbernya.
            2 => {
                let z = opcode & 0b111;
                let value = self.read_r(bus, z);
                self.alu((opcode >> 3) & 0b111, value);
                Some(if z == 6 { 8 } else { 4 })
            }
            _ => self.execute_block3(bus, opcode),
        };
        cycles.ok_or_else(|| self.unimplemented(opcode, address))
    }

    /// Kelompok 0 (0x00–0x3F): NOP, LD 16-bit, INC/DEC, LD r,d8, JR.
    fn execute_block0(&mut self, bus: &mut impl Bus, opcode: u8) -> Option<u8> {
        let y = (opcode >> 3) & 0b111;
        let cycles = match opcode {
            // NOP: tidak melakukan apa-apa.
            0x00 => 4,

            // LD rr,d16: isi pasangan register dengan angka 16-bit.
            0x01 | 0x11 | 0x21 | 0x31 => {
                let value = self.fetch_u16(bus);
                self.set_rp(opcode >> 4, value);
                12
            }

            // INC r: tambah 1. Flag C tidak berubah.
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let old = self.read_r(bus, y);
                let new = old.wrapping_add(1);
                self.write_r(bus, y, new);
                self.regs.set_flag(Flag::Zero, new == 0);
                self.regs.set_flag(Flag::Subtract, false);
                self.regs.set_flag(Flag::HalfCarry, old & 0x0F == 0x0F);
                if y == 6 { 12 } else { 4 }
            }

            // DEC r: kurang 1. Flag C tidak berubah.
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let old = self.read_r(bus, y);
                let new = old.wrapping_sub(1);
                self.write_r(bus, y, new);
                self.regs.set_flag(Flag::Zero, new == 0);
                self.regs.set_flag(Flag::Subtract, true);
                // Bentuk `& 0x0F == 0` sengaja dipertahankan: sama dengan cara
                // dokumentasi hardware menulisnya ("4 bit bawah semuanya 0").
                #[expect(
                    clippy::verbose_bit_mask,
                    reason = "sama dengan notasi dokumentasi hardware"
                )]
                self.regs.set_flag(Flag::HalfCarry, old & 0x0F == 0x00);
                if y == 6 { 12 } else { 4 }
            }

            // LD r,d8: isi register dengan angka 8-bit berikutnya.
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let value = self.fetch(bus);
                self.write_r(bus, y, value);
                if y == 6 { 12 } else { 8 }
            }

            // JR e8: lompat relatif, -128..+127 dari alamat SETELAH instruksi.
            0x18 => {
                let offset = self.fetch_i8(bus);
                self.jump_relative(offset);
                12
            }

            // JR cc,e8: lompat relatif hanya kalau syaratnya terpenuhi.
            // Lompat butuh lebih banyak siklus daripada tidak lompat.
            0x20 | 0x28 | 0x30 | 0x38 => {
                let offset = self.fetch_i8(bus);
                if self.condition(y - 4) {
                    self.jump_relative(offset);
                    12
                } else {
                    8
                }
            }

            // LD (BC),A … LD A,(HL-): memori lewat alamat di pasangan register.
            0x02 | 0x12 | 0x22 | 0x32 | 0x0A | 0x1A | 0x2A | 0x3A => {
                self.load_indirect(bus, opcode);
                8
            }

            // INC rr / DEC rr: 16-bit, tidak menyentuh flag sama sekali.
            0x03 | 0x13 | 0x23 | 0x33 => {
                let p = opcode >> 4;
                self.set_rp(p, self.rp(p).wrapping_add(1));
                8
            }
            0x0B | 0x1B | 0x2B | 0x3B => {
                let p = opcode >> 4;
                self.set_rp(p, self.rp(p).wrapping_sub(1));
                8
            }

            // ADD HL,rr: penjumlahan 16-bit. Flag Z tidak berubah.
            0x09 | 0x19 | 0x29 | 0x39 => {
                self.add_hl(self.rp(opcode >> 4));
                8
            }

            // LD (a16),SP: simpan SP ke memori, byte rendah dulu.
            0x08 => {
                let address = self.fetch_u16(bus);
                let [lo, hi] = self.regs.sp.to_le_bytes();
                bus.write(address, lo);
                bus.write(address.wrapping_add(1), hi);
                20
            }

            // RLCA RRCA RLA RRA DAA CPL SCF CCF: operasi khusus A dan flag.
            0x07 | 0x0F | 0x17 | 0x1F | 0x27 | 0x2F | 0x37 | 0x3F => {
                self.accumulator_op(y);
                4
            }

            _ => return None,
        };
        Some(cycles)
    }

    /// Kelompok 1 (0x40–0x7F): LD r,r' dan HALT.
    fn execute_block1(&mut self, bus: &mut impl Bus, opcode: u8) -> Option<u8> {
        let y = (opcode >> 3) & 0b111;
        let z = opcode & 0b111;
        let cycles = match opcode {
            // HALT ada di tengah blok LD (0x76 akan menjadi "LD (HL),(HL)").
            // Ia butuh interrupt, jadi ditulis di M3.
            0x76 => return None,

            // LD r,r': salin register sumber ke register tujuan.
            0x40..=0x7F => {
                let value = self.read_r(bus, z);
                self.write_r(bus, y, value);
                if y == 6 || z == 6 { 8 } else { 4 }
            }

            _ => return None,
        };
        Some(cycles)
    }

    /// Kelompok 3 (0xC0–0xFF): ALU A,d8, JP, CALL, RET, PUSH, POP.
    fn execute_block3(&mut self, bus: &mut impl Bus, opcode: u8) -> Option<u8> {
        let y = (opcode >> 3) & 0b111;
        let cycles = match opcode {
            // ALU A,d8: operasi yang sama dengan angka 8-bit berikutnya.
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let value = self.fetch(bus);
                self.alu(y, value);
                8
            }

            // JP a16: lompat ke alamat 16-bit.
            0xC3 => {
                self.regs.pc = self.fetch_u16(bus);
                16
            }

            // JP cc,a16.
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                let target = self.fetch_u16(bus);
                if self.condition(y) {
                    self.regs.pc = target;
                    16
                } else {
                    12
                }
            }

            // JP HL: lompat ke alamat yang ada di HL (bukan ke memori di HL).
            0xE9 => {
                self.regs.pc = self.regs.hl();
                4
            }

            // CALL a16: simpan alamat kembali di stack, lalu lompat.
            0xCD => {
                let target = self.fetch_u16(bus);
                self.push(bus, self.regs.pc);
                self.regs.pc = target;
                24
            }

            // CALL cc,a16.
            0xC4 | 0xCC | 0xD4 | 0xDC => {
                let target = self.fetch_u16(bus);
                if self.condition(y) {
                    self.push(bus, self.regs.pc);
                    self.regs.pc = target;
                    24
                } else {
                    12
                }
            }

            // RET: ambil alamat kembali dari stack.
            0xC9 => {
                self.regs.pc = self.pop(bus);
                16
            }

            // RET cc.
            0xC0 | 0xC8 | 0xD0 | 0xD8 => {
                if self.condition(y) {
                    self.regs.pc = self.pop(bus);
                    20
                } else {
                    8
                }
            }

            // PUSH rr: urutan pasangan BC DE HL AF (bit 5–4 opcode).
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                let value = match opcode {
                    0xC5 => self.regs.bc(),
                    0xD5 => self.regs.de(),
                    0xE5 => self.regs.hl(),
                    _ => self.regs.af(),
                };
                self.push(bus, value);
                16
            }

            // POP rr. POP AF tetap membuang 4 bit bawah F (lihat set_af).
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let value = self.pop(bus);
                match opcode {
                    0xC1 => self.regs.set_bc(value),
                    0xD1 => self.regs.set_de(value),
                    0xE1 => self.regs.set_hl(value),
                    _ => self.regs.set_af(value),
                }
                12
            }

            // RST n: CALL ke salah satu dari 8 alamat tetap (0x00, 0x08, … 0x38).
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                self.push(bus, self.regs.pc);
                self.regs.pc = u16::from(y) * 8;
                16
            }

            // LDH dan LD lewat alamat 16-bit langsung.
            0xE0 | 0xF0 | 0xE2 | 0xF2 | 0xEA | 0xFA => self.load_high_or_absolute(bus, opcode),

            // ADD SP,e8 dan LD HL,SP+e8: SP ditambah angka bertanda.
            0xE8 => {
                self.regs.sp = self.sp_plus_e8(bus);
                16
            }
            0xF8 => {
                let value = self.sp_plus_e8(bus);
                self.regs.set_hl(value);
                12
            }

            // LD SP,HL.
            0xF9 => {
                self.regs.sp = self.regs.hl();
                8
            }

            _ => return None,
        };
        Some(cycles)
    }

    /// Membaca byte di PC lalu memajukan PC.
    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let value = bus.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        value
    }

    /// Membaca angka 16-bit di PC. SM83 little-endian: byte rendah dulu.
    fn fetch_u16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        u16::from_le_bytes([lo, hi])
    }

    /// Membaca register nomor `index` (`B C D E H L (HL) A`).
    /// Nomor 6 bukan register, melainkan memori di alamat HL.
    fn read_r(&self, bus: &mut impl Bus, index: u8) -> u8 {
        match index {
            0 => self.regs.b,
            1 => self.regs.c,
            2 => self.regs.d,
            3 => self.regs.e,
            4 => self.regs.h,
            5 => self.regs.l,
            6 => bus.read(self.regs.hl()),
            _ => self.regs.a,
        }
    }

    /// Menulis register nomor `index` (`B C D E H L (HL) A`).
    fn write_r(&mut self, bus: &mut impl Bus, index: u8, value: u8) {
        match index {
            0 => self.regs.b = value,
            1 => self.regs.c = value,
            2 => self.regs.d = value,
            3 => self.regs.e = value,
            4 => self.regs.h = value,
            5 => self.regs.l = value,
            6 => bus.write(self.regs.hl(), value),
            _ => self.regs.a = value,
        }
    }

    /// Membaca byte di PC sebagai angka bertanda (-128..=127).
    fn fetch_i8(&mut self, bus: &mut impl Bus) -> i8 {
        i8::from_le_bytes([self.fetch(bus)])
    }

    /// PC = PC + `offset` (PC sudah menunjuk ke instruksi berikutnya).
    const fn jump_relative(&mut self, offset: i8) {
        self.regs.pc = self.regs.pc.wrapping_add_signed(offset as i16);
    }

    /// Syarat nomor `cc` (0–3): NZ, Z, NC, C.
    const fn condition(&self, cc: u8) -> bool {
        match cc {
            0 => !self.regs.flag(Flag::Zero),
            1 => self.regs.flag(Flag::Zero),
            2 => !self.regs.flag(Flag::Carry),
            _ => self.regs.flag(Flag::Carry),
        }
    }

    /// Menaruh angka 16-bit di stack. Stack tumbuh ke **bawah**:
    /// SP dikurangi dulu, byte tinggi ditulis di alamat yang lebih tinggi.
    fn push(&mut self, bus: &mut impl Bus, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write(self.regs.sp, hi);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write(self.regs.sp, lo);
    }

    /// Mengambil angka 16-bit dari stack (kebalikan [`Cpu::push`]).
    fn pop(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = bus.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let hi = bus.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    /// Satu operasi ALU dengan A dan `value`. Nomor `op` (0–7):
    /// ADD, ADC, SUB, SBC, AND, XOR, OR, CP.
    fn alu(&mut self, op: u8, value: u8) {
        let a = self.regs.a;
        let carry_in = u8::from(self.regs.flag(Flag::Carry));
        // (hasil, N, H, C). Z selalu "hasil == 0", jadi dihitung di bawah.
        let (result, subtract, half, carry) = match op {
            // ADD / ADC: carry_in hanya ikut di ADC.
            0 | 1 => {
                let c = if op == 1 { carry_in } else { 0 };
                let sum = u16::from(a) + u16::from(value) + u16::from(c);
                let half = (a & 0x0F) + (value & 0x0F) + c > 0x0F;
                (
                    a.wrapping_add(value).wrapping_add(c),
                    false,
                    half,
                    sum > 0xFF,
                )
            }
            // SUB / SBC / CP: "pinjam" dari bit 4 (H) dan bit 8 (C).
            2 | 3 | 7 => {
                let c = if op == 3 { carry_in } else { 0 };
                let half = (a & 0x0F) < (value & 0x0F) + c;
                let borrow = u16::from(a) < u16::from(value) + u16::from(c);
                (a.wrapping_sub(value).wrapping_sub(c), true, half, borrow)
            }
            // AND menyalakan H; ini memang perilaku SM83, bukan salah ketik.
            4 => (a & value, false, true, false),
            5 => (a ^ value, false, false, false),
            _ => (a | value, false, false, false),
        };
        // CP = SUB tanpa menyimpan hasil: hanya flag yang berubah.
        if op != 7 {
            self.regs.a = result;
        }
        self.regs.set_flag(Flag::Zero, result == 0);
        self.regs.set_flag(Flag::Subtract, subtract);
        self.regs.set_flag(Flag::HalfCarry, half);
        self.regs.set_flag(Flag::Carry, carry);
    }

    /// Pasangan 16-bit nomor `p` (bit 5–4 opcode): BC, DE, HL, SP.
    const fn rp(&self, p: u8) -> u16 {
        match p & 0b11 {
            0 => self.regs.bc(),
            1 => self.regs.de(),
            2 => self.regs.hl(),
            _ => self.regs.sp,
        }
    }

    /// Mengisi pasangan 16-bit nomor `p`: BC, DE, HL, SP.
    const fn set_rp(&mut self, p: u8, value: u16) {
        match p & 0b11 {
            0 => self.regs.set_bc(value),
            1 => self.regs.set_de(value),
            2 => self.regs.set_hl(value),
            _ => self.regs.sp = value,
        }
    }

    /// `LD (rr),A` atau `LD A,(rr)` untuk 0x02/0x12/0x22/0x32 dan 0x0A/…/0x3A.
    ///
    /// Bit 5–4 memilih alamat: BC, DE, HL lalu HL+1 (`HL+`), HL lalu HL−1
    /// (`HL-`). Bit 3 memilih arah: 0 = tulis A ke memori, 1 = baca ke A.
    fn load_indirect(&mut self, bus: &mut impl Bus, opcode: u8) {
        let address = match opcode >> 4 {
            0 => self.regs.bc(),
            1 => self.regs.de(),
            _ => self.regs.hl(),
        };
        match opcode >> 4 {
            2 => self.regs.set_hl(address.wrapping_add(1)),
            3 => self.regs.set_hl(address.wrapping_sub(1)),
            _ => {}
        }
        if opcode & 0x08 == 0 {
            bus.write(address, self.regs.a);
        } else {
            self.regs.a = bus.read(address);
        }
    }

    /// `LDH (a8),A`, `LDH A,(a8)`, `LD (C),A`, `LD A,(C)`, `LD (a16),A`,
    /// `LD A,(a16)`. Mengembalikan jumlah siklus.
    ///
    /// LDH ("load high") hanya menyebut byte rendah alamat; byte tingginya
    /// selalu 0xFF. Area 0xFF00–0xFFFF berisi register hardware dan HRAM,
    /// jadi instruksi pendek ini sangat sering dipakai game.
    fn load_high_or_absolute(&mut self, bus: &mut impl Bus, opcode: u8) -> u8 {
        let (address, cycles) = match opcode {
            0xE0 | 0xF0 => (0xFF00 | u16::from(self.fetch(bus)), 12),
            0xE2 | 0xF2 => (0xFF00 | u16::from(self.regs.c), 8),
            _ => (self.fetch_u16(bus), 16),
        };
        // Bit 4: 0 (0xE_) = tulis A ke memori, 1 (0xF_) = baca ke A.
        if opcode & 0x10 == 0 {
            bus.write(address, self.regs.a);
        } else {
            self.regs.a = bus.read(address);
        }
        cycles
    }

    /// HL = HL + `value`. H dari bit 11, C dari bit 15, Z tidak berubah.
    fn add_hl(&mut self, value: u16) {
        let hl = self.regs.hl();
        let (result, carry) = hl.overflowing_add(value);
        self.regs.set_hl(result);
        self.regs.set_flag(Flag::Subtract, false);
        self.regs
            .set_flag(Flag::HalfCarry, (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF);
        self.regs.set_flag(Flag::Carry, carry);
    }

    /// SP + angka bertanda berikutnya, untuk `ADD SP,e8` dan `LD HL,SP+e8`.
    ///
    /// Keanehan SM83: flag H dan C dihitung seolah-olah byte rendah SP
    /// ditambah byte offset **tanpa tanda**, walaupun offsetnya negatif.
    /// Z dan N selalu 0.
    fn sp_plus_e8(&mut self, bus: &mut impl Bus) -> u16 {
        let byte = self.fetch(bus);
        let sp = self.regs.sp;
        let low = u16::from(byte);
        self.regs.set_flag(Flag::Zero, false);
        self.regs.set_flag(Flag::Subtract, false);
        self.regs
            .set_flag(Flag::HalfCarry, (sp & 0x0F) + (low & 0x0F) > 0x0F);
        self.regs.set_flag(Flag::Carry, (sp & 0xFF) + low > 0xFF);
        sp.wrapping_add_signed(i16::from(i8::from_le_bytes([byte])))
    }

    /// Operasi khusus A nomor `op` (bit 5–3 opcode):
    /// RLCA, RRCA, RLA, RRA, DAA, CPL, SCF, CCF.
    fn accumulator_op(&mut self, op: u8) {
        let a = self.regs.a;
        let carry = self.regs.flag(Flag::Carry);
        if op < 4 {
            // Rotasi: bit yang "jatuh" dari ujung masuk ke flag C.
            // RLCA/RRCA memutar 8 bit; RLA/RRA memutar 9 bit lewat C.
            let (result, out) = match op {
                0 => (a.rotate_left(1), a >> 7),
                1 => (a.rotate_right(1), a & 1),
                2 => ((a << 1) | u8::from(carry), a >> 7),
                _ => ((a >> 1) | (u8::from(carry) << 7), a & 1),
            };
            self.regs.a = result;
            // Versi "A" selalu mematikan Z, berbeda dengan versi CB (M2).
            self.regs.set_flag(Flag::Zero, false);
            self.regs.set_flag(Flag::Subtract, false);
            self.regs.set_flag(Flag::HalfCarry, false);
            self.regs.set_flag(Flag::Carry, out == 1);
            return;
        }
        match op {
            4 => self.daa(),
            // CPL: balik semua bit A.
            5 => {
                self.regs.a = !a;
                self.regs.set_flag(Flag::Subtract, true);
                self.regs.set_flag(Flag::HalfCarry, true);
            }
            // SCF: nyalakan carry. CCF: balik carry.
            _ => {
                self.regs.set_flag(Flag::Subtract, false);
                self.regs.set_flag(Flag::HalfCarry, false);
                self.regs.set_flag(Flag::Carry, op == 6 || !carry);
            }
        }
    }

    /// DAA: koreksi A menjadi BCD (dua digit desimal, satu per 4 bit)
    /// setelah ADD/SUB. Contoh: 0x15 + 0x27 = 0x3C, lalu DAA → 0x42.
    ///
    /// Arah koreksi bergantung pada N (operasi terakhir tambah atau kurang),
    /// besarnya bergantung pada H, C, dan nilai A.
    fn daa(&mut self) {
        let mut a = self.regs.a;
        let mut carry = self.regs.flag(Flag::Carry);
        let half = self.regs.flag(Flag::HalfCarry);
        if self.regs.flag(Flag::Subtract) {
            if carry {
                a = a.wrapping_sub(0x60);
            }
            if half {
                a = a.wrapping_sub(0x06);
            }
        } else {
            if carry || a > 0x99 {
                a = a.wrapping_add(0x60);
                carry = true;
            }
            if half || a & 0x0F > 0x09 {
                a = a.wrapping_add(0x06);
            }
        }
        self.regs.a = a;
        self.regs.set_flag(Flag::Zero, a == 0);
        self.regs.set_flag(Flag::HalfCarry, false);
        self.regs.set_flag(Flag::Carry, carry);
    }

    /// Mengembalikan PC ke alamat opcode dan membuat error-nya.
    const fn unimplemented(&mut self, opcode: u8, address: u16) -> CpuError {
        self.regs.pc = address;
        CpuError::Unimplemented { opcode, address }
    }
}

#[cfg(test)]
mod tests;
