/*
 * layar — program C kedua: menulis teks ke LAYAR (M4).
 *
 * printf() milik GBDK menggambar huruf sebagai tile 8×8 di peta background.
 * Program ini menguji PPU kita: tile di VRAM, peta background, palet BGP,
 * dan pergantian frame (V-blank).
 *
 * Tesnya golden test: setelah 60 frame, hash isi layar harus sama dengan
 * homebrew/layar/screen.hash. Serial tetap mencetak "Passed" supaya
 * pemeriksaan biasa (tools/build-homebrew.sh) juga berjalan.
 */
#include <gb/gb.h>
#include <stdio.h>

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
    printf("\n  MXM GAME BOY\n\n");
    printf("  Halo dari layar!\n\n");
    printf("  gb-core + PPU\n");
    printf("  tile 8x8, BGP\n\n");
    printf("  M4: gambar\n  pertama");

    serial_puts("Passed\n");
    while (1) {
        vsync(); /* tunggu V-blank, seperti game sungguhan */
    }
}
