//! Save state: seluruh keadaan mesin sebagai byte (M8, ADR-0016).
//!
//! Berbeda dengan simpan permainan (RAM cartridge, M6) yang hanya berisi apa
//! yang game sendiri simpan, save state memotret *semuanya*: register CPU,
//! RAM, VRAM, timer, PPU, APU, joypad, dan isi MBC. Memuat state = mesin
//! berada persis di titik yang sama, sampai ke siklus.
//!
//! Format (semua angka little-endian):
//!
//! ```text
//! "MXMS"  versi:u16  sidik-ROM:u32  CPU  bus  timer  PPU  APU  joypad  cartridge
//! ```
//!
//! Setiap komponen menulis dan membaca field-nya sendiri, dalam urutan yang
//! sama (`save`/`load` di modulnya masing-masing). ROM tidak ikut: hanya
//! sidiknya, supaya state dari game lain ditolak.

use alloc::vec::Vec;

/// Penanda awal berkas state.
const MAGIC: &[u8; 4] = b"MXMS";
/// Naikkan setiap kali urutan atau isi field berubah. State versi lain
/// ditolak, bukan dibaca salah.
pub const VERSION: u16 = 1;

/// Kenapa state tidak bisa dimuat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    /// Bukan berkas state mxm-gameboy.
    NotAState,
    /// Dibuat oleh versi format lain.
    Version(u16),
    /// Dibuat dari ROM lain.
    WrongRom,
    /// Berkas terpotong atau nilainya tidak masuk akal.
    Corrupt,
}

impl core::fmt::Display for StateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAState => write!(f, "Ini bukan save state mxm-gameboy."),
            Self::Version(v) => {
                write!(
                    f,
                    "Save state versi {v}; emulator ini memakai versi {VERSION}."
                )
            }
            Self::WrongRom => write!(f, "Save state ini dibuat dari ROM lain."),
            Self::Corrupt => write!(f, "Save state rusak atau terpotong."),
        }
    }
}

/// Penulis byte state.
#[derive(Default)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    /// Penulis dengan kepala berkas (penanda, versi, sidik ROM).
    #[must_use]
    pub fn new(rom_fingerprint: u32) -> Self {
        let mut writer = Self::default();
        writer.bytes.extend_from_slice(MAGIC);
        writer.u16(VERSION);
        writer.u32(rom_fingerprint);
        writer
    }

    /// Hasil akhir.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// Satu byte.
    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// `true`/`false` sebagai 1/0.
    pub fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    /// Dua byte, little-endian.
    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Empat byte, little-endian.
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Deretan byte dengan panjang di depannya (u32).
    pub fn bytes(&mut self, value: &[u8]) {
        self.u32(u32::try_from(value.len()).unwrap_or(u32::MAX));
        self.bytes.extend_from_slice(value);
    }
}

/// Pembaca byte state. Setiap pembacaan bisa gagal ([`StateError::Corrupt`])
/// kalau berkas habis lebih cepat.
pub struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    /// Memeriksa kepala berkas dan sidik ROM.
    ///
    /// # Errors
    ///
    /// [`StateError`] kalau penanda, versi, atau sidik ROM tidak cocok.
    pub fn new(bytes: &'a [u8], rom_fingerprint: u32) -> Result<Self, StateError> {
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err(StateError::NotAState);
        }
        let mut reader = Self { bytes, position: 4 };
        let version = reader.u16()?;
        if version != VERSION {
            return Err(StateError::Version(version));
        }
        if reader.u32()? != rom_fingerprint {
            return Err(StateError::WrongRom);
        }
        Ok(reader)
    }

    /// Semua byte sudah terbaca: tidak ada sisa yang aneh di ujung.
    ///
    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau masih ada byte tersisa.
    pub const fn finish(&self) -> Result<(), StateError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(StateError::Corrupt)
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], StateError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(StateError::Corrupt)?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(StateError::Corrupt)?;
        self.position = end;
        Ok(slice)
    }

    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau berkas habis.
    pub fn u8(&mut self) -> Result<u8, StateError> {
        Ok(self.take(1)?[0])
    }

    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau berkas habis atau nilainya bukan 0/1.
    pub fn bool(&mut self) -> Result<bool, StateError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StateError::Corrupt),
        }
    }

    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau berkas habis.
    pub fn u16(&mut self) -> Result<u16, StateError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau berkas habis.
    pub fn u32(&mut self) -> Result<u32, StateError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Deretan byte yang panjangnya harus persis `into.len()`.
    ///
    /// # Errors
    ///
    /// [`StateError::Corrupt`] kalau panjangnya lain atau berkas habis.
    pub fn bytes_into(&mut self, into: &mut [u8]) -> Result<(), StateError> {
        let length = usize::try_from(self.u32()?).map_err(|_| StateError::Corrupt)?;
        if length != into.len() {
            return Err(StateError::Corrupt);
        }
        into.copy_from_slice(self.take(length)?);
        Ok(())
    }
}

/// Sidik ROM: FNV-1a 32-bit seluruh isi ROM.
#[must_use]
pub fn fingerprint(rom: &[u8]) -> u32 {
    rom.iter().fold(0x811c_9dc5, |hash, &byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests;
