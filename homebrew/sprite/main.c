/*
 * sprite — program C ketiga: sprite dan window (M5).
 *
 * Layar terdiri dari tiga lapisan yang digambar PPU kita:
 *   1. background — teks dari printf(),
 *   2. window     — bilah di bawah layar (move_win, SHOW_WIN),
 *   3. sprite     — huruf F 8×8: biasa, cermin X, cermin Y, palet OBP1.
 *
 * Tesnya golden test: setelah 60 frame, hash isi layar harus sama dengan
 * homebrew/sprite/screen.hash. Serial tetap mencetak "Passed".
 */
#include <gb/gb.h>
#include <stdio.h>

/* Huruf "F" 8×8: garis warna 3 (#), isi warna 1 (1). Bentuknya tidak
 * simetris ke kiri-kanan maupun atas-bawah, jadi cermin X dan Y terlihat.
 * Format 2bpp GB: per baris dua byte (bit rendah, bit tinggi). */
static const unsigned char letter_f[16] = {
    0xFC, 0xFC, /* ######.. */
    0xFC, 0x84, /* #1111#.. */
    0xFC, 0xBC, /* #1####.. */
    0xE0, 0xA0, /* #1#..... */
    0xF8, 0xB8, /* #1###... */
    0xE0, 0xA0, /* #1#..... */
    0xE0, 0xA0, /* #1#..... */
    0xE0, 0xE0, /* ###..... */
};

/* Tile window: blok warna 2 penuh (bit tinggi saja). */
static const unsigned char block[16] = {
    0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF,
    0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF,
};

static unsigned char bar[20];

static void serial_puts(const char *text)
{
    while (*text) {
        SB_REG = *text++;
        SC_REG = 0x81;
        while (SC_REG & 0x80) {
        }
    }
}

void main(void)
{
    unsigned char i;

    printf("\n  MXM GAME BOY\n\n");
    printf("  M5: sprite\n");
    printf("  + window\n");

    /* Window: tile 0x80 di area tile data bersama, satu baris penuh. */
    set_bkg_data(0x80, 1, block);
    for (i = 0; i < 20; i++) {
        bar[i] = 0x80;
    }
    set_win_tiles(0, 0, 20, 1, bar);
    move_win(7, 128); /* WX - 7 = kolom 0, mulai baris layar 128 */

    /* Sprite: satu tile, empat salinan dengan atribut berbeda.
     * move_sprite memakai koordinat OAM (X + 8, Y + 16). */
    OBP1_REG = 0x1B; /* palet terbalik: warna 3 jadi putih, dst. */
    set_sprite_data(0, 1, letter_f);
    for (i = 0; i < 4; i++) {
        set_sprite_tile(i, 0);
        move_sprite(i, 8 + 24 + i * 24, 16 + 80);
    }
    set_sprite_prop(1, S_FLIPX);
    set_sprite_prop(2, S_FLIPY);
    set_sprite_prop(3, S_PALETTE);

    SHOW_BKG;
    SHOW_WIN;
    SHOW_SPRITES;

    serial_puts("Passed\n");
    while (1) {
        vsync();
    }
}
