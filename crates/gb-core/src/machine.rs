//! Mesin Game Boy: CPU yang tersambung ke bus memori sungguhan.
//!
//! [`crate::bus::Bus`] hanyalah "colokan". Modul ini menyediakan isinya:
//! [`SystemBus`] memetakan setiap alamat 16-bit ke komponen yang benar.
//!
//! ```text
//! 0000–7FFF  ROM cartridge (32 KiB; bank switching menyusul di M6)
//! 8000–9FFF  RAM video (8 KiB)
//! A000–BFFF  RAM cartridge (belum ada → baca 0xFF)
//! C000–DFFF  RAM kerja (8 KiB)
//! E000–FDFF  "echo": cermin C000–DDFF
//! FE00–FE9F  OAM, data 40 sprite
//! FF00–FF7F  register I/O
//! FF80–FFFE  HRAM, RAM kecil cepat
//! FFFF       saklar interrupt (IE)
//! ```
//!
//! Komponen yang belum dibangun (timer, PPU, suara) untuk sementara hanya
//! menyimpan nilai yang ditulis. Pengecualiannya dicatat di
//! [`SystemBus::read`].

use alloc::boxed::Box;

use crate::bus::Bus;
use crate::cpu::{Cpu, CpuError};

/// Alamat register serial: byte data (SB) dan kontrol (SC).
const SB: u16 = 0xFF01;
const SC: u16 = 0xFF02;
/// Alamat register LY: baris layar yang sedang digambar.
const LY: u16 = 0xFF44;

/// Bus memori lengkap sebuah Game Boy DMG.
pub struct SystemBus {
    rom: Box<[u8]>,
    vram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    serial_out: Option<u8>,
}

impl SystemBus {
    /// Bus dengan `rom` terpasang di 0000–7FFF dan semua RAM berisi nol.
    #[must_use]
    pub fn new(rom: Box<[u8]>) -> Self {
        Self {
            rom,
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            serial_out: None,
        }
    }

    /// Mengambil byte yang baru dikirim lewat port serial, kalau ada.
    ///
    /// Game Boy asli mengirim byte ini lewat kabel link ke Game Boy lain.
    /// Kita tidak punya kabel, jadi byte-nya ditampung di sini dan diambil
    /// adapter. ROM uji Blargg memakai jalur ini untuk "mencetak" hasilnya.
    pub const fn take_serial_byte(&mut self) -> Option<u8> {
        self.serial_out.take()
    }
}

impl Bus for SystemBus {
    fn read(&mut self, address: u16) -> u8 {
        let a = usize::from(address);
        match address {
            0x0000..=0x7FFF => self.rom.get(a).copied().unwrap_or(0xFF),
            0x8000..=0x9FFF => self.vram[a - 0x8000],
            // Tidak ada RAM cartridge (M6) dan area FEA0–FEFF tidak terpakai.
            0xA000..=0xBFFF | 0xFEA0..=0xFEFF => 0xFF,
            0xC000..=0xDFFF => self.wram[a - 0xC000],
            0xE000..=0xFDFF => self.wram[a - 0xE000],
            0xFE00..=0xFE9F => self.oam[a - 0xFE00],
            // Sementara, sampai PPU ada (M4): LY selalu 0x90 (baris 144,
            // awal VBlank). Program yang menunggu VBlank tidak macet.
            LY => 0x90,
            0xFF00..=0xFF7F => self.io[a - 0xFF00],
            0xFF80..=0xFFFE => self.hram[a - 0xFF80],
            0xFFFF => self.ie,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        let a = usize::from(address);
        match address {
            // Tulisan ke area ROM adalah perintah ke chip MBC (ganti bank).
            // ROM 32 KiB tidak butuh itu; MBC menyusul di M6.
            0x0000..=0x7FFF | 0xA000..=0xBFFF | 0xFEA0..=0xFEFF => {}
            0x8000..=0x9FFF => self.vram[a - 0x8000] = value,
            0xC000..=0xDFFF => self.wram[a - 0xC000] = value,
            0xE000..=0xFDFF => self.wram[a - 0xE000] = value,
            0xFE00..=0xFE9F => self.oam[a - 0xFE00] = value,
            // SC = 0x81: "kirim byte SB sekarang, pakai jam internal".
            // Transfer dianggap langsung selesai: byte ditampung, bit 7 SC
            // dimatikan, dan SB terisi 0xFF (tidak ada Game Boy di ujung kabel).
            SC if value == 0x81 => {
                self.serial_out = Some(self.io[usize::from(SB - 0xFF00)]);
                self.io[usize::from(SB - 0xFF00)] = 0xFF;
                self.io[usize::from(SC - 0xFF00)] = value & 0x7F;
            }
            0xFF00..=0xFF7F => self.io[a - 0xFF00] = value,
            0xFF80..=0xFFFE => self.hram[a - 0xFF80] = value,
            0xFFFF => self.ie = value,
        }
    }
}

/// Satu Game Boy: CPU + bus.
pub struct GameBoy {
    /// CPU SM83.
    pub cpu: Cpu,
    /// Bus memori dan semua komponen di baliknya.
    pub bus: SystemBus,
}

impl GameBoy {
    /// Game Boy dengan `rom` terpasang, dalam keadaan setelah boot ROM.
    #[must_use]
    pub fn new(rom: Box<[u8]>) -> Self {
        Self {
            cpu: Cpu::new(),
            bus: SystemBus::new(rom),
        }
    }

