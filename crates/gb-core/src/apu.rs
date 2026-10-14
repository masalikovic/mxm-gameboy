//! APU: chip suara Game Boy (ADR-0015).
//!
//! Empat kanal, masing-masing menghasilkan angka 0–15 setiap saat:
//!
//! | Kanal | Bentuk | Register | Fitur |
//! |---|---|---|---|
//! | 1 | gelombang kotak | NR10–NR14 | duty, length, envelope, **sweep** frekuensi |
//! | 2 | gelombang kotak | NR21–NR24 | duty, length, envelope |
//! | 3 | gelombang bebas | NR30–NR34 + RAM 0xFF30–0xFF3F | length, volume 0/100/50/25 % |
//! | 4 | noise (LFSR) | NR41–NR44 | length, envelope, lebar 15/7 bit |
//!
//! **Frame sequencer** (512 Hz, dipicu bit 12 pencacah DIV lewat bus) memberi
//! "detak lambat": langkah 0, 2, 4, 6 memajukan *length*; langkah 2 dan 6
//! memajukan *sweep*; langkah 7 memajukan *envelope*.
//!
//! Angka 0–15 tiap kanal masuk DAC (−1.0 … +1.0), dipilih kiri/kanan oleh
//! NR51, dikali volume NR50, lalu disaring *high-pass*. Hasilnya sampel
//! stereo `f32` pada laju yang dipasang adapter ([`Apu::set_sample_rate`]).

use crate::state::{Reader, StateError, Writer};
use alloc::vec::Vec;

/// Frekuensi jam CPU, satuan waktu core.
const CPU_HZ: u32 = 4_194_304;

/// Pola duty kanal kotak: 12,5 %, 25 %, 50 %, 75 % (bit 7 = langkah 0).
const DUTY: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

/// Pembagi kanal noise menurut NR43 bit 2–0.
const NOISE_DIVISOR: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

/// Penghitung panjang bunyi: kanal berhenti saat mencapai 0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Length {
    enabled: bool,
    counter: u16,
}

impl Length {
    /// Satu detak frame sequencer. `true` kalau kanal harus berhenti.
    const fn clock(&mut self) -> bool {
        if self.enabled && self.counter > 0 {
            self.counter -= 1;
            return self.counter == 0;
        }
        false
    }
}

/// Envelope volume: naik/turun satu langkah setiap `period` detak.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Envelope {
    /// Isi register `NRx2` (volume awal, arah, periode).
    register: u8,
    volume: u8,
    timer: u8,
}

impl Envelope {
    const fn period(&self) -> u8 {
        self.register & 0x07
    }

    /// DAC menyala kalau 5 bit atas `NRx2` tidak semuanya 0.
    const fn dac_enabled(&self) -> bool {
        self.register & 0xF8 != 0
    }

    const fn trigger(&mut self) {
        self.volume = self.register >> 4;
        self.timer = if self.period() == 0 { 8 } else { self.period() };
    }

    const fn clock(&mut self) {
        if self.period() == 0 {
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            if self.register & 0x08 != 0 && self.volume < 15 {
                self.volume += 1;
            } else if self.register & 0x08 == 0 && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
}

/// Sweep frekuensi kanal 1 (NR10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Sweep {
    register: u8,
    enabled: bool,
    shadow: u16,
    timer: u8,
    /// Sudah ada perhitungan dengan arah turun sejak trigger: mengganti arah
    /// ke naik setelah ini mematikan kanal (perilaku hardware).
    negate_used: bool,
}

impl Sweep {
    const fn period(&self) -> u8 {
        (self.register >> 4) & 0x07
    }

    const fn shift(&self) -> u8 {
        self.register & 0x07
    }

    const fn negate(&self) -> bool {
        self.register & 0x08 != 0
    }

    const fn reload_timer(&mut self) {
        // Periode 0 diperlakukan sebagai 8 untuk timer.
        self.timer = if self.period() == 0 { 8 } else { self.period() };
    }

    /// Frekuensi berikutnya; `None` kalau melewati 2047 (kanal berhenti).
    const fn next(&mut self) -> Option<u16> {
        let delta = self.shadow >> self.shift();
        let value = if self.negate() {
            self.negate_used = true;
            self.shadow - delta
        } else {
            self.shadow + delta
        };
        if value > 2047 { None } else { Some(value) }
    }
}

/// Kanal 1 dan 2: gelombang kotak.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Square {
    enabled: bool,
    duty: u8,
    duty_step: u8,
    frequency: u16,
    timer: u32,
    length: Length,
    envelope: Envelope,
    sweep: Sweep,
}

