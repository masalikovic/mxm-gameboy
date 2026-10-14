//! Timer DMG: `DIV`, `TIMA`, `TMA`, `TAC` (alamat 0xFF04–0xFF07).
//!
//! Di dalam chip ada satu penghitung 16-bit yang bertambah setiap T-cycle.
//!
//! - **DIV** (0xFF04) adalah 8 bit atas penghitung itu, jadi naik setiap
//!   256 siklus (16.384 kali per detik). Menulis apa pun ke DIV mengosongkan
//!   seluruh penghitung.
//! - **TIMA** (0xFF05) naik setiap kali satu bit tertentu penghitung berubah
//!   dari 1 ke 0 ("falling edge"). Bit mana yang diamati dipilih **TAC**:
//!
//!   | TAC bit 1–0 | bit penghitung | TIMA naik setiap | frekuensi |
//!   |---|---|---|---|
//!   | 00 | 9 | 1024 siklus | 4.096 Hz |
//!   | 01 | 3 | 16 siklus | 262.144 Hz |
//!   | 10 | 5 | 64 siklus | 65.536 Hz |
//!   | 11 | 7 | 256 siklus | 16.384 Hz |
//!
//!   TAC bit 2 menyalakan/mematikan TIMA.
//! - Saat TIMA meluap (0xFF → 0x00), TIMA diisi ulang dengan **TMA**
//!   (0xFF06) dan interrupt timer diminta.
//!
//! Penyederhanaan yang disadari: di hardware asli isi ulang dari TMA dan
//! permintaan interrupt terjadi 4 siklus setelah luapan. Di sini keduanya
//! seketika. ROM uji yang mengukur jeda 4 siklus itu (mooneye) belum target.

/// Timer DMG.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timer {
    counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
}

impl Timer {
    /// Pencacah 16-bit internal. Bus memakainya untuk frame sequencer APU
    /// (bit 12 jatuh 1 → 0 = satu detak 512 Hz, ADR-0015).
    #[must_use]
    pub const fn counter(&self) -> u16 {
        self.counter
    }

    /// Membaca register timer di `address` (0xFF04–0xFF07).
    #[must_use]
    pub const fn read(&self, address: u16) -> u8 {
        match address {
            0xFF04 => self.counter.to_be_bytes()[0],
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            // Bit 7–3 TAC tidak dipakai dan terbaca 1.
            _ => self.tac | 0xF8,
        }
    }

    /// Menulis register timer. Mengembalikan `true` kalau tulisan itu
    /// membuat TIMA meluap (lihat [`Timer::tick`]).
    pub fn write(&mut self, address: u16, value: u8) -> bool {
        match address {
            0xFF04 => {
                // Mengosongkan penghitung bisa membuat bit yang diamati jatuh
                // 1 → 0, dan itu menaikkan TIMA, persis seperti hardware.
                let before = self.watched_bit();
                self.counter = 0;
                return before && !self.watched_bit() && self.increment_tima();
            }
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            _ => self.tac = value & 0b111,
        }
        false
    }

    /// Memajukan timer sebanyak `cycles` T-cycle. Mengembalikan `true` kalau
    /// TIMA meluap sedikitnya sekali (interrupt timer harus diminta).
    pub fn tick(&mut self, cycles: u8) -> bool {
        let mut overflow = false;
        for _ in 0..cycles {
            let before = self.watched_bit();
            self.counter = self.counter.wrapping_add(1);
            if before && !self.watched_bit() {
                overflow |= self.increment_tima();
            }
        }
        overflow
    }

    /// Nilai bit penghitung yang sedang diamati, digabung dengan saklar TAC.
    /// Kalau timer mati, nilainya selalu `false`, jadi tidak ada "jatuh".
    const fn watched_bit(&self) -> bool {
        let bit = match self.tac & 0b11 {
            0 => 9,
            1 => 3,
            2 => 5,
            _ => 7,
        };
        self.tac & 0b100 != 0 && self.counter & (1 << bit) != 0
    }

