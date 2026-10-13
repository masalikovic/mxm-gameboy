//! Mesin Game Boy: CPU yang tersambung ke bus memori sungguhan.
//!
//! [`crate::bus::Bus`] hanyalah "colokan". Modul ini menyediakan isinya:
//! [`SystemBus`] memetakan setiap alamat 16-bit ke komponen yang benar.
//!
//! ```text
//! 0000–7FFF  ROM cartridge, bank dipilih MBC (cartridge.rs, ADR-0013)
//! 8000–9FFF  RAM video (8 KiB)
//! A000–BFFF  RAM cartridge (kalau ada dan dinyalakan; selain itu 0xFF)
//! C000–DFFF  RAM kerja (8 KiB)
//! E000–FDFF  "echo": cermin C000–DDFF
//! FE00–FE9F  OAM, data 40 sprite
//! FF00–FF7F  register I/O
//! FF80–FFFE  HRAM, RAM kecil cepat
//! FFFF       saklar interrupt (IE)
//! ```
//!
//! Komponen yang belum dibangun (suara, tombol) untuk sementara hanya
//! menyimpan nilai yang ditulis. Pengecualiannya dicatat di
//! [`SystemBus::read`].
//!
//! ## Waktu (ADR-0011)
//!
//! CPU menjalankan satu instruksi utuh, lalu [`GameBoy::step`] memajukan
//! komponen lain (timer dan PPU) sebanyak siklus yang dipakai instruksi
//! itu. Komponen yang ingin menyela CPU menyalakan bitnya di register IF.

use alloc::boxed::Box;

use crate::bus::Bus;
use crate::cartridge::{Cartridge, CartridgeError};
use crate::cpu::{Cpu, CpuError};
use crate::joypad::{Buttons, JOYPAD_INTERRUPT, Joypad};
use crate::ppu::Ppu;
use crate::timer::Timer;

/// Alamat register serial: byte data (SB) dan kontrol (SC).
const SB: u16 = 0xFF01;
const SC: u16 = 0xFF02;
/// Alamat register P1: tombol.
const P1: u16 = 0xFF00;
/// Alamat register DMA: salin 160 byte ke OAM.
const DMA: u16 = 0xFF46;
/// Alamat register IF: interrupt yang diminta.
const IF: u16 = 0xFF0F;
/// Bit interrupt timer di IF.
const TIMER_INTERRUPT: u8 = 0b0000_0100;

/// Bit yang selalu terbaca 1 di register I/O 0xFF00–0xFF7F pada DMG.
///
/// Bit yang tidak tersambung ke apa pun di chip terbaca 1. Register yang sama
/// sekali tidak ada di DMG (termasuk register khusus Game Boy Color seperti
/// KEY1 di 0xFF4D) terbaca 0xFF. Program memakai ini untuk mengenali
/// mesinnya: kalau KEY1 terbaca 0x00, program mengira berjalan di CGB.
/// Timer, PPU, joypad, dan IF mengurus bitnya sendiri; tabel ini untuk sisanya.
const IO_UNUSED_BITS: [u8; 0x80] = {
    let mut mask = [0xFF; 0x80];
    let used: [(usize, u8); 31] = [
        (0x01, 0x00), // SB
        (0x02, 0x7E), // SC: hanya bit 7 dan 0
        (0x10, 0x80), // suara kanal 1 (M7)
        (0x11, 0x3F),
        (0x12, 0x00),
        (0x13, 0xFF),
        (0x14, 0xBF),
        (0x16, 0x3F), // kanal 2
        (0x17, 0x00),
        (0x18, 0xFF),
        (0x19, 0xBF),
        (0x1A, 0x7F), // kanal 3
        (0x1B, 0xFF),
        (0x1C, 0x9F),
        (0x1D, 0xFF),
        (0x1E, 0xBF),
        (0x20, 0xFF), // kanal 4
        (0x21, 0x00),
        (0x22, 0x00),
        (0x23, 0xBF),
        (0x24, 0x00), // volume utama
        (0x25, 0x00), // panning
        (0x26, 0x70), // NR52: bit 6–4 tidak terpakai
        (0x30, 0x00), // RAM gelombang 0xFF30–0xFF3F
        (0x31, 0x00),
        (0x32, 0x00),
        (0x33, 0x00),
        (0x34, 0x00),
        (0x35, 0x00),
        (0x36, 0x00),
        (0x37, 0x00),
    ];
    let mut i = 0;
    while i < used.len() {
        mask[used[i].0] = used[i].1;
        i += 1;
    }
    let mut wave = 0x38;
    while wave <= 0x3F {
        mask[wave] = 0x00;
        wave += 1;
    }
    mask
};