impl Square {
    const fn period(&self) -> u32 {
        (2048 - self.frequency as u32) * 4
    }

    fn advance(&mut self, cycles: u32) {
        let mut cycles = cycles;
        while cycles >= self.timer {
            cycles -= self.timer;
            self.timer = self.period();
            self.duty_step = (self.duty_step + 1) & 7;
        }
        self.timer -= cycles;
    }

    const fn output(&self) -> u8 {
        if self.enabled && (DUTY[self.duty as usize] >> (7 - self.duty_step)) & 1 != 0 {
            self.envelope.volume
        } else {
            0
        }
    }

    /// Satu detak sweep (langkah 2 dan 6). Hanya kanal 1 yang memakainya.
    fn clock_sweep(&mut self) {
        self.sweep.timer = self.sweep.timer.saturating_sub(1);
        if self.sweep.timer != 0 {
            return;
        }
        self.sweep.reload_timer();
        if !self.sweep.enabled || self.sweep.period() == 0 {
            return;
        }
        match self.sweep.next() {
            None => self.enabled = false,
            Some(value) if self.sweep.shift() != 0 => {
                self.sweep.shadow = value;
                self.frequency = value;
                // Dihitung sekali lagi hanya untuk memeriksa luapan.
                if self.sweep.next().is_none() {
                    self.enabled = false;
                }
            }
            Some(_) => {}
        }
    }

    fn trigger(&mut self, with_sweep: bool) {
        self.enabled = self.envelope.dac_enabled();
        self.timer = self.period();
        self.envelope.trigger();
        if with_sweep {
            self.sweep.shadow = self.frequency;
            self.sweep.reload_timer();
            self.sweep.enabled = self.sweep.period() != 0 || self.sweep.shift() != 0;
            self.sweep.negate_used = false;
            if self.sweep.shift() != 0 && self.sweep.next().is_none() {
                self.enabled = false;
            }
        }
    }
}

/// Kanal 3: gelombang dari 32 sampel 4-bit di RAM 0xFF30–0xFF3F.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Wave {
    enabled: bool,
    dac_enabled: bool,
    /// NR32 bit 6–5: 0 = bisu, 1 = 100 %, 2 = 50 %, 3 = 25 %.
    volume_code: u8,
    frequency: u16,
    timer: u32,
    position: u8,
    sample: u8,
    length: Length,
}

impl Wave {
    const fn period(&self) -> u32 {
        (2048 - self.frequency as u32) * 2
    }

    fn advance(&mut self, cycles: u32, ram: &[u8; 16]) {
        let mut cycles = cycles;
        while cycles >= self.timer {
            cycles -= self.timer;
            self.timer = self.period();
            self.position = (self.position + 1) & 31;
            let byte = ram[usize::from(self.position / 2)];
            self.sample = if self.position.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0F
            };
        }
        self.timer -= cycles;
    }

    const fn output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        match self.volume_code {
            0 => 0,
            code => self.sample >> (code - 1),
        }
    }

    const fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        self.timer = self.period();
        self.position = 0;
    }
}

/// Kanal 4: noise dari LFSR 15 bit (atau 7 bit).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Noise {
    enabled: bool,
    /// Isi NR43: geser jam, lebar, pembagi.
    register: u8,
    lfsr: u16,
    timer: u32,
    length: Length,
    envelope: Envelope,
}

impl Noise {
    const fn period(&self) -> u32 {
        NOISE_DIVISOR[(self.register & 0x07) as usize] << (self.register >> 4)
    }

    fn advance(&mut self, cycles: u32) {
        let mut cycles = cycles;
        while cycles >= self.timer {
            cycles -= self.timer;
            self.timer = self.period();
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 14);
            if self.register & 0x08 != 0 {
                self.lfsr = (self.lfsr & !(1 << 6)) | (bit << 6);
            }
        }
        self.timer -= cycles;
    }

    const fn output(&self) -> u8 {
        if self.enabled && self.lfsr & 1 == 0 {
            self.envelope.volume
        } else {
            0
        }
    }

    const fn trigger(&mut self) {
        self.enabled = self.envelope.dac_enabled();
        self.timer = self.period();
        self.lfsr = 0x7FFF;
        self.envelope.trigger();
    }
}