    /// Menjalankan satu instruksi. Lihat [`Cpu::step`].
    ///
    /// # Errors
    ///
    /// [`CpuError`] kalau CPU bertemu opcode yang belum ditulis.
    pub fn step(&mut self) -> Result<u8, CpuError> {
        self.cpu.step(&mut self.bus)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{Bus, GameBoy, SystemBus};

    fn bus() -> SystemBus {
        SystemBus::new(vec![0xAB; 0x8000].into_boxed_slice())
    }

    #[test]
    fn rom_is_read_only() {
        let mut bus = bus();
        bus.write(0x1234, 0x00);
        assert_eq!(bus.read(0x1234), 0xAB);
    }

    #[test]
    fn echo_ram_mirrors_work_ram() {
        let mut bus = bus();
        bus.write(0xC010, 0x42);
        assert_eq!(bus.read(0xE010), 0x42);
        bus.write(0xE020, 0x99);
        assert_eq!(bus.read(0xC020), 0x99);
    }

    #[test]
    fn each_ram_region_keeps_its_own_bytes() {
        let mut bus = bus();
        for (address, value) in [
            (0x8000, 1),
            (0xC000, 2),
            (0xFE00, 3),
            (0xFF80, 4),
            (0xFFFF, 5),
        ] {
            bus.write(address, value);
        }
        for (address, value) in [
            (0x8000, 1),
            (0xC000, 2),
            (0xFE00, 3),
            (0xFF80, 4),
            (0xFFFF, 5),
        ] {
            assert_eq!(bus.read(address), value, "alamat {address:#06X}");
        }
    }

    #[test]
    fn missing_cartridge_ram_reads_ff() {
        let mut bus = bus();
        bus.write(0xA000, 0x12);
        assert_eq!(bus.read(0xA000), 0xFF);
    }

    #[test]
    fn serial_transfer_hands_the_byte_to_the_adapter() {
        let mut bus = bus();
        bus.write(0xFF01, b'P');
        assert_eq!(bus.take_serial_byte(), None); // belum dikirim
        bus.write(0xFF02, 0x81);
        assert_eq!(bus.take_serial_byte(), Some(b'P'));
        assert_eq!(bus.take_serial_byte(), None); // hanya sekali
        assert_eq!(bus.read(0xFF02) & 0x80, 0); // transfer selesai
    }

    #[test]
    fn program_prints_through_serial() {
        // LD A,'H' ; LDH (01),A ; LD A,0x81 ; LDH (02),A
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x108].copy_from_slice(&[0x3E, b'H', 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02]);
        let mut gb = GameBoy::new(rom.into_boxed_slice());
        for _ in 0..4 {
            gb.step().expect("opcode harus ada");
        }
        assert_eq!(gb.bus.take_serial_byte(), Some(b'H'));
    }
}
