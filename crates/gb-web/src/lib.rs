//! Adapter browser untuk `gb-core`.
//!
//! Crate ini tipis dengan sengaja: ia hanya menerjemahkan antara dunia
//! JavaScript (byte ROM, array piksel) dan API core. Logika emulasi tidak
//! boleh ditulis di sini — tempatnya di `gb-core`. Warna juga bukan urusan
//! crate ini: framebuffer dikirim sebagai tingkat kegelapan 0–3, dan
//! `web/main.js` memetakannya ke palet MXM dari CSS (ADR-0008, ADR-0012).
//!
//! Fungsi dan tipe bertanda `#[wasm_bindgen]` bisa dipakai dari `web/main.js`.

use gb_core::bus::Bus;
use gb_core::joypad::Buttons;
use gb_core::machine::GameBoy;
use wasm_bindgen::prelude::wasm_bindgen;

/// Sapaan dari core, untuk ditampilkan di halaman web.
#[wasm_bindgen]
#[must_use]
pub fn greeting() -> String {
    gb_core::greeting().into()
}

/// Versi `gb-core` yang ikut ter-build ke dalam file `.wasm` ini.
#[wasm_bindgen(js_name = coreVersion)]
#[must_use]
pub fn core_version() -> String {
    gb_core::VERSION.into()
}

/// Satu Game Boy yang berjalan di browser.
#[wasm_bindgen]
pub struct Emulator {
    gb: GameBoy,
}

#[wasm_bindgen]
impl Emulator {
    /// Memasang `rom` (isi file `.gb`). Data ROM disalin ke memori wasm;
    /// tidak pernah dikirim ke mana pun (ADR-0006).
    ///
    /// # Errors
    ///
    /// Pesan error kalau file terlalu kecil untuk menjadi ROM Game Boy atau
    /// tipe cartridge-nya belum didukung (ADR-0013).
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<Emulator, String> {
        let gb = GameBoy::new(rom.into()).map_err(|error| error.to_string())?;
        Ok(Self { gb })
    }

    /// Menjalankan satu frame (±16,7 ms waktu Game Boy).
    ///
    /// # Errors
    ///
    /// Pesan error kalau CPU bertemu opcode yang belum ada.
    #[wasm_bindgen(js_name = runFrame)]
    pub fn run_frame(&mut self) -> Result<(), String> {
        self.gb.run_frame().map_err(|error| format!("{error:?}"))
    }

    /// Tombol yang sedang ditekan, satu bit per tombol (sama dengan
    /// `gb_core::joypad::Button`): kanan 0x01, kiri 0x02, atas 0x04,
    /// bawah 0x08, A 0x10, B 0x20, Select 0x40, Start 0x80.
    ///
    /// JavaScript yang mengurus sentuhan dan keyboard; di sini hanya
    /// diteruskan ke core.
    #[wasm_bindgen(js_name = setButtons)]
    pub fn set_buttons(&mut self, mask: u8) {
        self.gb.set_buttons(Buttons(mask));
    }

    /// Judul game dari header cartridge (untuk ditampilkan dan menamai simpanan).
    #[must_use]
    pub fn title(&self) -> String {
        self.gb.bus.cartridge().header().title.clone()
    }

    /// Cartridge berbaterai: RAM-nya layak disimpan (simpan permainan).
    #[wasm_bindgen(js_name = hasBattery)]
    #[must_use]
    pub fn has_battery(&self) -> bool {
        self.gb.bus.cartridge().header().battery
    }

    /// Salinan RAM cartridge, untuk disimpan JavaScript.
    #[must_use]
    pub fn ram(&self) -> Vec<u8> {
        self.gb.bus.cartridge().ram().to_vec()
    }

    /// Memasang RAM yang pernah disimpan (dipanggil sebelum frame pertama).
    #[wasm_bindgen(js_name = loadRam)]
    pub fn load_ram(&mut self, saved: &[u8]) {
        self.gb.bus.cartridge_mut().load_ram(saved);
    }

    /// `true` sekali setiap kali RAM berubah: saatnya menyimpan.
    #[wasm_bindgen(js_name = takeRamDirty)]
    pub fn take_ram_dirty(&mut self) -> bool {
        self.gb.bus.cartridge_mut().take_ram_dirty()
    }

