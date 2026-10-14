/*
 * jeruk — "Kejar Jeruk", game pertama yang bisa dimainkan (M5).
 *
 * Gerakkan robot dengan D-pad, ambil jeruk sebanyak mungkin. Rekor skor
 * disimpan di RAM cartridge berbaterai (M6): tetap ada setelah HP dimatikan.
 *   D-pad      bergerak 1 piksel per frame
 *   A atau B   (ditahan) berlari, 2 piksel per frame
 *   Start      jeda / lanjut
 *   Select     skor kembali ke 0
 *
 * Bunyi (M7): "pling" naik saat jeruk diambil (kanal 1 + sweep), dan nada
 * tinggi tambahan saat rekor baru (kanal 2).
 *
 * Yang diuji dari emulator kita: joypad (register P1), sprite (robot dan
 * jeruk, robot dicerminkan saat menghadap kiri), window (bilah skor di
 * bawah), background (judul dan pagar), dan RAM cartridge MBC1 berbaterai
 * (lcc.flags: -Wm-yt0x03 = MBC1+RAM+baterai, -Wm-ya1 = satu bank RAM 8 KiB).
 *
 * Tesnya golden test (homebrew/jeruk/screen.hash) dengan tombol ditahan:
 * tanpa tombol, tahan kanan, tahan kanan + A, dan tahan Start. Serial tetap
 * mencetak "Passed" supaya pemeriksaan biasa juga berjalan.
 */
#include <gb/gb.h>
#include <gbdk/console.h>
#include <stdio.h>

/* Gambar tile 8×8. Format 2bpp GB: per baris dua byte (bit rendah, bit
 * tinggi); angka di komentar = nomor warna 0–3 (titik = 0, # = 3). */
static const unsigned char robot[16] = {
    0x3C, 0x3C, /* ..####.. */
    0x42, 0x7E, /* .#2222#. */
    0x95, 0xFF, /* #22#2#2#   mata agak ke kanan: robot menghadap kanan */
    0x81, 0xFF, /* #222222# */
    0x9D, 0xFF, /* #22###2# */
    0x42, 0x7E, /* .#2222#. */
    0x24, 0x24, /* ..#..#.. */
    0x66, 0x66, /* .##..##. */
};

static const unsigned char orange[16] = {
    0x0C, 0x0C, /* ....33..   daun */
    0x10, 0x18, /* ...32... */
    0x3C, 0x3C, /* ..####.. */
    0x7E, 0x42, /* .#1111#. */
    0xFF, 0x81, /* #111111# */
    0xFF, 0x81, /* #111111# */
    0x7E, 0x42, /* .#1111#. */
    0x3C, 0x3C, /* ..####.. */
};

static const unsigned char fence[16] = {
    0x00, 0xFF, /* 22222222 */
    0x00, 0xAA, /* 2.2.2.2. */
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x55, /* .2.2.2.2 */
    0x00, 0xFF, /* 22222222 */
};

#define FENCE_TILE 0x80
#define PLAYER 0 /* nomor sprite */
#define FRUIT 1

/* Arena dalam koordinat layar (pojok kiri-atas sprite). Pagar di baris
 * tile 1 dan 16, bilah skor (window) di baris 17. */
#define MIN_X 0
#define MAX_X 152
#define MIN_Y 16
#define MAX_Y 120

static unsigned char px = 16, py = 72; /* robot */
static unsigned char ox = 64, oy = 72; /* jeruk: sejajar robot di awal */
static unsigned int score;
static unsigned int best; /* rekor, dibaca dari dan ditulis ke RAM cartridge */
static unsigned char paused;
static unsigned int rng = 0xACE1;
static unsigned char row[20];

/* RAM cartridge di A000–BFFF. Isinya acak (atau nol) di cartridge baru,
 * jadi rekor hanya dipercaya kalau diawali tanda "MXMJ". */
#define SAVE ((volatile unsigned char *)0xA000)
static const unsigned char magic[4] = {'M', 'X', 'M', 'J'};

static void load_best(void)
{
    unsigned char i;
    best = 0;
    ENABLE_RAM; /* tulis 0x0A ke 0000–1FFF: MBC menyalakan RAM */
    for (i = 0; i < 4; i++) {
        if (SAVE[i] != magic[i]) {
            DISABLE_RAM;
            return;
        }
    }
    best = SAVE[4] | (SAVE[5] << 8);
    DISABLE_RAM; /* RAM dimatikan lagi: aman kalau baterai lemah/HP mati */
}

static void save_best(void)
{
    unsigned char i;
    ENABLE_RAM;
    for (i = 0; i < 4; i++) {
        SAVE[i] = magic[i];
    }
    SAVE[4] = best & 0xFF;
    SAVE[5] = best >> 8;
    DISABLE_RAM;
}

static void serial_puts(const char *text)
{
    while (*text) {
        SB_REG = *text++;
        SC_REG = 0x81;
        while (SC_REG & 0x80) {
        }
    }
}

/* LFSR 16-bit: angka "acak" yang selalu sama untuk urutan input yang sama,
 * jadi golden test tetap stabil. */
static unsigned int next_random(void)
{
    unsigned int bit = ((rng >> 0) ^ (rng >> 2) ^ (rng >> 3) ^ (rng >> 5)) & 1;
    rng = (rng >> 1) | (bit << 15);
    return rng;
}

static unsigned char distance(unsigned char a, unsigned char b)
{
    return a > b ? a - b : b - a;
}

/* Bilah skor di window. printf() hanya bisa menulis ke background, jadi
 * teksnya ditulis dulu ke baris 17 background (tertutup window, tidak
 * pernah terlihat), lalu tile-nya disalin ke window. Dengan begitu kita
 * tidak perlu tahu nomor tile tiap huruf di font GBDK. */