    /// TIMA + 1. Saat meluap: isi ulang dari TMA dan kembalikan `true`.
    const fn increment_tima(&mut self) -> bool {
        let (value, overflow) = self.tima.overflowing_add(1);
        self.tima = if overflow { self.tma } else { value };
        overflow
    }
}

#[cfg(test)]
mod tests {
    use super::Timer;

    /// Memajukan timer `cycles` siklus (boleh lebih dari 255).
    fn run(timer: &mut Timer, mut cycles: u32) -> bool {
        let mut overflow = false;
        while cycles > 0 {
            let step = u8::try_from(cycles.min(4)).unwrap_or(4);
            overflow |= timer.tick(step);
            cycles -= u32::from(step);
        }
        overflow
    }

    #[test]
    fn div_rises_every_256_cycles() {
        let mut timer = Timer::default();
        run(&mut timer, 255);
        assert_eq!(timer.read(0xFF04), 0);
        run(&mut timer, 1);
        assert_eq!(timer.read(0xFF04), 1);
        run(&mut timer, 256 * 9);
        assert_eq!(timer.read(0xFF04), 10);
    }

    #[test]
    fn writing_div_resets_it() {
        let mut timer = Timer::default();
        run(&mut timer, 1000);
        timer.write(0xFF04, 0x55); // nilainya tidak penting
        assert_eq!(timer.read(0xFF04), 0);
        run(&mut timer, 255);
        assert_eq!(timer.read(0xFF04), 0);
    }

    #[test]
    fn tima_rate_follows_tac() {
        // (TAC, siklus per kenaikan TIMA)
        for (tac, period) in [(0b100, 1024), (0b101, 16), (0b110, 64), (0b111, 256)] {
            let mut timer = Timer::default();
            timer.write(0xFF07, tac);
            run(&mut timer, period - 4);
            assert_eq!(timer.read(0xFF05), 0, "TAC {tac:#05b}: belum naik");
            run(&mut timer, 4);
            assert_eq!(timer.read(0xFF05), 1, "TAC {tac:#05b}: naik tepat waktu");
            run(&mut timer, period * 3);
            assert_eq!(timer.read(0xFF05), 4, "TAC {tac:#05b}");
        }
    }

    #[test]
    fn tima_stays_still_when_disabled() {
        let mut timer = Timer::default();
        timer.write(0xFF07, 0b001); // frekuensi dipilih, tetapi bit 2 mati
        run(&mut timer, 10_000);
        assert_eq!(timer.read(0xFF05), 0);
    }

    #[test]
    fn overflow_reloads_tma_and_requests_interrupt() {
        let mut timer = Timer::default();
        timer.write(0xFF06, 0xAB); // TMA
        timer.write(0xFF05, 0xFE); // TIMA
        timer.write(0xFF07, 0b101); // naik setiap 16 siklus
        assert!(!run(&mut timer, 16)); // 0xFE → 0xFF: belum meluap
        assert_eq!(timer.read(0xFF05), 0xFF);
        assert!(run(&mut timer, 16)); // 0xFF → luap → TMA
        assert_eq!(timer.read(0xFF05), 0xAB);
    }

    #[test]
    fn unused_tac_bits_read_as_one() {
        let mut timer = Timer::default();
        timer.write(0xFF07, 0xFF);
        assert_eq!(timer.read(0xFF07), 0xFF);
        timer.write(0xFF07, 0x00);
        assert_eq!(timer.read(0xFF07), 0xF8);
    }

    #[test]
    fn writing_div_can_bump_tima() {
        // Bit 3 penghitung menyala (counter = 8), timer 262 kHz aktif.
        // Mengosongkan DIV membuat bit 3 jatuh 1 → 0, jadi TIMA naik.
        let mut timer = Timer::default();
        timer.write(0xFF07, 0b101);
        run(&mut timer, 8);
        assert_eq!(timer.read(0xFF05), 0);
        timer.write(0xFF04, 0);
        assert_eq!(timer.read(0xFF05), 1);
    }
}
