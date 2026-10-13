//! PPU (Picture Processing Unit): chip yang menggambar layar 160×144.
//!
//! ## Waktu
//!
//! Satu frame = 154 baris × 456 T-cycle = 70.224 siklus (±59,7 frame/detik).
//! Baris 0–143 terlihat; baris 144–153 adalah **V-blank** (jeda vertikal).
//! Setiap baris terlihat melewati tiga mode:
//!
//! ```text
//! siklus   0 ───── 80 ─────────── 252 ──────────────── 456
//! mode        2         3                  0
//!          cari      gambar          H-blank (istirahat)
//!          sprite    piksel
//! ```
//!
//! Di baris V-blank modenya 1. Register `LY` (0xFF44) memberi tahu baris
//! mana yang sedang digambar; program memakainya untuk menunggu V-blank,
//! satu-satunya saat aman mengubah VRAM tanpa merusak gambar.
//!
//! ## Gambar (M4: background saja)
//!
//! Background adalah peta 32×32 tile; setiap tile 8×8 piksel, 2 bit per
//! piksel (16 byte). Register `SCX`/`SCY` menggeser jendela 160×144 di atas
//! peta 256×256 itu. Setiap piksel menghasilkan nomor warna 0–3, lalu
//! palet `BGP` (0xFF47) memetakannya ke tingkat kegelapan 0–3.
//!
//! Framebuffer berisi tingkat kegelapan 0 (terang) – 3 (gelap), bukan warna.
//! Warna sebenarnya urusan adapter (ADR-0008, palet MXM).
//!
//! Penyederhanaan yang disadari (dicatat juga di ADR-0012):
//! satu baris digambar sekaligus saat masuk H-blank; panjang mode 3 selalu
//! 172 siklus; VRAM/OAM tidak dikunci selama mode 2/3. Window dan sprite
//! menyusul di M5.

/// Lebar layar dalam piksel.
pub const WIDTH: usize = 160;
/// Tinggi layar dalam piksel.
pub const HEIGHT: usize = 144;

/// Jumlah baris terlihat, sebagai `u8` untuk dibandingkan dengan `LY`.
const VISIBLE_LINES: u8 = 144;

/// Siklus per baris.
const DOTS_PER_LINE: u16 = 456;
/// Siklus akhir mode 2 (cari sprite).
const MODE2_END: u16 = 80;
/// Siklus akhir mode 3 (gambar piksel).
const MODE3_END: u16 = 80 + 172;
/// Jumlah baris per frame, termasuk V-blank.
const LINES_PER_FRAME: u8 = 154;

/// Bit interrupt di IF yang bisa diminta PPU.
pub const VBLANK_INTERRUPT: u8 = 0b0000_0001;
/// Bit interrupt STAT di IF.
pub const STAT_INTERRUPT: u8 = 0b0000_0010;

use alloc::boxed::Box;
use alloc::vec;