/// Chip suara.
#[derive(Clone, Debug, PartialEq)]
pub struct Apu {
    powered: bool,
    /// Register mentah 0xFF10–0xFF26, untuk dibaca kembali.
    registers: [u8; 0x17],
    wave_ram: [u8; 16],
    ch1: Square,
    ch2: Square,
    ch3: Wave,
    ch4: Noise,
    /// Langkah frame sequencer yang akan datang (0–7).
    frame_step: u8,
    sample_rate: u32,
    sample_clock: u32,
    samples: Vec<f32>,
    /// Muatan "kapasitor" filter high-pass kiri/kanan.
    capacitor: [f32; 2],
    /// Seberapa banyak muatan tersisa per sampel (bergantung laju sampel).
    charge_factor: f32,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    /// APU dalam keadaan setelah boot ROM: menyala, semua kanal diam.
    #[must_use]
    pub fn new() -> Self {
        let mut apu = Self {
            powered: true,
            registers: [0; 0x17],
            wave_ram: [0; 16],
            ch1: Square::default(),
            ch2: Square::default(),
            ch3: Wave::default(),
            ch4: Noise::default(),
            frame_step: 0,
            sample_rate: 0,
            sample_clock: 0,
            samples: Vec::new(),
            capacitor: [0.0; 2],
            charge_factor: 0.0,
        };
        // Nilai setelah boot ROM DMG yang relevan: volume penuh, semua kanal
        // ke kiri dan kanan.
        apu.write(0xFF24, 0x77);
        apu.write(0xFF25, 0xF3);
        apu
    }

