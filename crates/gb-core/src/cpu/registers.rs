//! Register CPU SM83.
//!
//! SM83 punya delapan register 8-bit: A, F, B, C, D, E, H, L, ditambah dua
//! register 16-bit: SP (stack pointer) dan PC (program counter). Register 8-bit
//! juga bisa dipakai berpasangan sebagai 16-bit: AF, BC, DE, HL.
//!
//! F adalah register **flag**. Hanya 4 bit atasnya yang dipakai:
//!
//! ```text
//! bit  7 6 5 4 3 2 1 0
//!      Z N H C 0 0 0 0
//! ```
//!
//! - **Z** (zero): hasil operasi terakhir nol.
//! - **N** (subtract): operasi terakhir adalah pengurangan.
//! - **H** (half carry): ada carry/borrow dari bit 3 ke bit 4.
//! - **C** (carry): ada carry/borrow dari bit 7 (atau bit 15 untuk 16-bit).
//!
//! Empat bit bawah F selalu 0 di hardware asli. [`Registers::set_f`] menjaga
//! aturan itu, jadi tidak ada kode lain yang perlu mengingatnya.

/// Satu flag di register F, disimpan sebagai posisi bitnya.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    /// Z, bit 7: hasil nol.
    Zero = 0b1000_0000,
    /// N, bit 6: operasi pengurangan.
    Subtract = 0b0100_0000,
    /// H, bit 5: carry dari bit 3 ke bit 4.
    HalfCarry = 0b0010_0000,
    /// C, bit 4: carry dari bit 7.
    Carry = 0b0001_0000,
}

/// Semua register SM83.
///
/// Field 8-bit dibuat publik supaya tes dan debugger (M8) mudah membacanya.
/// Hanya F yang privat, karena 4 bit bawahnya harus selalu 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    /// Akumulator: tempat hasil sebagian besar operasi aritmetika.
    pub a: u8,
    f: u8,
    /// Register B.
    pub b: u8,
    /// Register C.
    pub c: u8,
    /// Register D.
    pub d: u8,
    /// Register E.
    pub e: u8,
    /// Register H (byte atas pasangan HL, sering dipakai sebagai alamat).
    pub h: u8,
    /// Register L (byte bawah pasangan HL).
    pub l: u8,
    /// Stack pointer.
    pub sp: u16,
    /// Program counter: alamat instruksi berikutnya.
    pub pc: u16,
}

impl Registers {
    /// Isi register F.
    #[must_use]
    pub const fn f(&self) -> u8 {
        self.f
    }

    /// Mengisi register F. Empat bit bawah selalu dibuang.
    pub const fn set_f(&mut self, value: u8) {
        self.f = value & 0xF0;
    }

    /// Apakah `flag` sedang menyala.
    #[must_use]
    pub const fn flag(&self, flag: Flag) -> bool {
        self.f & flag as u8 != 0
    }

    /// Menyalakan atau mematikan satu flag.
    pub const fn set_flag(&mut self, flag: Flag, on: bool) {
        if on {
            self.f |= flag as u8;
        } else {
            self.f &= !(flag as u8);
        }
    }

    /// Pasangan AF (A byte atas, F byte bawah).
    #[must_use]
    pub const fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    /// Mengisi pasangan AF. Empat bit bawah F tetap dibuang.
    pub const fn set_af(&mut self, value: u16) {
        let [hi, lo] = value.to_be_bytes();
        self.a = hi;
        self.set_f(lo);
    }

    /// Pasangan BC.
    #[must_use]
    pub const fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }

    /// Mengisi pasangan BC.
    pub const fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }

    /// Pasangan DE.
    #[must_use]
    pub const fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }

    /// Mengisi pasangan DE.
    pub const fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }

    /// Pasangan HL.
    #[must_use]
    pub const fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }

    /// Mengisi pasangan HL.
    pub const fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::{Flag, Registers};

    #[test]
    fn pairs_combine_high_and_low_bytes() {
        let mut r = Registers::default();
        r.set_bc(0x1234);
        assert_eq!((r.b, r.c), (0x12, 0x34));
        assert_eq!(r.bc(), 0x1234);

        r.d = 0xAB;
        r.e = 0xCD;
        assert_eq!(r.de(), 0xABCD);

        r.set_hl(0xC000);
        assert_eq!((r.h, r.l), (0xC0, 0x00));
    }

    #[test]
    fn low_nibble_of_f_is_always_zero() {
        let mut r = Registers::default();
        r.set_f(0xFF);
        assert_eq!(r.f(), 0xF0);

        r.set_af(0x12FF);
        assert_eq!(r.a, 0x12);
        assert_eq!(r.f(), 0xF0);
        assert_eq!(r.af(), 0x12F0);
    }

    #[test]
    fn flags_are_independent_bits() {
        let mut r = Registers::default();
        r.set_flag(Flag::Zero, true);
        r.set_flag(Flag::Carry, true);
        assert_eq!(r.f(), 0b1001_0000);
        assert!(r.flag(Flag::Zero));
        assert!(!r.flag(Flag::Subtract));

        r.set_flag(Flag::Zero, false);
        assert_eq!(r.f(), 0b0001_0000);
    }
}