/// PPU DMG, beserta VRAM dan OAM yang hanya ia gunakan.
pub struct Ppu {
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    /// 23.040 byte: di heap, bukan di stack (stack HP/mikrokontroler kecil).
    framebuffer: Box<[u8]>,
    lcdc: u8,
    /// Hanya bit 3–6 (pilihan sumber interrupt STAT) yang disimpan di sini.
    stat_select: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    dot: u16,
    /// Keadaan "garis" interrupt STAT sebelumnya; interrupt hanya diminta
    /// saat garis naik dari 0 ke 1 (hardware asli bekerja begini).
    stat_line: bool,
    frame_ready: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    /// PPU dalam keadaan setelah boot ROM: LCD menyala, BGP = 0xFC.
    #[must_use]
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            framebuffer: vec![0; WIDTH * HEIGHT].into_boxed_slice(),
            lcdc: 0x91,
            stat_select: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            dot: 0,
            stat_line: false,
            frame_ready: false,
        }
    }

    /// Tingkat kegelapan 160×144 piksel, baris demi baris (0 terang – 3 gelap).
    #[must_use]
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// `true` sekali setiap kali frame selesai (saat masuk V-blank).
    pub const fn take_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    /// Apakah LCD menyala (LCDC bit 7).
    #[must_use]
    pub const fn lcd_enabled(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    /// Membaca VRAM (0x8000–0x9FFF).
    #[must_use]
    pub const fn read_vram(&self, address: u16) -> u8 {
        self.vram[(address - 0x8000) as usize]
    }

    /// Menulis VRAM (0x8000–0x9FFF).
    pub const fn write_vram(&mut self, address: u16, value: u8) {
        self.vram[(address - 0x8000) as usize] = value;
    }

    /// Membaca OAM (0xFE00–0xFE9F).
    #[must_use]
    pub const fn read_oam(&self, address: u16) -> u8 {
        self.oam[(address - 0xFE00) as usize]
    }

    /// Menulis OAM (0xFE00–0xFE9F).
    pub const fn write_oam(&mut self, address: u16, value: u8) {
        self.oam[(address - 0xFE00) as usize] = value;
    }

    /// Mode sekarang: 0 H-blank, 1 V-blank, 2 cari sprite, 3 gambar.
    const fn mode(&self) -> u8 {
        if !self.lcd_enabled() {
            0
        } else if self.ly >= VISIBLE_LINES {
            1
        } else if self.dot < MODE2_END {
            2
        } else if self.dot < MODE3_END {
            3
        } else {
            0
        }
    }

    /// Membaca register LCD 0xFF40–0xFF4B (kecuali 0xFF46, DMA, di bus).
    #[must_use]
    pub const fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF40 => self.lcdc,
            // Bit 7 selalu 1; bit 2 = LY sama dengan LYC; bit 1–0 = mode.
            0xFF41 => {
                let coincidence = if self.ly == self.lyc { 0b100 } else { 0 };
                0x80 | self.stat_select | coincidence | self.mode()
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    /// Menulis register LCD. `LY` hanya bisa dibaca.
    pub const fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0xFF40 => {
                let was_on = self.lcd_enabled();
                self.lcdc = value;
                // Mematikan LCD mengembalikan PPU ke awal frame.
                if was_on && !self.lcd_enabled() {
                    self.ly = 0;
                    self.dot = 0;
                }
            }
            0xFF41 => self.stat_select = value & 0x78,
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    /// Memajukan PPU `cycles` T-cycle. Mengembalikan bit interrupt yang
    /// diminta (gabungan [`VBLANK_INTERRUPT`] dan [`STAT_INTERRUPT`]).
    pub fn tick(&mut self, cycles: u8) -> u8 {
        if !self.lcd_enabled() {
            return 0;
        }
        let mut interrupts = 0;
        for _ in 0..cycles {
            self.dot += 1;
            if self.dot == MODE3_END && self.ly < VISIBLE_LINES {
                self.render_line();
            }
            if self.dot == DOTS_PER_LINE {
                self.dot = 0;
                self.ly += 1;
                if self.ly == VISIBLE_LINES {
                    interrupts |= VBLANK_INTERRUPT;
                    self.frame_ready = true;
                } else if self.ly == LINES_PER_FRAME {
                    self.ly = 0;
                }
            }
            interrupts |= self.update_stat_line();
        }
        interrupts
    }

    /// Menghitung ulang garis interrupt STAT; mengembalikan
    /// [`STAT_INTERRUPT`] hanya saat garis naik dari 0 ke 1.
    const fn update_stat_line(&mut self) -> u8 {
        let mode = self.mode();
        let line = (self.stat_select & 0x08 != 0 && mode == 0)
            || (self.stat_select & 0x10 != 0 && mode == 1)
            || (self.stat_select & 0x20 != 0 && mode == 2)
            || (self.stat_select & 0x40 != 0 && self.ly == self.lyc);
        let rising = line && !self.stat_line;
        self.stat_line = line;
        if rising { STAT_INTERRUPT } else { 0 }
    }

    /// Menggambar baris `LY` dari background ke framebuffer.
    fn render_line(&mut self) {
        let row = usize::from(self.ly) * WIDTH;
        // LCDC bit 0 mati: di DMG background menjadi putih (warna 0).
        if self.lcdc & 0x01 == 0 {
            self.framebuffer[row..row + WIDTH].fill(0);
            return;
        }
        let map_base: u16 = if self.lcdc & 0x08 != 0 {
            0x9C00
        } else {
            0x9800
        };
        let y = self.ly.wrapping_add(self.scy);
        for x in 0..WIDTH {
            // `x` < 160, jadi muat di u8.
            let px = u8::try_from(x).unwrap_or(0).wrapping_add(self.scx);
            let map_address = map_base + u16::from(y / 8) * 32 + u16::from(px / 8);
            let tile = self.read_vram(map_address);
            let color = self.tile_pixel(tile, px % 8, y % 8);
            self.framebuffer[row + x] = (self.bgp >> (color * 2)) & 0b11;
        }
    }

    /// Nomor warna 0–3 piksel (`col`, `line`) di tile `tile`.
    ///
    /// LCDC bit 4 memilih cara menomori tile: 1 = 0x8000 + tile × 16
    /// (0–255); 0 = 0x9000 + tile × 16 dengan tile bertanda (−128–127).
    /// Setiap baris tile = 2 byte: bit rendah warna di byte pertama, bit
    /// tinggi di byte kedua; piksel paling kiri di bit 7.
    fn tile_pixel(&self, tile: u8, col: u8, line: u8) -> u8 {
        let base = if self.lcdc & 0x10 != 0 {
            0x8000 + u16::from(tile) * 16
        } else {
            0x9000u16.wrapping_add_signed(i16::from(i8::from_le_bytes([tile])) * 16)
        };
        let address = base + u16::from(line) * 2;
        let lo = self.read_vram(address);
        let hi = self.read_vram(address + 1);
        let bit = 7 - col;
        (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{Ppu, STAT_INTERRUPT, VBLANK_INTERRUPT, WIDTH};

    /// Memajukan PPU `cycles` siklus (boleh banyak); gabungan interrupt-nya.
    fn run(ppu: &mut Ppu, mut cycles: u32) -> u8 {
        let mut interrupts = 0;
        while cycles > 0 {
            let step = u8::try_from(cycles.min(4)).unwrap_or(4);
            interrupts |= ppu.tick(step);
            cycles -= u32::from(step);
        }
        interrupts
    }

    fn stat_mode(ppu: &Ppu) -> u8 {
        ppu.read_register(0xFF41) & 0b11
    }

    #[test]
    fn one_line_is_456_cycles_and_passes_modes_2_3_0() {
        let mut ppu = Ppu::new();
        assert_eq!((ppu.read_register(0xFF44), stat_mode(&ppu)), (0, 2));
        run(&mut ppu, 76);
        assert_eq!(stat_mode(&ppu), 2, "masih mode 2 tepat sebelum siklus 80");
        run(&mut ppu, 4);
        assert_eq!(stat_mode(&ppu), 3);
        // Batas dicek dari dua sisi: mode 3 harus masih berlangsung sampai
        // tepat sebelum siklus 252, bukan hanya "sudah selesai" sesudahnya.
        run(&mut ppu, 168);
        assert_eq!(stat_mode(&ppu), 3, "masih mode 3 tepat sebelum siklus 252");
        run(&mut ppu, 4);
        assert_eq!(stat_mode(&ppu), 0);
        run(&mut ppu, 204 - 4);
        assert_eq!(ppu.read_register(0xFF44), 0, "baris belum berganti");
        run(&mut ppu, 4);
        assert_eq!((ppu.read_register(0xFF44), stat_mode(&ppu)), (1, 2));
    }

    #[test]
    fn vblank_starts_at_line_144_once_per_frame() {
        let mut ppu = Ppu::new();
        let before = run(&mut ppu, 144 * 456 - 4);
        assert_eq!(before & VBLANK_INTERRUPT, 0);
        assert!(!ppu.take_frame_ready());
        let at = run(&mut ppu, 4);
        assert_eq!(at & VBLANK_INTERRUPT, VBLANK_INTERRUPT);
        assert_eq!((ppu.read_register(0xFF44), stat_mode(&ppu)), (144, 1));
        assert!(ppu.take_frame_ready());
        assert!(!ppu.take_frame_ready(), "tanda frame hanya sekali");
        // Sisa frame (10 baris V-blank) lalu kembali ke baris 0.
        let rest = run(&mut ppu, 10 * 456);
        assert_eq!(rest & VBLANK_INTERRUPT, 0);
        assert_eq!(ppu.read_register(0xFF44), 0);
    }

    #[test]
    fn a_frame_is_70224_cycles() {
        let mut ppu = Ppu::new();
        run(&mut ppu, 70_224);
        assert_eq!(ppu.read_register(0xFF44), 0);
        assert_eq!(stat_mode(&ppu), 2);
    }

    #[test]
    fn lyc_coincidence_flag_and_interrupt() {
        let mut ppu = Ppu::new();
        ppu.write_register(0xFF45, 3); // LYC = 3
        ppu.write_register(0xFF41, 0x40); // interrupt saat LY == LYC
        let before = run(&mut ppu, 3 * 456 - 4);
        assert_eq!(before & STAT_INTERRUPT, 0);
        assert_eq!(ppu.read_register(0xFF41) & 0b100, 0);
        let at = run(&mut ppu, 4);
        assert_eq!(at & STAT_INTERRUPT, STAT_INTERRUPT);
        assert_eq!(ppu.read_register(0xFF41) & 0b100, 0b100);
        // Selama LY tetap 3, interrupt tidak diulang (hanya saat naik).
        assert_eq!(run(&mut ppu, 100) & STAT_INTERRUPT, 0);
    }

    #[test]
    fn stat_hblank_interrupt_fires_every_visible_line() {
        let mut ppu = Ppu::new();
        ppu.write_register(0xFF41, 0x08); // interrupt saat masuk mode 0
        let mut count = 0;
        for _ in 0..(144 * 456 / 4) {
            if ppu.tick(4) & STAT_INTERRUPT != 0 {
                count += 1;
            }
        }
        assert_eq!(count, 144);
    }

    #[test]
    fn turning_lcd_off_resets_ly_and_stops_time() {
        let mut ppu = Ppu::new();
        run(&mut ppu, 10 * 456);
        ppu.write_register(0xFF40, 0x11); // bit 7 mati
        assert_eq!((ppu.read_register(0xFF44), stat_mode(&ppu)), (0, 0));
        assert_eq!(run(&mut ppu, 70_224), 0);
        assert_eq!(ppu.read_register(0xFF44), 0);
    }

    #[test]
    fn stat_bit_7_reads_one_and_ly_is_read_only() {
        let mut ppu = Ppu::new();
        ppu.write_register(0xFF41, 0x00);
        assert_eq!(ppu.read_register(0xFF41) & 0x80, 0x80);
        ppu.write_register(0xFF44, 99);
        assert_eq!(ppu.read_register(0xFF44), 0);
    }

    /// Tile 1 di 0x8010: baris 0 = warna 0,1,2,3,0,1,2,3 dari kiri.
    fn ppu_with_striped_tile() -> Ppu {
        let mut ppu = Ppu::new();
        // Bit rendah: 0101_0101, bit tinggi: 0011_0011 → 0 1 2 3 0 1 2 3.
        ppu.write_vram(0x8010, 0b0101_0101);
        ppu.write_vram(0x8011, 0b0011_0011);
        ppu.write_vram(0x9800, 1); // tile kiri atas peta = tile 1
        ppu.write_register(0xFF47, 0b11_10_01_00); // BGP identitas
        ppu
    }

    #[test]
    fn background_pixels_come_from_tile_data() {
        let mut ppu = ppu_with_striped_tile();
        run(&mut ppu, 456); // baris 0 selesai digambar
        assert_eq!(&ppu.framebuffer()[..8], &[0, 1, 2, 3, 0, 1, 2, 3]);
        // Tile berikutnya di peta = tile 0 (kosong).
        assert_eq!(&ppu.framebuffer()[8..16], &[0; 8]);
    }

    #[test]
    fn bgp_maps_colors_to_shades() {
        let mut ppu = ppu_with_striped_tile();
        ppu.write_register(0xFF47, 0b00_01_10_11); // dibalik: warna 0 → 3 dst.
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..4], &[3, 2, 1, 0]);
    }

    #[test]
    fn scx_scrolls_horizontally() {
        let mut ppu = ppu_with_striped_tile();
        ppu.write_register(0xFF43, 2); // geser 2 piksel
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..6], &[2, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn scy_scrolls_vertically() {
        let mut ppu = ppu_with_striped_tile();
        ppu.write_register(0xFF42, 1); // geser 1 baris: baris 0 layar = baris 1 tile
        ppu.write_vram(0x8012, 0xFF); // baris 1 tile 1: semua warna 1
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[1; 8]);
    }

    #[test]
    fn signed_tile_addressing_uses_0x9000() {
        // LCDC bit 4 = 0: tile 0xFF berarti −1 → 0x9000 − 16 = 0x8FF0.
        let mut ppu = Ppu::new();
        ppu.write_register(0xFF40, 0x81); // LCD on, BG on, tile data 0x8800
        ppu.write_register(0xFF47, 0b11_10_01_00);
        ppu.write_vram(0x8FF0, 0xFF);
        ppu.write_vram(0x8FF1, 0xFF); // baris 0: semua warna 3
        ppu.write_vram(0x9800, 0xFF);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[3; 8]);
    }

    #[test]
    fn background_off_draws_shade_zero() {
        let mut ppu = ppu_with_striped_tile();
        ppu.write_register(0xFF40, 0x90); // LCD on, BG off
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..WIDTH], &[0; WIDTH]);
    }
}