    /// Laju sampel yang diinginkan adapter (mis. 48 000). 0 = tidak membuat
    /// sampel (bawaan; untuk tes dan `gb-headless`).
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate;
        self.sample_clock = 0;
        self.samples.clear();
        // Filter high-pass DMG: muatan tersisa 0,999958 per siklus. Dipangkatkan
        // dengan perkalian berulang karena `powf` tidak ada di `no_std`.
        let mut factor = 1.0f32;
        if let Some(cycles_per_sample) = CPU_HZ.checked_div(rate) {
            for _ in 0..cycles_per_sample {
                factor *= 0.999_958;
            }
        }
        self.charge_factor = factor;
    }

    /// Mengambil semua sampel yang sudah dibuat: stereo berselang-seling
    /// (kiri, kanan, kiri, kanan, …).
    pub fn take_samples(&mut self) -> Vec<f32> {
        core::mem::take(&mut self.samples)
    }

    /// Memajukan kanal sebanyak `cycles` T-cycle dan membuat sampel yang
    /// jatuh tempo.
    pub fn tick(&mut self, cycles: u8) {
        let cycles = u32::from(cycles);
        if self.powered {
            self.ch1.advance(cycles);
            self.ch2.advance(cycles);
            self.ch3.advance(cycles, &self.wave_ram);
            self.ch4.advance(cycles);
        }
        if self.sample_rate == 0 {
            return;
        }
        self.sample_clock += cycles * self.sample_rate;
        while self.sample_clock >= CPU_HZ {
            self.sample_clock -= CPU_HZ;
            self.push_sample();
        }
    }

    /// Satu langkah frame sequencer (512 Hz), dipanggil bus saat bit 12
    /// pencacah DIV jatuh dari 1 ke 0.
    pub fn frame_sequencer(&mut self) {
        if !self.powered {
            return;
        }
        let step = self.frame_step;
        if step.is_multiple_of(2) {
            if self.ch1.length.clock() {
                self.ch1.enabled = false;
            }
            if self.ch2.length.clock() {
                self.ch2.enabled = false;
            }
            if self.ch3.length.clock() {
                self.ch3.enabled = false;
            }
            if self.ch4.length.clock() {
                self.ch4.enabled = false;
            }
        }
        if step == 2 || step == 6 {
            self.ch1.clock_sweep();
        }
        if step == 7 {
            self.ch1.envelope.clock();
            self.ch2.envelope.clock();
            self.ch4.envelope.clock();
        }
        self.frame_step = (step + 1) & 7;
    }

    /// Satu sampel stereo dari keadaan kanal saat ini.
    fn push_sample(&mut self) {
        let mut mix = [0.0f32; 2];
        if self.powered {
            let nr51 = self.registers[0x15];
            let digital = [
                (self.ch1.output(), self.ch1.envelope.dac_enabled()),
                (self.ch2.output(), self.ch2.envelope.dac_enabled()),
                (self.ch3.output(), self.ch3.dac_enabled),
                (self.ch4.output(), self.ch4.envelope.dac_enabled()),
            ];
            for (index, (value, dac)) in digital.into_iter().enumerate() {
                if !dac {
                    continue;
                }
                // DAC: 0 → +1.0, 15 → −1.0 (tegangan terbalik di DMG).
                let analog = 1.0 - f32::from(value) / 7.5;
                if nr51 & (0x10 << index) != 0 {
                    mix[0] += analog;
                }
                if nr51 & (0x01 << index) != 0 {
                    mix[1] += analog;
                }
            }
            let nr50 = self.registers[0x14];
            mix[0] *= f32::from(((nr50 >> 4) & 7) + 1) / 8.0;
            mix[1] *= f32::from((nr50 & 7) + 1) / 8.0;
        }
        for (side, value) in mix.into_iter().enumerate() {
            let input = value / 4.0;
            let output = input - self.capacitor[side];
            self.capacitor[side] = input - output * self.charge_factor;
            self.samples.push(output);
        }
    }

    /// Membaca 0xFF10–0xFF3F. Bit yang tidak terpakai terbaca 1.
    #[must_use]
    pub fn read(&self, address: u16) -> u8 {
        /// Bit yang selalu 1 per register 0xFF10–0xFF26.
        const UNUSED: [u8; 0x17] = [
            0x80, 0x3F, 0x00, 0xFF, 0xBF, // NR10–NR14
            0xFF, 0x3F, 0x00, 0xFF, 0xBF, // —, NR21–NR24
            0x7F, 0xFF, 0x9F, 0xFF, 0xBF, // NR30–NR34
            0xFF, 0xFF, 0x00, 0x00, 0xBF, // —, NR41–NR44
            0x00, 0x00, 0x70, // NR50, NR51, NR52
        ];
        match address {
            0xFF30..=0xFF3F => self.wave_ram[usize::from(address - 0xFF30)],
            0xFF26 => {
                let status = u8::from(self.ch1.enabled)
                    | u8::from(self.ch2.enabled) << 1
                    | u8::from(self.ch3.enabled) << 2
                    | u8::from(self.ch4.enabled) << 3;
                u8::from(self.powered) << 7 | 0x70 | status
            }
            0xFF10..=0xFF25 => {
                let index = usize::from(address - 0xFF10);
                self.registers[index] | UNUSED[index]
            }
            _ => 0xFF,
        }
    }

    /// Menulis 0xFF10–0xFF3F.
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF30..=0xFF3F => self.wave_ram[usize::from(address - 0xFF30)] = value,
            0xFF26 => self.write_power(value & 0x80 != 0),
            0xFF10..=0xFF25 if self.powered => {
                self.registers[usize::from(address - 0xFF10)] = value;
                self.write_channel(address, value);
            }
            // Mati: register lain diabaikan, kecuali penghitung panjang (DMG).
            0xFF11 | 0xFF16 | 0xFF20 => self.write_length(address, value & 0x3F),
            0xFF1B => self.ch3.length.counter = 256 - u16::from(value),
            _ => {}
        }
    }

    fn write_power(&mut self, on: bool) {
        if on && !self.powered {
            // Menyala: langkah berikutnya 0, duty kembali ke awal.
            self.frame_step = 0;
            self.ch1.duty_step = 0;
            self.ch2.duty_step = 0;
            self.ch3.sample = 0;
        } else if !on && self.powered {
            // Mati: semua register dikosongkan, tetapi penghitung panjang
            // tetap (DMG).
            let lengths = [
                self.ch1.length.counter,
                self.ch2.length.counter,
                self.ch3.length.counter,
                self.ch4.length.counter,
            ];
            for address in 0xFF10..=0xFF25 {
                self.write(address, 0);
            }
            self.ch1.length.counter = lengths[0];
            self.ch2.length.counter = lengths[1];
            self.ch3.length.counter = lengths[2];
            self.ch4.length.counter = lengths[3];
        }
        self.powered = on;
    }

    fn write_length(&mut self, address: u16, length: u8) {
        let counter = 64 - u16::from(length);
        match address {
            0xFF11 => self.ch1.length.counter = counter,
            0xFF16 => self.ch2.length.counter = counter,
            _ => self.ch4.length.counter = counter,
        }
    }

    fn write_channel(&mut self, address: u16, value: u8) {
        match address {
            0xFF10 => {
                self.ch1.sweep.register = value;
                // Arah turun sudah dipakai, lalu diganti naik: kanal berhenti.
                if self.ch1.sweep.negate_used && value & 0x08 == 0 {
                    self.ch1.enabled = false;
                }
            }
            0xFF11 | 0xFF16 => {
                let channel = if address == 0xFF11 {
                    &mut self.ch1
                } else {
                    &mut self.ch2
                };
                channel.duty = value >> 6;
                self.write_length(address, value & 0x3F);
            }
            0xFF12 | 0xFF17 => {
                let channel = if address == 0xFF12 {
                    &mut self.ch1
                } else {
                    &mut self.ch2
                };
                channel.envelope.register = value;
                if !channel.envelope.dac_enabled() {
                    channel.enabled = false;
                }
            }
            0xFF13 | 0xFF18 => {
                let channel = if address == 0xFF13 {
                    &mut self.ch1
                } else {
                    &mut self.ch2
                };
                channel.frequency = (channel.frequency & 0x700) | u16::from(value);
            }
            0xFF14 | 0xFF19 => {
                let extra = self.extra_length_clock();
                let channel = if address == 0xFF14 {
                    &mut self.ch1
                } else {
                    &mut self.ch2
                };
                channel.frequency = (channel.frequency & 0xFF) | (u16::from(value & 7) << 8);
                let disable = control(&mut channel.length, value, extra, 64);
                if value & 0x80 != 0 {
                    channel.trigger(address == 0xFF14);
                } else if disable {
                    channel.enabled = false;
                }
            }
            0xFF1A => {
                self.ch3.dac_enabled = value & 0x80 != 0;
                if !self.ch3.dac_enabled {
                    self.ch3.enabled = false;
                }
            }
            0xFF1B => self.ch3.length.counter = 256 - u16::from(value),
            0xFF1C => self.ch3.volume_code = (value >> 5) & 3,
            0xFF1D => self.ch3.frequency = (self.ch3.frequency & 0x700) | u16::from(value),
            0xFF1E => {
                let extra = self.extra_length_clock();
                self.ch3.frequency = (self.ch3.frequency & 0xFF) | (u16::from(value & 7) << 8);
                let disable = control(&mut self.ch3.length, value, extra, 256);
                if value & 0x80 != 0 {
                    self.ch3.trigger();
                } else if disable {
                    self.ch3.enabled = false;
                }
            }
            0xFF20 => self.write_length(address, value & 0x3F),
            0xFF21 => {
                self.ch4.envelope.register = value;
                if !self.ch4.envelope.dac_enabled() {
                    self.ch4.enabled = false;
                }
            }
            0xFF22 => self.ch4.register = value,
            0xFF23 => {
                let extra = self.extra_length_clock();
                let disable = control(&mut self.ch4.length, value, extra, 64);
                if value & 0x80 != 0 {
                    self.ch4.trigger();
                } else if disable {
                    self.ch4.enabled = false;
                }
            }
            // NR50, NR51 dan register kosong cukup disimpan.
            _ => {}
        }
    }

    /// Langkah berikutnya tidak memajukan length (langkah ganjil): menulis
    /// `NRx4` sekarang memicu detak length tambahan (perilaku hardware).
    const fn extra_length_clock(&self) -> bool {
        self.frame_step % 2 == 1
    }
}

