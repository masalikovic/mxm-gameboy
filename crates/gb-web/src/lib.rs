//! Adapter browser untuk `gb-core`.
//!
//! Crate ini tipis dengan sengaja: ia hanya menerjemahkan antara dunia
//! JavaScript (byte ROM, array piksel) dan API core. Logika emulasi tidak
//! boleh ditulis di sini — tempatnya di `gb-core`. Warna juga bukan urusan
//! crate ini: framebuffer dikirim sebagai tingkat kegelapan 0–3, dan
//! `web/main.js` memetakannya ke palet MXM dari CSS (ADR-0008, ADR-0012).
//!
//! Fungsi dan tipe bertanda `#[wasm_bindgen]` bisa dipakai dari `web/main.js`.

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
    fn missing_opcode_becomes_a_readable_error() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100] = 0xD3; // opcode ilegal
        let mut emulator = Emulator::new(&rom).expect("ROM cukup besar");
        let error = emulator.run_frame().err().unwrap_or_default();
        assert!(error.contains("Unimplemented"), "{error}");
    }
}