/// Bus memori lengkap sebuah Game Boy DMG.
pub struct SystemBus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    timer: Timer,
    ppu: Ppu,
    joypad: Joypad,
    serial_out: Option<u8>,
}

impl SystemBus {
    /// Bus dengan `cartridge` terpasang dan semua RAM lain berisi nol.
    #[must_use]
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            wram: [0; 0x2000],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            timer: Timer::default(),
            ppu: Ppu::new(),
            joypad: Joypad::new(),
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

    /// Cartridge yang terpasang (header, RAM untuk simpan permainan).
    #[must_use]
    pub const fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    /// Cartridge yang bisa diubah, misalnya untuk memasang RAM simpanan.
    pub const fn cartridge_mut(&mut self) -> &mut Cartridge {
        &mut self.cartridge
    }

    /// PPU (layar), untuk membaca framebuffer.
    #[must_use]
    pub const fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    /// PPU yang bisa diubah, misalnya untuk mengambil tanda frame selesai.
    pub const fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }

    /// Memajukan komponen di balik bus sebanyak `cycles` T-cycle.
    pub fn tick(&mut self, cycles: u8) {
        if self.timer.tick(cycles) {
            self.request_interrupt(TIMER_INTERRUPT);
        }
        let ppu_interrupts = self.ppu.tick(cycles);
        self.request_interrupt(ppu_interrupts);
    }

    /// Mengganti tombol yang sedang ditekan; meminta interrupt joypad kalau
    /// ada tombol yang baru ditekan.
    pub fn set_buttons(&mut self, buttons: Buttons) {
        if self.joypad.set_buttons(buttons) {
            self.request_interrupt(JOYPAD_INTERRUPT);
        }
    }

    /// OAM DMA: salin 160 byte dari `page` × 0x100 ke OAM (data sprite).
    /// Di hardware asli butuh 640 siklus; di sini seketika (ADR-0012).
    fn oam_dma(&mut self, page: u8) {
        let source = u16::from(page) << 8;
        for offset in 0..0xA0 {
            let value = self.read(source + offset);
            self.ppu.write_oam(0xFE00 + offset, value);
        }
    }

    /// Menyalakan bit interrupt di IF. CPU memeriksanya di awal `step`.
    const fn request_interrupt(&mut self, bit: u8) {
        self.io[(IF - 0xFF00) as usize] |= bit;
    }
}