/// Bagian length dari tulisan `NRx4`. Mengembalikan `true` kalau kanal harus
/// berhenti karena detak tambahan membuat penghitung 0 tanpa trigger.
fn control(length: &mut Length, value: u8, extra: bool, max: u16) -> bool {
    let was_enabled = length.enabled;
    length.enabled = value & 0x40 != 0;
    let mut disable = false;
    if extra && !was_enabled && length.enabled && length.counter > 0 {
        length.counter -= 1;
        disable = length.counter == 0;
    }
    if value & 0x80 != 0 && length.counter == 0 {
        // Trigger dengan penghitung 0: diisi penuh, dikurangi satu kalau
        // detak tambahan berlaku.
        length.counter = if length.enabled && extra {
            max - 1
        } else {
            max
        };
    }
    disable
}

// --- Save state (ADR-0016) -------------------------------------------------
// Laju sampel, buffer sampel, dan filter high-pass tidak ikut: itu urusan
// adapter dan keadaan sementara, bukan keadaan chip.

impl Length {
    fn save(&self, w: &mut Writer) {
        w.bool(self.enabled);
        w.u16(self.counter);
    }

    fn load(&mut self, r: &mut Reader<'_>) -> Result<(), StateError> {
        self.enabled = r.bool()?;
        self.counter = r.u16()?;
        Ok(())
    }
}