    /// Laju sampel suara (biasanya `AudioContext.sampleRate`, mis. 48 000).
    /// 0 = suara mati: core tidak membuat sampel sama sekali (hemat CPU HP).
    #[wasm_bindgen(js_name = setSampleRate)]
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.gb.set_sample_rate(rate);
    }

    /// Sampel suara sejak panggilan terakhir, nilai −1.0 … +1.0, **terpisah
    /// per sisi**: semua sampel kiri dulu, lalu semua sampel kanan. Menjadi
    /// `Float32Array` di JS, yang bisa langsung disalin per kanal dengan
    /// `AudioBuffer.copyToChannel` — jauh lebih cepat daripada memisahkan
    /// kiri/kanan satu per satu di JavaScript.
    #[wasm_bindgen(js_name = takeSamples)]
    pub fn take_samples(&mut self) -> Vec<f32> {
        let interleaved = self.gb.take_samples();
        let mut planar = Vec::with_capacity(interleaved.len());
        planar.extend(interleaved.iter().step_by(2));
        planar.extend(interleaved.iter().skip(1).step_by(2));
        planar
    }

    /// Register CPU untuk debugger: `[AF, BC, DE, HL, SP, PC, status]`.
    /// `status` bit 0 = interrupt menyala (IME), bit 1 = CPU sedang HALT.
    #[must_use]
    pub fn registers(&self) -> Vec<u16> {
        let cpu = &self.gb.cpu;
        let r = &cpu.regs;
        let status = u16::from(cpu.interrupts_enabled()) | u16::from(cpu.is_halted()) << 1;
        vec![r.af(), r.bc(), r.de(), r.hl(), r.sp, r.pc, status]
    }

    /// `count` instruksi mulai dari PC, masing-masing satu baris teks:
    /// `"0150  3E 3C     LD A,$3C"`.
    pub fn disassembly(&mut self, count: usize) -> Vec<String> {
        let mut address = self.gb.cpu.regs.pc;
        let mut lines = Vec::with_capacity(count);
        for _ in 0..count {
            let instruction = self.gb.disassemble_at(address);
            let bytes: Vec<String> = (0..u16::from(instruction.length))
                .map(|i| format!("{:02X}", self.gb.bus.read(address.wrapping_add(i))))
                .collect();
            lines.push(format!(
                "{address:04X}  {:<9} {}",
                bytes.join(" "),
                instruction.text
            ));
            address = address.wrapping_add(u16::from(instruction.length));
        }
        lines
    }

    /// Menjalankan tepat satu instruksi (tombol "Langkah" di debugger).
    ///
    /// # Errors
    ///
    /// Pesan error kalau CPU bertemu opcode yang belum ada.
    #[wasm_bindgen(js_name = stepInstruction)]
    pub fn step_instruction(&mut self) -> Result<(), String> {
        self.gb
            .step()
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    /// Salinan layar: 160 × 144 byte, tingkat kegelapan 0 (terang) – 3 (gelap).
    #[must_use]
    pub fn framebuffer(&self) -> Vec<u8> {
        self.gb.framebuffer().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::Emulator;

    #[test]
    fn web_shows_the_same_text_as_core() {
        assert_eq!(super::greeting(), gb_core::greeting());
        assert_eq!(super::core_version(), gb_core::VERSION);
    }

    #[test]
    fn rejects_files_too_small_to_be_a_rom() {
        let error = Emulator::new(&[0; 16]).err().unwrap_or_default();
        assert!(error.contains("terlalu kecil"), "{error}");
    }

    #[test]
    fn runs_frames_and_exposes_a_full_screen() {
        // ROM berisi NOP semua + JR -2 di 0x100: loop tanpa akhir yang aman.
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x102].copy_from_slice(&[0x18, 0xFE]);
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        for _ in 0..3 {
            emulator.run_frame().expect("tidak ada opcode yang hilang");
        }
        assert_eq!(emulator.framebuffer().len(), 160 * 144);
    }

    #[test]
    fn buttons_reach_the_joypad_register() {
        // ROM: pilih grup arah (P1 = 0x20), baca P1 ke A, lalu kirim lewat
        // serial dan berhenti di loop.
        //   LD A,0x20 / LDH (0x00),A / LDH A,(0x00) / LDH (0x01),A /
        //   LD A,0x81 / LDH (0x02),A / JR -2
        let program = [
            0x3E, 0x20, 0xE0, 0x00, 0xF0, 0x00, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0x18, 0xFE,
        ];
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x100 + program.len()].copy_from_slice(&program);
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        emulator.set_buttons(0x01 | 0x80); // kanan + Start
        emulator.run_frame().expect("tidak ada opcode yang hilang");
        // Grup arah: kanan ditekan → bit 0 bernilai 0. Start (grup aksi)
        // tidak terlihat karena grupnya tidak dipilih.
        assert_eq!(emulator.gb.bus.take_serial_byte(), Some(0xEE));
    }

    #[test]
    fn battery_ram_can_be_saved_and_restored() {
        // ROM MBC1 + RAM + baterai. Program: nyalakan RAM, tulis 0x42 ke A000,
        // lalu berputar.
        //   LD A,0x0A / LD (0x0000),A / LD A,0x42 / LD (0xA000),A / JR -2
        let program = [
            0x3E, 0x0A, 0xEA, 0x00, 0x00, 0x3E, 0x42, 0xEA, 0x00, 0xA0, 0x18, 0xFE,
        ];
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x100 + program.len()].copy_from_slice(&program);
        rom[0x134..0x139].copy_from_slice(b"SIMPN");
        rom[0x147] = 0x03;
        rom[0x149] = 0x02;
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        assert!(emulator.has_battery());
        assert_eq!(emulator.title(), "SIMPN");
        emulator.run_frame().expect("tidak ada opcode yang hilang");
        assert!(emulator.take_ram_dirty());
        let saved = emulator.ram();
        assert_eq!(saved[0], 0x42);

        let mut fresh = Emulator::new(&rom).expect("ROM cukup besar");
        fresh.load_ram(&saved);
        assert_eq!(fresh.ram()[0], 0x42);
        assert!(!fresh.take_ram_dirty(), "memuat simpanan bukan perubahan");
    }

    #[test]
    fn sound_samples_follow_the_sample_rate() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x102].copy_from_slice(&[0x18, 0xFE]);
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        emulator.run_frame().expect("tidak ada opcode yang hilang");
        assert!(emulator.take_samples().is_empty(), "bawaan: suara mati");
        emulator.set_sample_rate(48_000);
        emulator.run_frame().expect("tidak ada opcode yang hilang");
        // Satu frame ±16,74 ms × 48 kHz ≈ 803 sampel × 2 kanal.
        let samples = emulator.take_samples().len();
        assert!((1500..1700).contains(&samples), "{samples}");
    }

    #[test]
    fn samples_are_handed_over_left_then_right() {
        // Kanal 2 hanya ke kiri (NR51 = 0x20), berbunyi terus:
        //   NR52=0x80, NR51=0x20, NR22=0xF0, NR21=0x80, NR24=0x87 (trigger)
        let program = [
            0x3E, 0x80, 0xE0, 0x26, 0x3E, 0x20, 0xE0, 0x25, 0x3E, 0xF0, 0xE0, 0x17, 0x3E, 0x80,
            0xE0, 0x16, 0x3E, 0x87, 0xE0, 0x19, 0x18, 0xFE,
        ];
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x100 + program.len()].copy_from_slice(&program);
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        emulator.set_sample_rate(48_000);
        emulator.run_frame().expect("tidak ada opcode yang hilang");
        let samples = emulator.take_samples();
        let (left, right) = samples.split_at(samples.len() / 2);
        let energy = |side: &[f32]| side.iter().map(|s| s.abs()).sum::<f32>();
        assert!(energy(left) > 1.0, "paruh pertama = kiri, berbunyi");
        assert!(energy(right) < 1e-6, "paruh kedua = kanan, sunyi");
    }

    #[test]
    fn debugger_shows_registers_and_next_instructions() {
        // LD A,$3C / JR -2
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x104].copy_from_slice(&[0x3E, 0x3C, 0x18, 0xFE]);
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        assert_eq!(emulator.registers()[5], 0x0100, "PC");
        assert_eq!(
            emulator.disassembly(2),
            ["0100  3E 3C     LD A,$3C", "0102  18 FE     JR $0102"]
        );
        emulator.step_instruction().expect("opcode ada");
        let registers = emulator.registers();
        assert_eq!(registers[0] >> 8, 0x3C, "A setelah satu langkah");
        assert_eq!(registers[5], 0x0102, "PC maju 2 byte");
    }

    #[test]
    fn missing_opcode_becomes_a_readable_error() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100] = 0xD3; // opcode ilegal
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        let error = emulator.run_frame().err().unwrap_or_default();
        assert!(error.contains("Unimplemented"), "{error}");
    }
}
