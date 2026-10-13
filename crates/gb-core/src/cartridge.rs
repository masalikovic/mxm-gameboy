//! Cartridge: ROM, RAM cartridge, dan chip MBC (ADR-0013).
//!
//! CPU hanya melihat 32 KiB ROM dan 8 KiB RAM cartridge sekaligus:
//!
//! ```text
//! 0000–3FFF  ROM bank 0 (MBC1 mode 1: bisa bank lain)
//! 4000–7FFF  ROM bank yang dipilih MBC
//! A000–BFFF  RAM cartridge, bank yang dipilih MBC
//! ```
//!
//! Game yang lebih besar membawa MBC (*memory bank controller*). Menulis ke
//! area ROM tidak mengubah ROM — itu perintah ke MBC: nyalakan RAM, pilih
//! bank ROM, pilih bank RAM. Tipe MBC dibaca dari header di 0x147.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Ukuran satu bank ROM (16 KiB) dan satu bank RAM (8 KiB).
const ROM_BANK: usize = 0x4000;
const RAM_BANK: usize = 0x2000;
/// MBC2 membawa RAM sendiri: 512 sel × 4 bit.
const MBC2_RAM: usize = 512;

/// Kenapa ROM tidak bisa dipasang.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CartridgeError {
    /// File lebih kecil dari header cartridge (berakhir di 0x014F).
    TooSmall(usize),
    /// Tipe cartridge (byte 0x147) yang belum didukung.
    Unsupported(u8),
}

impl core::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooSmall(size) => write!(
                f,
                "File terlalu kecil untuk ROM Game Boy ({size} byte, minimal 336)."
            ),
            Self::Unsupported(kind) => {
                write!(f, "Tipe cartridge 0x{kind:02X} belum didukung.")
            }
        }
    }
}

/// Isi header cartridge (0x0134–0x014F) yang kita pakai.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// Judul game, huruf ASCII yang bisa dicetak saja.
    pub title: String,
    /// Byte tipe cartridge (0x147): menentukan MBC, RAM, dan baterai.
    pub kind: u8,
    /// Ukuran RAM cartridge menurut header, dalam byte.
    pub ram_size: usize,
    /// Ada baterai: isi RAM bertahan saat Game Boy dimatikan (simpan permainan).
    pub battery: bool,
    /// Checksum header (0x14D) cocok. Game Boy asli menolak boot kalau salah;
    /// kita tetap menjalankannya, tetapi adapter bisa memperingatkan.
    pub checksum_ok: bool,
}

impl Header {
    /// Membaca header dari `rom`. `rom` harus minimal 0x150 byte.
    fn parse(rom: &[u8]) -> Self {
        let title = rom[0x134..0x144]
            .iter()
            .take_while(|&&byte| byte != 0)
            .filter(|byte| byte.is_ascii_graphic() || **byte == b' ')
            .map(|&byte| char::from(byte))
            .collect();
        let ram_size = match rom[0x149] {
            2 => RAM_BANK,
            3 => 4 * RAM_BANK,
            4 => 16 * RAM_BANK,
            5 => 8 * RAM_BANK,
            _ => 0,
        };
        let checksum = rom[0x134..0x14D]
            .iter()
            .fold(0u8, |sum, &byte| sum.wrapping_sub(byte).wrapping_sub(1));
        let kind = rom[0x147];
        Self {
            title,
            kind,
            ram_size,
            battery: matches!(kind, 0x03 | 0x06 | 0x09 | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E),
            checksum_ok: checksum == rom[0x14D],
        }
    }
}

/// Chip MBC dan register-registernya.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mbc {
    /// Tanpa MBC: ROM 32 KiB, RAM opsional (tipe 0x08/0x09).
    None,
    /// MBC1: bank ROM 5 bit + 2 bit "atas" yang juga bisa memilih bank RAM.
    Mbc1 {
        /// RAM cartridge boleh dibaca/ditulis (0x0A ditulis ke 0000–1FFF).
        ram_enabled: bool,
        /// BANK1: 5 bit bawah nomor bank ROM (0 dibaca sebagai 1).
        bank1: u8,
        /// BANK2: 2 bit, bit 5–6 nomor bank ROM atau nomor bank RAM.
        bank2: u8,
        /// Mode 1: BANK2 juga berlaku untuk 0000–3FFF dan RAM.
        mode: bool,
    },
    /// MBC2: bank ROM 4 bit, RAM 512 × 4 bit di dalam chip.
    Mbc2 {
        /// RAM di dalam chip boleh dibaca/ditulis.
        ram_enabled: bool,
        /// Bank ROM di 4000–7FFF (4 bit, 0 dibaca 1).
        rom_bank: u8,
    },
    /// MBC3: bank ROM 7 bit, bank RAM 0–3 atau register jam 08–0C.
    Mbc3 {
        /// RAM dan register jam boleh dibaca/ditulis.
        ram_enabled: bool,
        /// Bank ROM di 4000–7FFF (7 bit, 0 dibaca 1).
        rom_bank: u8,
        /// 0–3: bank RAM; 0x08–0x0C: register jam (RTC).
        ram_select: u8,
        /// Register jam: detik, menit, jam, hari (bawah), hari (atas + flag).
        /// Waktu tidak maju (ADR-0013).
        rtc: [u8; 5],
    },
    /// MBC5: bank ROM 9 bit (bank 0 boleh), bank RAM 4 bit.
    Mbc5 {
        /// RAM cartridge boleh dibaca/ditulis.
        ram_enabled: bool,
        /// Bank ROM di 4000–7FFF (9 bit).
        rom_bank: u16,
        /// Bank RAM di A000–BFFF (4 bit).
        ram_bank: u8,
    },
}