impl Bus for SystemBus {
    fn read(&mut self, address: u16) -> u8 {
        let a = usize::from(address);
        match address {
            0x0000..=0x7FFF => self.cartridge.read_rom(address),
            0x8000..=0x9FFF => self.ppu.read_vram(address),
            0xA000..=0xBFFF => self.cartridge.read_ram(address),
            // Area FEA0–FEFF tidak terpakai.
            0xFEA0..=0xFEFF => 0xFF,
            0xC000..=0xDFFF => self.wram[a - 0xC000],
            0xE000..=0xFDFF => self.wram[a - 0xE000],
            0xFE00..=0xFE9F => self.ppu.read_oam(address),
            P1 => self.joypad.read(),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.ppu.read_register(address),
            0xFF04..=0xFF07 => self.timer.read(address),
            // Bit 7–5 IF tidak dipakai dan terbaca 1.
            IF => self.io[a - 0xFF00] | 0xE0,
            0xFF01..=0xFF7F => self.io[a - 0xFF00] | IO_UNUSED_BITS[a - 0xFF00],
            0xFF80..=0xFFFE => self.hram[a - 0xFF80],
            0xFFFF => self.ie,
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        let a = usize::from(address);
        match address {
            // Tulisan ke area ROM adalah perintah ke chip MBC (ganti bank).
            0x0000..=0x7FFF => self.cartridge.write_rom(address, value),
            0xA000..=0xBFFF => self.cartridge.write_ram(address, value),
            0xFEA0..=0xFEFF => {}
            0x8000..=0x9FFF => self.ppu.write_vram(address, value),
            0xC000..=0xDFFF => self.wram[a - 0xC000] = value,
            0xE000..=0xFDFF => self.wram[a - 0xE000] = value,
            0xFE00..=0xFE9F => self.ppu.write_oam(address, value),
            P1 => self.joypad.write(value),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.ppu.write_register(address, value),
            DMA => {
                self.io[a - 0xFF00] = value;
                self.oam_dma(value);
            }
            // SC = 0x81: "kirim byte SB sekarang, pakai jam internal".
            // Transfer dianggap langsung selesai: byte ditampung, bit 7 SC
            // dimatikan, dan SB terisi 0xFF (tidak ada Game Boy di ujung kabel).
            SC if value == 0x81 => {
                self.serial_out = Some(self.io[usize::from(SB - 0xFF00)]);
                self.io[usize::from(SB - 0xFF00)] = 0xFF;
                self.io[usize::from(SC - 0xFF00)] = value & 0x7F;
            }
            0xFF04..=0xFF07 => {
                if self.timer.write(address, value) {
                    self.request_interrupt(TIMER_INTERRUPT);
                }
            }
            IF => self.io[a - 0xFF00] = value & 0x1F,
            0xFF01..=0xFF7F => self.io[a - 0xFF00] = value,
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
    ///
    /// # Errors
    ///
    /// [`CartridgeError`] kalau `rom` terlalu kecil atau MBC-nya belum
    /// didukung (ADR-0013).
    pub fn new(rom: Box<[u8]>) -> Result<Self, CartridgeError> {
        Ok(Self {
            cpu: Cpu::new(),
            bus: SystemBus::new(Cartridge::new(rom)?),
        })
    }

    /// Menjalankan satu instruksi. Lihat [`Cpu::step`].
    ///
    /// # Errors
    ///
    /// [`CpuError`] kalau CPU bertemu opcode yang belum ditulis.
    pub fn step(&mut self) -> Result<u8, CpuError> {
        let cycles = self.cpu.step(&mut self.bus)?;
        self.bus.tick(cycles);
        Ok(cycles)
    }

    /// Menjalankan instruksi sampai satu frame selesai digambar (awal
    /// V-blank). Kalau LCD mati, berhenti setelah waktu satu frame
    /// (70.224 siklus) supaya pemanggil tidak menunggu selamanya.
    ///
    /// # Errors
    ///
    /// [`CpuError`] kalau CPU bertemu opcode yang belum ditulis.
    pub fn run_frame(&mut self) -> Result<(), CpuError> {
        let mut cycles: u32 = 0;
        while cycles < 70_224 * 2 {
            cycles += u32::from(self.step()?);
            if self.bus.ppu_mut().take_frame_ready() {
                return Ok(());
            }
            if !self.bus.ppu().lcd_enabled() && cycles >= 70_224 {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Tombol yang sedang ditekan, dari adapter (sentuh, keyboard, tombol fisik).
    pub fn set_buttons(&mut self, buttons: Buttons) {
        self.bus.set_buttons(buttons);
    }

    /// Layar terakhir: 160×144 tingkat kegelapan 0 (terang) – 3 (gelap).
    #[must_use]
    pub fn framebuffer(&self) -> &[u8] {
        self.bus.ppu().framebuffer()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{Bus, Cartridge, GameBoy, SystemBus};

    fn bus() -> SystemBus {
        // ROM berisi 0xAB, kecuali header: tanpa MBC, tanpa RAM.
        let mut rom = vec![0xAB; 0x8000];
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;
        SystemBus::new(Cartridge::new(rom.into_boxed_slice()).expect("ROM tes valid"))
    }

    #[test]
    fn unused_io_bits_read_as_one() {
        let mut bus = bus();
        // KEY1 (0xFF4D) hanya ada di Game Boy Color. Di DMG terbaca 0xFF;
        // kalau terbaca 0x00, program Blargg mengira mesinnya CGB dan macet.
        bus.write(0xFF4D, 0x01);
        assert_eq!(bus.read(0xFF4D), 0xFF);
        assert_eq!(bus.read(0xFF03), 0xFF);
        // SC: hanya bit 7 dan 0 yang ada.
        bus.write(0xFF02, 0x00);
        assert_eq!(bus.read(0xFF02), 0x7E);
        // Byte yang dipakai penuh tetap terbaca apa adanya.
        bus.write(0xFF01, 0x00);
        assert_eq!(bus.read(0xFF01), 0x00);
        bus.write(0xFF30, 0x12);
        assert_eq!(bus.read(0xFF30), 0x12, "RAM gelombang suara");
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
    fn if_register_keeps_low_5_bits_and_reads_upper_bits_as_one() {
        let mut bus = bus();
        bus.write(0xFF0F, 0xFF);
        assert_eq!(bus.read(0xFF0F), 0xFF);
        bus.write(0xFF0F, 0x00);
        assert_eq!(bus.read(0xFF0F), 0xE0);
    }

    #[test]
    fn timer_overflow_requests_interrupt_through_the_bus() {
        let mut bus = bus();
        bus.write(0xFF05, 0xFF); // TIMA
        bus.write(0xFF07, 0b101); // naik setiap 16 siklus
        bus.tick(12);
        assert_eq!(bus.read(0xFF0F) & 0b100, 0);
        bus.tick(4);
        assert_eq!(bus.read(0xFF0F) & 0b100, 0b100);
    }

    #[test]
    fn timer_interrupt_reaches_the_cpu_and_halt_wakes_up() {
        // 0x0050 (penangan timer): INC B ; RETI
        // 0x0100: LD A,0x04 ; LDH (FF),A  (IE = timer)
        //         LD A,0xFF ; LDH (05),A  (TIMA = 0xFF, hampir meluap)
        //         LD A,0x05 ; LDH (07),A  (timer aktif, 16 siklus)
        //         EI ; HALT ; INC C
        let mut rom = vec![0u8; 0x8000];
        rom[0x50..0x52].copy_from_slice(&[0x04, 0xD9]);
        rom[0x100..0x10F].copy_from_slice(&[
            0x3E, 0x04, 0xE0, 0xFF, 0x3E, 0xFF, 0xE0, 0x05, 0x3E, 0x05, 0xE0, 0x07, 0xFB, 0x76,
            0x0C,
        ]);
        let mut gb = GameBoy::new(rom.into_boxed_slice()).expect("ROM tes valid");
        gb.cpu.regs.b = 0;
        gb.cpu.regs.c = 0;
        // Cukup banyak langkah untuk: siapkan, tidur, timer meluap,
        // penangan jalan (INC B), RETI, lalu INC C setelah HALT.
        for _ in 0..40 {
            gb.step().expect("opcode harus ada");
            if gb.cpu.regs.c == 1 {
                break;
            }
        }
        assert_eq!(gb.cpu.regs.b, 1, "penangan timer dijalankan sekali");
        assert_eq!(gb.cpu.regs.c, 1, "CPU bangun dari HALT dan lanjut");
        assert!(!gb.cpu.is_halted());
    }

    #[test]
    fn oam_dma_copies_160_bytes() {
        let mut bus = bus();
        for i in 0..0xA0u16 {
            bus.write(0xC100 + i, u8::try_from(i).unwrap_or(0));
        }
        bus.write(0xFF46, 0xC1); // salin dari 0xC100
        assert_eq!(bus.read(0xFE00), 0x00);
        assert_eq!(bus.read(0xFE9F), 0x9F);
    }

    #[test]
    fn ly_is_real_now() {
        // Stub M2 (LY selalu 0x90) sudah diganti PPU: LY mulai dari 0 dan naik.
        let mut bus = bus();
        assert_eq!(bus.read(0xFF44), 0);
        for _ in 0..(456 / 4) {
            bus.tick(4);
        }
        assert_eq!(bus.read(0xFF44), 1);
    }

    #[test]
    fn vblank_interrupt_reaches_if() {
        let mut bus = bus();
        for _ in 0..(144 * 456 / 4) {
            bus.tick(4);
        }
        assert_eq!(bus.read(0xFF0F) & 0b1, 0b1);
    }

    #[test]
    fn button_press_is_visible_in_p1_and_requests_interrupt() {
        use crate::joypad::{Button, Buttons};
        let mut bus = bus();
        bus.write(0xFF00, 0x10); // pilih tombol aksi
        assert_eq!(bus.read(0xFF00) & 0x0F, 0x0F);
        bus.set_buttons(Buttons(Button::A as u8));
        assert_eq!(bus.read(0xFF00) & 0x0F, 0x0E, "A = bit 0 → 0");
        assert_eq!(bus.read(0xFF0F) & 0x10, 0x10, "interrupt joypad diminta");
    }

    #[test]
    fn program_prints_through_serial() {
        // LD A,'H' ; LDH (01),A ; LD A,0x81 ; LDH (02),A
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x108].copy_from_slice(&[0x3E, b'H', 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02]);
        let mut gb = GameBoy::new(rom.into_boxed_slice()).expect("ROM tes valid");
        for _ in 0..4 {
            gb.step().expect("opcode harus ada");
        }
        assert_eq!(gb.bus.take_serial_byte(), Some(b'H'));
    }
}
