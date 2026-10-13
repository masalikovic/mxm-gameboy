/*
 * halo — program C pertama untuk mxm-gameboy (M3, ADR-0004).
 *
 * Program ini juga sebuah TES. Ia mencetak teks lewat port serial (bukan
 * layar, karena PPU baru dibangun di M4) dan diakhiri "Passed" atau "Failed",
 * sama seperti ROM uji Blargg. Dengan begitu gb-headless dan CI bisa menilai
 * hasilnya tanpa manusia.
 *
 * Yang ikut teruji di emulator kita:
 *   - startup GBDK (crt0) yang menyiapkan stack, RAM, dan interrupt;
 *   - pemanggilan fungsi C, loop, dan variabel 8/16-bit;
 *   - rutin pembagian/modulo SDCC (SM83 tidak punya instruksi bagi);
 *   - register hardware yang dipetakan ke memori (SB 0xFF01, SC 0xFF02).
 */
#include <gb/gb.h>
#include <stdint.h>

/* Kirim satu karakter lewat port serial, lalu tunggu transfernya selesai. */
static void serial_putc(char c)
{
    SB_REG = c;     /* byte yang akan dikirim             */
    SC_REG = 0x81;  /* mulai transfer, pakai jam internal */
    while (SC_REG & 0x80) {
        /* bit 7 SC mati = transfer selesai */
    }
}

static void serial_puts(const char *text)
{
    while (*text) {
        serial_putc(*text++);
    }
}

/* Cetak angka 16-bit dalam desimal, tanpa printf (latihan pembagian). */
static void serial_put_u16(uint16_t value)
{
    char digits[5];
    uint8_t count = 0;
    do {
        digits[count++] = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    while (count != 0) {
        serial_putc(digits[--count]);
    }
}

/* 1 + 2 + ... + n, dengan loop biasa (bukan rumus). */
static uint16_t sum_to(uint8_t n)
{
    uint16_t total = 0;
    for (uint8_t i = 1; i <= n; i++) {
        total += i;
    }
    return total;
}

void main(void)
{
    uint16_t total = sum_to(100);

    serial_puts("Halo dari C!\n");
    serial_puts("1 + 2 + ... + 100 = ");
    serial_put_u16(total);
    serial_putc('\n');

    /* Jawaban yang benar sudah diketahui: 100 * 101 / 2 = 5050. */
    serial_puts(total == 5050 ? "Passed\n" : "Failed\n");

    while (1) {
        /* selesai; Game Boy tidak punya "keluar dari program" */
    }
}