/// Cartridge yang terpasang: ROM, RAM, dan MBC-nya.
#[derive(Clone, Debug)]
pub struct Cartridge {
    rom: Box<[u8]>,
    ram: Vec<u8>,
    header: Header,
    mbc: Mbc,
    /// Jumlah bank ROM, selalu pangkat dua (untuk mask nomor bank).
    rom_banks: usize,
}

impl Cartridge {
    /// Memasang `rom`.
    ///
    /// # Errors
    ///
    /// [`CartridgeError`] kalau file terlalu kecil atau tipe MBC-nya belum
    /// didukung.
    pub fn new(rom: Box<[u8]>) -> Result<Self, CartridgeError> {
        if rom.len() < 0x150 {
            return Err(CartridgeError::TooSmall(rom.len()));
        }
        let header = Header::parse(&rom);
        let mbc = match header.kind {
            0x00 | 0x08 | 0x09 => Mbc::None,
            0x01..=0x03 => Mbc::Mbc1 {
                ram_enabled: false,
                bank1: 1,
                bank2: 0,
                mode: false,
            },
            0x05 | 0x06 => Mbc::Mbc2 {
                ram_enabled: false,
                rom_bank: 1,
            },
            0x0F..=0x13 => Mbc::Mbc3 {
                ram_enabled: false,
                rom_bank: 1,
                ram_select: 0,
                rtc: [0; 5],
            },
            0x19..=0x1E => Mbc::Mbc5 {
                ram_enabled: false,
                rom_bank: 1,
                ram_bank: 0,
            },
            kind => return Err(CartridgeError::Unsupported(kind)),
        };
        let ram_size = if matches!(mbc, Mbc::Mbc2 { .. }) {
            MBC2_RAM
        } else {
            header.ram_size
        };
        // Ukuran file, bukan header, yang menentukan jumlah bank (ADR-0013).
        let rom_banks = rom.len().div_ceil(ROM_BANK).next_power_of_two().max(2);
        Ok(Self {
            rom,
            ram: vec![0; ram_size],
            header,
            mbc,
            rom_banks,
        })
    }

    /// Header cartridge.
    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Keadaan MBC saat ini (untuk tes dan debugger).
    #[must_use]
    pub const fn mbc(&self) -> &Mbc {
        &self.mbc
    }

    /// Isi RAM cartridge, untuk disimpan adapter (simpan permainan).
    #[must_use]
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    /// Memasang kembali RAM yang pernah disimpan. Kalau ukurannya berbeda
    /// (file simpanan dari ROM lain), hanya bagian yang muat yang disalin.
    pub fn load_ram(&mut self, saved: &[u8]) {
        let n = saved.len().min(self.ram.len());
        self.ram[..n].copy_from_slice(&saved[..n]);
    }