static void draw_score(void)
{
    /* Di SDCC, argumen %c harus bertipe char (tidak dinaikkan ke int
     * seperti C standar). Tanpa cast, printf membaca argumen bergeser.
     * Teks 19 huruf mulai kolom 0: tidak sampai kolom terakhir, jadi
     * konsol GBDK tidak pindah baris dan tidak menggulung layar. */
    gotoxy(0, 17);
    if (paused) {
        printf("JERUK %c%c%c   JEDA   ", (char)('0' + score / 100 % 10),
               (char)('0' + score / 10 % 10), (char)('0' + score % 10));
    } else {
        printf("JERUK %c%c%c REKOR %c%c%c", (char)('0' + score / 100 % 10),
               (char)('0' + score / 10 % 10), (char)('0' + score % 10),
               (char)('0' + best / 100 % 10), (char)('0' + best / 10 % 10),
               (char)('0' + best % 10));
    }
    get_bkg_tiles(0, 17, 20, 1, row);
    set_win_tiles(0, 0, 20, 1, row);
}

/* Suara (APU). Register NRxx: lihat crates/gb-core/src/apu.rs. */
static void sound_init(void)
{
    NR52_REG = 0x80; /* APU menyala */
    NR50_REG = 0x77; /* volume kiri/kanan penuh */
    NR51_REG = 0xFF; /* semua kanal ke kiri dan kanan */
}

/* Kanal 1: "pling" yang meluncur naik. Nada = 131072 / (2048 − f) Hz.
 * Mulai f = 1798 (±524 Hz); setiap 1/128 detik sweep menambah f/64, jadi
 * nada naik ±590 → 680 → 790 → … Hz sampai f melewati 2047 dan kanal
 * berhenti sendiri (±60 ms). Makin dekat 2048, nada makin cepat melonjak:
 * titik awal yang terlalu tinggi membuat bunyi langsung mencicit. */
static void sound_pick(void)
{
    NR10_REG = 0x16; /* sweep: periode 1, naik, geser 6 */
    NR11_REG = 0x80; /* duty 50 % */
    NR12_REG = 0xF2; /* volume 15, turun, periode 2 */
    NR13_REG = 0x06; /* frekuensi 0x706 = 1798 */
    NR14_REG = 0x87; /* trigger + 3 bit atas frekuensi */
}

/* Kanal 2: nada lebih tinggi (±2 kHz) untuk rekor baru. */
static void sound_record(void)
{
    NR21_REG = 0x40; /* duty 25 % */
    NR22_REG = 0xA3; /* volume 10, turun, periode 3 */
    NR23_REG = 0xC1; /* frekuensi 0x7C1 = 1985 → ±2 kHz */
    NR24_REG = 0x87;
}

static void place_fruit(void)
{
    do {
        ox = MIN_X + 4 + (unsigned char)(next_random() % (MAX_X - MIN_X - 8));
        oy = MIN_Y + 4 + (unsigned char)(next_random() % (MAX_Y - MIN_Y - 8));
    } while (distance(ox, px) < 24 && distance(oy, py) < 24);
    move_sprite(FRUIT, ox + 8, oy + 16);
}

void main(void)
{
    unsigned char keys, previous = 0, pressed, speed, i;

    gotoxy(2, 0);
    printf("KEJAR JERUK MXM");

    set_bkg_data(FENCE_TILE, 1, fence);
    for (i = 0; i < 20; i++) {
        row[i] = FENCE_TILE;
    }
    set_bkg_tiles(0, 1, 20, 1, row);
    set_bkg_tiles(0, 16, 20, 1, row);

    set_sprite_data(0, 1, robot);
    set_sprite_data(1, 1, orange);
    set_sprite_tile(PLAYER, 0);
    set_sprite_tile(FRUIT, 1);
    move_sprite(PLAYER, px + 8, py + 16);
    move_sprite(FRUIT, ox + 8, oy + 16);

    sound_init();
    load_best();
    draw_score();
    move_win(7, 136);

    SHOW_BKG;
    SHOW_WIN;
    SHOW_SPRITES;

    serial_puts("Passed\n");

    while (1) {
        vsync();
        keys = joypad();
        pressed = keys & ~previous; /* hanya tombol yang BARU ditekan */
        previous = keys;
        rng ^= keys; /* urutan tombol pemain ikut mengacak posisi jeruk */

        if (pressed & J_START) {
            paused = !paused;
            draw_score();
        }
        if (pressed & J_SELECT) {
            score = 0;
            draw_score();
        }
        if (paused) {
            continue;
        }

        speed = (keys & (J_A | J_B)) ? 2 : 1;
        if (keys & J_RIGHT) {
            px = px + speed > MAX_X ? MAX_X : px + speed;
            set_sprite_prop(PLAYER, 0);
        }
        if (keys & J_LEFT) {
            px = px < MIN_X + speed ? MIN_X : px - speed;
            set_sprite_prop(PLAYER, S_FLIPX); /* menghadap kiri */
        }
        if (keys & J_DOWN) {
            py = py + speed > MAX_Y ? MAX_Y : py + speed;
        }
        if (keys & J_UP) {
            py = py < MIN_Y + speed ? MIN_Y : py - speed;
        }
        move_sprite(PLAYER, px + 8, py + 16);

        if (distance(px, ox) < 8 && distance(py, oy) < 8) {
            score++;
            sound_pick();
            if (score > best) {
                best = score;
                save_best();
                sound_record();
            }
            place_fruit();
            draw_score();
        }
    }
}