impl Envelope {
    fn save(&self, w: &mut Writer) {
        w.u8(self.register);
        w.u8(self.volume);
        w.u8(self.timer);
    }

    fn load(&mut self, r: &mut Reader<'_>) -> Result<(), StateError> {
        self.register = r.u8()?;
        self.volume = r.u8()?;
        self.timer = r.u8()?;
        Ok(())
    }
}

impl Square {
    fn save(&self, w: &mut Writer) {
        w.bool(self.enabled);
        w.u8(self.duty);
        w.u8(self.duty_step);
        w.u16(self.frequency);
        w.u32(self.timer);
        self.length.save(w);
        self.envelope.save(w);
        w.u8(self.sweep.register);
        w.bool(self.sweep.enabled);
        w.u16(self.sweep.shadow);
        w.u8(self.sweep.timer);
        w.bool(self.sweep.negate_used);
    }

    fn load(&mut self, r: &mut Reader<'_>) -> Result<(), StateError> {
        self.enabled = r.bool()?;
        self.duty = r.u8()? & 3;
        self.duty_step = r.u8()? & 7;
        self.frequency = r.u16()? & 0x7FF;
        self.timer = r.u32()?;
        self.length.load(r)?;
        self.envelope.load(r)?;
        self.sweep.register = r.u8()?;
        self.sweep.enabled = r.bool()?;
        self.sweep.shadow = r.u16()?;
        self.sweep.timer = r.u8()?;
        self.sweep.negate_used = r.bool()?;
        Ok(())
    }
}

impl Apu {
    /// Tes: kosongkan bagian yang sengaja tidak ikut save state.
    #[cfg(test)]
    pub(crate) fn clear_transient(&mut self) {
        self.sample_rate = 0;
        self.sample_clock = 0;
        self.samples.clear();
        self.capacitor = [0.0; 2];
        self.charge_factor = 0.0;
    }

    /// Menulis keadaan chip suara ke save state.
    pub(crate) fn save(&self, w: &mut Writer) {
        w.bool(self.powered);
        w.bytes(&self.registers);
        w.bytes(&self.wave_ram);
        self.ch1.save(w);
        self.ch2.save(w);
        let ch3 = &self.ch3;
        w.bool(ch3.enabled);
        w.bool(ch3.dac_enabled);
        w.u8(ch3.volume_code);
        w.u16(ch3.frequency);
        w.u32(ch3.timer);
        w.u8(ch3.position);
        w.u8(ch3.sample);
        ch3.length.save(w);
        let ch4 = &self.ch4;
        w.bool(ch4.enabled);
        w.u8(ch4.register);
        w.u16(ch4.lfsr);
        w.u32(ch4.timer);
        ch4.length.save(w);
        ch4.envelope.save(w);
        w.u8(self.frame_step);
    }

    /// Membaca keadaan chip suara dari save state.
    pub(crate) fn load(&mut self, r: &mut Reader<'_>) -> Result<(), StateError> {
        self.powered = r.bool()?;
        r.bytes_into(&mut self.registers)?;
        r.bytes_into(&mut self.wave_ram)?;
        self.ch1.load(r)?;
        self.ch2.load(r)?;
        let ch3 = &mut self.ch3;
        ch3.enabled = r.bool()?;
        ch3.dac_enabled = r.bool()?;
        ch3.volume_code = r.u8()? & 3;
        ch3.frequency = r.u16()? & 0x7FF;
        ch3.timer = r.u32()?;
        ch3.position = r.u8()? & 31;
        ch3.sample = r.u8()? & 0x0F;
        ch3.length.load(r)?;
        let ch4 = &mut self.ch4;
        ch4.enabled = r.bool()?;
        ch4.register = r.u8()?;
        ch4.lfsr = r.u16()?;
        ch4.timer = r.u32()?;
        ch4.length.load(r)?;
        ch4.envelope.load(r)?;
        self.frame_step = r.u8()? & 7;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