    /// Membaca 0000–7FFF.
    #[must_use]
    pub fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            self.low_rom_bank()
        } else {
            self.high_rom_bank()
        };
        let bank = bank & (self.rom_banks - 1);
        let offset = usize::from(address) & (ROM_BANK - 1);
        self.rom
            .get(bank * ROM_BANK + offset)
            .copied()
            .unwrap_or(0xFF)
    }

    /// Bank yang terlihat di 0000–3FFF.
    fn low_rom_bank(&self) -> usize {
        match self.mbc {
            Mbc::Mbc1 {
                bank2, mode: true, ..
            } => usize::from(bank2) << 5,
            _ => 0,
        }
    }

    /// Bank yang terlihat di 4000–7FFF.
    fn high_rom_bank(&self) -> usize {
        match self.mbc {
            Mbc::None => 1,
            Mbc::Mbc1 { bank1, bank2, .. } => usize::from(bank2) << 5 | usize::from(bank1),
            Mbc::Mbc2 { rom_bank, .. } | Mbc::Mbc3 { rom_bank, .. } => usize::from(rom_bank),
            Mbc::Mbc5 { rom_bank, .. } => usize::from(rom_bank),
        }
    }

    /// Menulis ke 0000–7FFF: perintah untuk MBC.
    pub fn write_rom(&mut self, address: u16, value: u8) {
        // Di semua MBC, 0x0A di 4 bit bawah menyalakan RAM; nilai lain mematikan.
        let enable = value & 0x0F == 0x0A;
        match &mut self.mbc {
            Mbc::None => {}
            Mbc::Mbc1 {
                ram_enabled,
                bank1,
                bank2,
                mode,
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = enable,
                // Nol diganti satu SEBELUM di-mask ke ukuran ROM: bank 0x20
                // tidak bisa dipilih lewat BANK1 = 0 (ciri khas MBC1).
                0x2000..=0x3FFF => *bank1 = (value & 0x1F).max(1),
                0x4000..=0x5FFF => *bank2 = value & 0x03,
                _ => *mode = value & 0x01 != 0,
            },
            Mbc::Mbc2 {
                ram_enabled,
                rom_bank,
            } => {
                // Hanya 0000–3FFF. Bit 8 alamat memilih: 0 = RAM, 1 = bank ROM.
                if address < 0x4000 {
                    if address & 0x0100 == 0 {
                        *ram_enabled = enable;
                    } else {
                        *rom_bank = (value & 0x0F).max(1);
                    }
                }
            }
            Mbc::Mbc3 {
                ram_enabled,
                rom_bank,
                ram_select,
                ..
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = enable,
                0x2000..=0x3FFF => *rom_bank = (value & 0x7F).max(1),
                0x4000..=0x5FFF => *ram_select = value,
                // 6000–7FFF: "latch" jam. Jam kita beku, jadi tidak ada efek.
                _ => {}
            },
            Mbc::Mbc5 {
                ram_enabled,
                rom_bank,
                ram_bank,
            } => match address {
                0x0000..=0x1FFF => *ram_enabled = enable,
                0x2000..=0x2FFF => *rom_bank = (*rom_bank & 0x100) | u16::from(value),
                0x3000..=0x3FFF => {
                    *rom_bank = (*rom_bank & 0xFF) | (u16::from(value & 0x01) << 8);
                }
                0x4000..=0x5FFF => *ram_bank = value & 0x0F,
                _ => {}
            },
        }
    }

    /// Lokasi di `ram` untuk alamat A000–BFFF, atau `None` kalau RAM mati
    /// atau tidak ada.
    fn ram_index(&self, address: u16) -> Option<usize> {
        let offset = usize::from(address - 0xA000);
        let (enabled, bank) = match self.mbc {
            Mbc::None => (true, 0),
            Mbc::Mbc1 {
                ram_enabled,
                bank2,
                mode,
                ..
            } => (ram_enabled, if mode { usize::from(bank2) } else { 0 }),
            // RAM MBC2 hanya 512 sel; alamat lain di A000–BFFF adalah cerminnya.
            Mbc::Mbc2 { ram_enabled, .. } => {
                return ram_enabled.then_some(offset & (MBC2_RAM - 1));
            }
            Mbc::Mbc3 {
                ram_enabled,
                ram_select,
                ..
            } => (ram_enabled && ram_select <= 3, usize::from(ram_select)),
            Mbc::Mbc5 {
                ram_enabled,
                ram_bank,
                ..
            } => (ram_enabled, usize::from(ram_bank)),
        };
        if !enabled || self.ram.is_empty() {
            return None;
        }
        // Mask ke ukuran RAM: bank yang tidak ada mencerminkan bank yang ada.
        Some((bank * RAM_BANK + offset) % self.ram.len())
    }

    /// Indeks register jam MBC3 yang sedang dipilih, kalau ada.
    fn rtc_register(&self) -> Option<usize> {
        match self.mbc {
            Mbc::Mbc3 {
                ram_enabled: true,
                ram_select: select @ 0x08..=0x0C,
                ..
            } => Some(usize::from(select - 0x08)),
            _ => None,
        }
    }

    /// Membaca A000–BFFF. RAM mati atau tidak ada terbaca 0xFF.
    #[must_use]
    pub fn read_ram(&self, address: u16) -> u8 {
        if let Some(register) = self.rtc_register()
            && let Mbc::Mbc3 { rtc, .. } = &self.mbc
        {
            return rtc[register];
        }
        let Some(index) = self.ram_index(address) else {
            return 0xFF;
        };
        let value = self.ram[index];
        // Sel MBC2 hanya 4 bit; 4 bit atas terbaca 1.
        if matches!(self.mbc, Mbc::Mbc2 { .. }) {
            value | 0xF0
        } else {
            value
        }
    }

    /// Menulis A000–BFFF. Diabaikan kalau RAM mati atau tidak ada.
    pub fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(register) = self.rtc_register() {
            if let Mbc::Mbc3 { rtc, .. } = &mut self.mbc {
                rtc[register] = value;
            }
            return;
        }
        if let Some(index) = self.ram_index(address) {
            self.ram[index] = value;
        }
    }
}

#[cfg(test)]
mod tests;
