//! Joypad: 8 tombol Game Boy dan register P1 (0xFF00).
//!
//! Hardware asli hanya punya 4 jalur baca untuk 8 tombol. Program memilih
//! kelompok mana yang dibaca dengan menulis bit 4–5 P1 (0 = dipilih):
//!
//! | P1 | bit 3 | bit 2 | bit 1 | bit 0 |
//! |---|---|---|---|---|
//! | bit 4 = 0 (arah) | Bawah | Atas | Kiri | Kanan |
//! | bit 5 = 0 (aksi) | Start | Select | B | A |
//!
//! Tombol yang **ditekan terbaca 0** (logika terbalik, seperti saklar yang
//! menarik jalur ke ground). Bit 6–7 selalu terbaca 1.
//!
//! Interrupt joypad (IF bit 4) diminta saat sebuah tombol berubah dari lepas
//! ke ditekan. Penyederhanaan: diminta untuk tombol mana pun, tanpa melihat
//! kelompok yang sedang dipilih P1.

use crate::state::{Reader, StateError, Writer};

/// Satu tombol Game Boy. Nilainya adalah bit di [`Buttons`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Kanan (D-pad).
    Right = 0b0000_0001,
    /// Kiri (D-pad).
    Left = 0b0000_0010,
    /// Atas (D-pad).
    Up = 0b0000_0100,
    /// Bawah (D-pad).
    Down = 0b0000_1000,
    /// Tombol A.
    A = 0b0001_0000,
    /// Tombol B.
    B = 0b0010_0000,
    /// Tombol Select.
    Select = 0b0100_0000,
    /// Tombol Start.
    Start = 0b1000_0000,
}

/// Kumpulan tombol yang sedang ditekan: satu bit per [`Button`] (1 = ditekan).
/// Bentuk sederhana ini mudah dikirim dari adapter mana pun (JavaScript,
/// keyboard, tombol fisik di STM32 kelak).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons(pub u8);

impl Buttons {
    /// Apakah `button` sedang ditekan.
    #[must_use]
    pub const fn pressed(self, button: Button) -> bool {
        self.0 & button as u8 != 0
    }
}

/// Bit interrupt joypad di IF.
pub const JOYPAD_INTERRUPT: u8 = 0b0001_0000;

/// Keadaan joypad.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Joypad {
    buttons: Buttons,
    /// Bit 4–5 terakhir yang ditulis program ke P1.
    select: u8,
}

impl Joypad {
    /// Joypad tanpa tombol ditekan dan tanpa kelompok dipilih.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buttons: Buttons(0),
            select: 0x30,
        }
    }

    /// Membaca P1 (0xFF00).
    #[must_use]
    pub const fn read(&self) -> u8 {
        let mut low = 0x0F; // 1 = tidak ditekan
        if self.select & 0x10 == 0 {
            low &= !(self.buttons.0 & 0x0F); // arah
        }
        if self.select & 0x20 == 0 {
            low &= !(self.buttons.0 >> 4); // aksi
        }
        0xC0 | self.select | low
    }

    /// Menulis P1: hanya bit 4–5 (pemilih kelompok) yang bisa diubah.
    pub const fn write(&mut self, value: u8) {
        self.select = value & 0x30;
    }

    /// Mengganti tombol yang sedang ditekan. Mengembalikan `true` kalau ada
    /// tombol yang baru ditekan (interrupt joypad harus diminta).
    pub const fn set_buttons(&mut self, buttons: Buttons) -> bool {
        let newly_pressed = buttons.0 & !self.buttons.0;
        self.buttons = buttons;
        newly_pressed != 0
    }
}

impl Joypad {
    /// Menulis keadaan joypad ke save state (ADR-0016).
    pub(crate) fn save(&self, w: &mut Writer) {
        w.u8(self.buttons.0);
        w.u8(self.select);
    }

    /// Membaca keadaan joypad dari save state.
    pub(crate) fn load(&mut self, r: &mut Reader<'_>) -> Result<(), StateError> {
        self.buttons = Buttons(r.u8()?);
        self.select = r.u8()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Buttons, Joypad};

    fn pressed(buttons: &[Button]) -> Buttons {
        Buttons(buttons.iter().fold(0, |bits, &b| bits | b as u8))
    }

    #[test]
    fn nothing_pressed_reads_all_ones() {
        let mut joypad = Joypad::new();
        for select in [0x00, 0x10, 0x20, 0x30] {
            joypad.write(select);
            assert_eq!(joypad.read() & 0x0F, 0x0F, "P1 = {select:#04X}");
        }
    }

    #[test]
    fn direction_group_reports_dpad_as_zero_bits() {
        let mut joypad = Joypad::new();
        joypad.set_buttons(pressed(&[Button::Left, Button::A]));
        joypad.write(0x20); // bit 4 = 0: pilih arah
        // Kiri = bit 1 → 0. Tombol A (kelompok lain) tidak terlihat.
        assert_eq!(joypad.read() & 0x0F, 0b1101);
    }

    #[test]
    fn action_group_reports_buttons_as_zero_bits() {
        let mut joypad = Joypad::new();
        joypad.set_buttons(pressed(&[Button::Start, Button::B, Button::Up]));
        joypad.write(0x10); // bit 5 = 0: pilih aksi
        // Start = bit 3, B = bit 1 → 0. Atas (kelompok lain) tidak terlihat.
        assert_eq!(joypad.read() & 0x0F, 0b0101);
    }

    #[test]
    fn no_group_selected_hides_every_button() {
        let mut joypad = Joypad::new();
        joypad.set_buttons(Buttons(0xFF));
        joypad.write(0x30);
        assert_eq!(joypad.read() & 0x0F, 0x0F);
    }

    #[test]
    fn upper_bits_read_as_one_and_echo_select() {
        let mut joypad = Joypad::new();
        joypad.write(0xFF);
        assert_eq!(joypad.read(), 0xFF);
        joypad.write(0x10);
        assert_eq!(joypad.read() & 0xF0, 0xD0);
    }

    #[test]
    fn interrupt_only_on_new_presses() {
        let mut joypad = Joypad::new();
        assert!(joypad.set_buttons(pressed(&[Button::A])), "A baru ditekan");
        assert!(
            !joypad.set_buttons(pressed(&[Button::A])),
            "A masih ditahan"
        );
        assert!(!joypad.set_buttons(Buttons(0)), "melepas bukan interrupt");
        assert!(joypad.set_buttons(pressed(&[Button::Down])));
    }
}
