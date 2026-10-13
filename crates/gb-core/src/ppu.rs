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
//! ## Gambar: tiga lapisan
//!
//! 1. **Background**: peta tile yang bisa digeser (`SCX`/`SCY`).
//! 2. **Window**: peta tile kedua yang tidak ikut bergeser, mulai di
//!    (`WX` − 7, `WY`). Biasa dipakai untuk skor atau menu.
//! 3. **Sprite** (objek): 40 gambar kecil yang bisa bergerak bebas, datanya
//!    di OAM (4 byte per sprite). Lihat [`Ppu::render_sprites`].
//!
//! ## Background
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
//! 172 siklus; VRAM/OAM tidak dikunci selama mode 2/3.

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
    /// Baris window berikutnya. Hanya naik pada baris yang benar-benar
    /// menggambar window, jadi window yang disembunyikan di tengah layar
    /// melanjutkan dari baris terakhirnya (perilaku hardware).
    window_line: u8,
    /// Nomor warna background/window (sebelum palet) untuk baris yang sedang
    /// digambar. Sprite berprioritas rendah hanya tampil di atas warna 0.
    line_colors: [u8; WIDTH],
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
            window_line: 0,
            line_colors: [0; WIDTH],
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
                    self.window_line = 0;
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

    /// Menggambar baris `LY`: background, lalu window, lalu sprite.
    fn render_line(&mut self) {
        self.render_background();
        self.render_window();
        if self.lcdc & 0x02 != 0 {
            self.render_sprites();
        }
    }

    /// Peta tile yang dipilih bit LCDC tertentu: 0x9C00 kalau menyala,
    /// 0x9800 kalau mati.
    const fn tile_map(&self, lcdc_bit: u8) -> u16 {
        if self.lcdc & lcdc_bit != 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Lapisan 1: background yang digeser `SCX`/`SCY`.
    fn render_background(&mut self) {
        let row = usize::from(self.ly) * WIDTH;
        // LCDC bit 0 mati: di DMG background dan window menjadi putih
        // (tingkat 0), tanpa lewat palet BGP. Sprite tetap bisa tampil.
        if self.lcdc & 0x01 == 0 {
            self.line_colors.fill(0);
            self.framebuffer[row..row + WIDTH].fill(0);
            return;
        }
        let map_base = self.tile_map(0x08);
        let y = self.ly.wrapping_add(self.scy);
        for x in 0..WIDTH {
            // `x` < 160, jadi muat di u8.
            let px = u8::try_from(x).unwrap_or(0).wrapping_add(self.scx);
            let map_address = map_base + u16::from(y / 8) * 32 + u16::from(px / 8);
            let tile = self.read_vram(map_address);
            let color = self.tile_pixel(tile, px % 8, y % 8);
            self.line_colors[x] = color;
            self.framebuffer[row + x] = (self.bgp >> (color * 2)) & 0b11;
        }
    }

    /// Lapisan 2: window, menimpa background mulai kolom `WX` − 7.
    fn render_window(&mut self) {
        // LCDC bit 5 = window menyala; di DMG bit 0 mematikan keduanya.
        if self.lcdc & 0x21 != 0x21 || self.ly < self.wy || self.wx > 166 {
            return;
        }
        let row = usize::from(self.ly) * WIDTH;
        let map_base = self.tile_map(0x40);
        let y = self.window_line;
        // WX 0–6 berarti window mulai sebelum layar; kolom di luar dilewati.
        let start = i16::from(self.wx) - 7;
        for x in 0..WIDTH {
            let screen_x = i16::try_from(x).unwrap_or(0);
            if screen_x < start {
                continue;
            }
            let wx = u8::try_from(screen_x - start).unwrap_or(0);
            let map_address = map_base + u16::from(y / 8) * 32 + u16::from(wx / 8);
            let tile = self.read_vram(map_address);
            let color = self.tile_pixel(tile, wx % 8, y % 8);
            self.line_colors[x] = color;
            self.framebuffer[row + x] = (self.bgp >> (color * 2)) & 0b11;
        }
        self.window_line = self.window_line.wrapping_add(1);
    }

    /// Lapisan 3: sprite.
    ///
    /// Setiap sprite di OAM memakai 4 byte:
    ///
    /// | byte | isi |
    /// |---|---|
    /// | 0 | Y + 16 (Y = 0 berarti tersembunyi di atas layar) |
    /// | 1 | X + 8 |
    /// | 2 | nomor tile (selalu dari 0x8000) |
    /// | 3 | atribut: bit 7 di belakang BG, bit 6 balik vertikal, bit 5 balik horizontal, bit 4 palet OBP1 |
    ///
    /// Aturan DMG: paling banyak **10 sprite per baris** (10 pertama di OAM
    /// yang menyentuh baris itu); warna 0 tembus pandang; kalau dua sprite
    /// bertumpuk, yang X-nya lebih kecil menang (X sama: urutan OAM lebih
    /// awal). Tinggi 8 atau 16 piksel menurut LCDC bit 2.
    fn render_sprites(&mut self) {
        let height: i16 = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let ly = i16::from(self.ly);
        // Kumpulkan sampai 10 sprite yang menyentuh baris ini, urut OAM.
        let mut chosen = [0usize; 10];
        let mut count = 0;
        for index in 0..40 {
            let top = i16::from(self.oam[index * 4]) - 16;
            if ly >= top && ly < top + height {
                chosen[count] = index;
                count += 1;
                if count == 10 {
                    break;
                }
            }
        }
        // Gambar dari prioritas terendah ke tertinggi, supaya yang menang
        // ditulis terakhir: X lebih besar dulu; X sama → OAM lebih akhir dulu.
        let chosen = &mut chosen[..count];
        chosen.sort_unstable_by_key(|&index| (self.oam[index * 4 + 1], index));
        let row = usize::from(self.ly) * WIDTH;
        for &index in chosen.iter().rev() {
            let base = index * 4;
            let top = i16::from(self.oam[base]) - 16;
            let left = i16::from(self.oam[base + 1]) - 8;
            let attributes = self.oam[base + 3];
            let mut line = ly - top;
            if attributes & 0x40 != 0 {
                line = height - 1 - line;
            }
            let mut tile = self.oam[base + 2];
            if height == 16 {
                tile &= 0xFE; // sprite 8×16 memakai pasangan tile genap/ganjil
            }
            // `line` 0..height, jadi muat di u8.
            let line = u8::try_from(line).unwrap_or(0);
            let tile_address = 0x8000 + u16::from(tile) * 16 + u16::from(line) * 2;
            let palette = if attributes & 0x10 != 0 {
                self.obp1
            } else {
                self.obp0
            };
            for col in 0..8u8 {
                let x = left + i16::from(col);
                let Ok(x) = usize::try_from(x) else { continue };
                if x >= WIDTH {
                    continue;
                }
                let bit = if attributes & 0x20 != 0 { col } else { 7 - col };
                let color = self.row_pixel(tile_address, bit);
                if color == 0 {
                    continue; // tembus pandang
                }
                if attributes & 0x80 != 0 && self.line_colors[x] != 0 {
                    continue; // di belakang BG/window yang bukan warna 0
                }
                self.framebuffer[row + x] = (palette >> (color * 2)) & 0b11;
            }
        }
    }

    /// Nomor warna 0–3 dari satu baris tile di `address` (2 byte), bit `bit`
    /// (7 = piksel paling kiri).
    fn row_pixel(&self, address: u16, bit: u8) -> u8 {
        let lo = self.read_vram(address);
        let hi = self.read_vram(address + 1);
        (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1)
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
        self.row_pixel(base + u16::from(line) * 2, 7 - col)
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
        // Palet yang memetakan warna 0 ke tingkat 3: kalau BG mati tetap lewat
        // palet, layar akan gelap. Hardware: putih (tingkat 0).
        ppu.write_register(0xFF47, 0b00_00_00_11);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..WIDTH], &[0; WIDTH]);
    }

    // ------------------------------------------------------------------
    // Sprite (OAM) dan window
    // ------------------------------------------------------------------

    /// PPU dengan LCD, BG, dan sprite menyala, palet identitas, dan tile 2
    /// (0x8020) yang setiap barisnya: kiri 4 piksel warna 1, kanan 4 warna 3.
    fn ppu_for_sprites() -> Ppu {
        let mut ppu = Ppu::new();
        ppu.write_register(0xFF40, 0x93); // LCD, tile 0x8000, sprite, BG
        ppu.write_register(0xFF47, 0b11_10_01_00);
        ppu.write_register(0xFF48, 0b11_10_01_00); // OBP0
        ppu.write_register(0xFF49, 0b00_01_10_11); // OBP1 (dibalik)
        for line in 0..8u16 {
            ppu.write_vram(0x8020 + line * 2, 0b1111_1111); // bit rendah
            ppu.write_vram(0x8021 + line * 2, 0b0000_1111); // bit tinggi
        }
        ppu
    }

    /// Menaruh sprite nomor `index` di posisi layar (`x`, `y`).
    fn place(ppu: &mut Ppu, index: u16, x: i16, y: i16, tile: u8, attributes: u8) {
        let base = 0xFE00 + index * 4;
        ppu.write_oam(base, u8::try_from(y + 16).unwrap_or(0));
        ppu.write_oam(base + 1, u8::try_from(x + 8).unwrap_or(0));
        ppu.write_oam(base + 2, tile);
        ppu.write_oam(base + 3, attributes);
    }

    #[test]
    fn sprite_draws_with_obp0() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, 0, 0, 2, 0);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[1, 1, 1, 1, 3, 3, 3, 3]);
    }

    #[test]
    fn sprite_color_zero_is_transparent() {
        let mut ppu = ppu_for_sprites();
        ppu.write_register(0xFF47, 0b11_10_01_10); // BG warna 0 → tingkat 2
        // Tile 3: hanya 2 piksel kiri yang berwarna (warna 1).
        ppu.write_vram(0x8030, 0b1100_0000);
        place(&mut ppu, 0, 0, 0, 3, 0);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..4], &[1, 1, 2, 2]);
    }

    #[test]
    fn sprite_attribute_bit_4_selects_obp1() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, 0, 0, 2, 0x10);
        run(&mut ppu, 456);
        // OBP1 dibalik: warna 1 → 2, warna 3 → 0.
        assert_eq!(&ppu.framebuffer()[..8], &[2, 2, 2, 2, 0, 0, 0, 0]);
    }

    #[test]
    fn sprite_x_flip_mirrors_horizontally() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, 0, 0, 2, 0x20);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[3, 3, 3, 3, 1, 1, 1, 1]);
    }

    #[test]
    fn sprite_y_flip_mirrors_vertically() {
        let mut ppu = ppu_for_sprites();
        ppu.write_vram(0x802E, 0b1000_0000); // baris 7: hanya piksel kiri warna 1
        ppu.write_vram(0x802F, 0);
        place(&mut ppu, 0, 0, 0, 2, 0x40); // dibalik: baris 7 tampil di baris 0
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..3], &[1, 0, 0]);
    }

    #[test]
    fn sprite_behind_background_only_shows_over_color_zero() {
        let mut ppu = ppu_for_sprites();
        // BG: tile 1 di kolom 0 (warna 2 di setiap piksel, beda dari warna
        // sprite), kolom 1 kosong.
        for line in 0..8u16 {
            ppu.write_vram(0x8011 + line * 2, 0xFF);
        }
        ppu.write_vram(0x9800, 1);
        place(&mut ppu, 0, 4, 0, 2, 0x80); // menutupi kolom 4–11
        run(&mut ppu, 456);
        // Kolom 4–7: BG warna 2 → sprite (warna 1) tersembunyi.
        // Kolom 8–11: BG warna 0 → sprite tampil (warna 3 di bagian kanan).
        assert_eq!(&ppu.framebuffer()[4..12], &[2, 2, 2, 2, 3, 3, 3, 3]);
    }

    #[test]
    fn only_ten_sprites_per_line() {
        let mut ppu = ppu_for_sprites();
        for i in 0..11u16 {
            place(&mut ppu, i, i16::try_from(i).unwrap_or(0) * 10, 0, 2, 0);
        }
        run(&mut ppu, 456);
        assert_eq!(ppu.framebuffer()[90], 1, "sprite ke-10 tampil");
        assert_eq!(ppu.framebuffer()[100], 0, "sprite ke-11 tidak tampil");
    }

    #[test]
    fn smaller_x_wins_when_sprites_overlap() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, 4, 0, 2, 0x10); // OAM lebih awal, X = 4, OBP1
        place(&mut ppu, 1, 2, 0, 2, 0); // OAM lebih akhir, X = 2, OBP0
        run(&mut ppu, 456);
        // Kolom 4–5: sprite 1 (X lebih kecil) menang → warna 1 lewat OBP0.
        assert_eq!(&ppu.framebuffer()[4..6], &[1, 1]);
    }

    #[test]
    fn same_x_lower_oam_index_wins() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, 0, 0, 2, 0); // OBP0
        place(&mut ppu, 1, 0, 0, 2, 0x10); // OBP1, X sama
        run(&mut ppu, 456);
        assert_eq!(ppu.framebuffer()[0], 1, "sprite 0 (OBP0) menang");
    }

    #[test]
    fn tall_sprites_use_the_odd_tile_for_the_lower_half() {
        let mut ppu = ppu_for_sprites();
        ppu.write_register(0xFF40, 0x97); // + sprite 8×16
        ppu.write_vram(0x8030, 0b1000_0000); // tile 3 baris 0: piksel kiri warna 1
        // Tile 3 (ganjil) dibulatkan ke pasangan 2/3: tile 2 di atas layar,
        // tile 3 di baris 0. Tanpa pembulatan, baris 0 jatuh ke tile 4 (kosong).
        place(&mut ppu, 0, 0, -8, 3, 0);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..2], &[1, 0]);
    }

    #[test]
    fn sprites_hidden_when_lcdc_bit_1_is_off() {
        let mut ppu = ppu_for_sprites();
        ppu.write_register(0xFF40, 0x91);
        place(&mut ppu, 0, 0, 0, 2, 0);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[0; 8]);
    }

    #[test]
    fn sprite_partly_off_the_left_edge() {
        let mut ppu = ppu_for_sprites();
        place(&mut ppu, 0, -4, 0, 2, 0); // hanya 4 piksel kanan yang terlihat
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..5], &[3, 3, 3, 3, 0]);
    }

    /// PPU dengan window menyala di peta 0x9C00; tile 1 = warna 3 penuh.
    fn ppu_for_window(wx: u8, wy: u8) -> Ppu {
        let mut ppu = Ppu::new();
        // LCD, window di peta 0x9C00, window on, tile data 0x8800 (bertanda), BG on.
        ppu.write_register(0xFF40, 0xE1);
        ppu.write_register(0xFF47, 0b11_10_01_00);
        for line in 0..8u16 {
            ppu.write_vram(0x9000 + 16 + line * 2, 0xFF);
            ppu.write_vram(0x9000 + 17 + line * 2, 0xFF);
        }
        for column in 0..32u16 {
            ppu.write_vram(0x9C00 + column, 1);
        }
        ppu.write_register(0xFF4B, wx);
        ppu.write_register(0xFF4A, wy);
        ppu
    }

    #[test]
    fn window_starts_at_wx_minus_7() {
        let mut ppu = ppu_for_window(7 + 20, 0);
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[18..22], &[0, 0, 3, 3]);
    }

    #[test]
    fn window_waits_for_wy() {
        let mut ppu = ppu_for_window(7, 2);
        run(&mut ppu, 2 * 456);
        assert_eq!(ppu.framebuffer()[WIDTH], 0, "baris 1: belum ada window");
        run(&mut ppu, 456);
        assert_eq!(ppu.framebuffer()[2 * WIDTH], 3, "baris 2: window mulai");
    }

    #[test]
    fn window_off_when_lcdc_bit_5_is_clear() {
        let mut ppu = ppu_for_window(7, 0);
        ppu.write_register(0xFF40, 0xC1); // sama, tetapi bit 5 (window) mati
        run(&mut ppu, 456);
        assert_eq!(&ppu.framebuffer()[..8], &[0; 8]);
    }

    #[test]
    fn window_line_counter_only_advances_when_drawn() {
        // Tile window: baris 0 dan 2 warna 3, baris 1 kosong. Jika penghitung
        // ikut naik saat window tersembunyi, baris layar 2 mengambil baris 2.
        let mut ppu = ppu_for_window(7, 0);
        ppu.write_vram(0x9000 + 16 + 2, 0);
        ppu.write_vram(0x9000 + 17 + 2, 0);
        run(&mut ppu, 456); // baris 0 layar = baris 0 window (warna 3)
        ppu.write_register(0xFF4B, 200); // sembunyikan window (WX > 166)
        run(&mut ppu, 456); // baris 1: window tidak digambar
        ppu.write_register(0xFF4B, 7);
        run(&mut ppu, 456); // baris 2 layar = baris 1 window (kosong)
        assert_eq!(ppu.framebuffer()[0], 3);
        assert_eq!(
            ppu.framebuffer()[2 * WIDTH],
            0,
            "lanjut dari baris 1, bukan 2"
        );
    }
}
