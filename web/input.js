// Tombol Game Boy dari sentuhan dan keyboard.
//
// Modul ini hanya menghasilkan satu angka: bit tombol yang sedang ditekan
// (sama dengan gb-core/src/joypad.rs: kanan 1, kiri 2, atas 4, bawah 8,
// A 16, B 32, Select 64, Start 128). main.js meneruskan angka itu ke
// Emulator.setButtons. Emulator tidak tahu apa itu jari atau keyboard,
// dan modul ini tidak tahu apa itu register P1 — itulah batas adapter.

const RIGHT = 1;
const LEFT = 2;
const UP = 4;
const DOWN = 8;

// KeyboardEvent.code (posisi tombol fisik, tidak tergantung layout).
const KEYS = {
  ArrowRight: RIGHT, KeyD: RIGHT,
  ArrowLeft: LEFT, KeyA: LEFT,
  ArrowUp: UP, KeyW: UP,
  ArrowDown: DOWN, KeyS: DOWN,
  KeyX: 16, KeyK: 16,        // A
  KeyZ: 32, KeyJ: 32,        // B
  ShiftLeft: 64, ShiftRight: 64, Backspace: 64, // Select
  Enter: 128,                // Start
};

// tan(22,5°): D-pad dibagi 8 arah sama besar, termasuk diagonal.
const DIAGONAL = 0.414;

/**
 * Memasang sentuhan (D-pad + tombol) dan keyboard. `onChange(mask)`
 * dipanggil setiap kali kombinasi tombol berubah.
 */
export function attachInput({ dpad, buttons, onChange }) {
  let keyboard = 0;
  const pointers = new Map(); // pointerId → bit tombol di bawah jari itu
  let current = 0;

  function update() {
    let mask = keyboard;
    for (const bits of pointers.values()) mask |= bits;
    if (mask === current) return;
    if (mask & ~current && pointers.size > 0) navigator.vibrate?.(8);
    current = mask;
    // Tombol di layar ikut menyala, juga saat ditekan dari keyboard.
    for (const element of [...dpad.querySelectorAll('[data-button]'), ...buttons]) {
      element.classList.toggle('pressed', (mask & Number(element.dataset.button)) !== 0);
    }
    onChange(mask);
  }

  /** Arah dari posisi jari relatif ke tengah D-pad. */
  function direction(event) {
    const box = dpad.getBoundingClientRect();
    const dx = event.clientX - (box.left + box.width / 2);
    const dy = event.clientY - (box.top + box.height / 2);
    if (Math.hypot(dx, dy) < box.width * 0.12) return 0; // zona mati di tengah
    let bits = 0;
    if (Math.abs(dx) >= Math.abs(dy) * DIAGONAL) bits |= dx > 0 ? RIGHT : LEFT;
    if (Math.abs(dy) >= Math.abs(dx) * DIAGONAL) bits |= dy > 0 ? DOWN : UP;
    return bits;
  }

  // Satu jari di D-pad boleh bergeser dari kiri ke atas tanpa diangkat.
  dpad.addEventListener('pointerdown', (event) => {
    event.preventDefault();
    dpad.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, direction(event));
    update();
  });
  dpad.addEventListener('pointermove', (event) => {
    if (!pointers.has(event.pointerId)) return;
    pointers.set(event.pointerId, direction(event));
    update();
  });

  for (const button of buttons) {
    button.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      pointers.set(event.pointerId, Number(button.dataset.button));
      update();
    });
  }

  // Jari diangkat / dibatalkan sistem (mis. notifikasi muncul).
  for (const element of [dpad, ...buttons]) {
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) {
      element.addEventListener(type, (event) => {
        if (pointers.delete(event.pointerId)) update();
      });
    }
    // Tekan lama tidak boleh memunculkan menu salin/simpan.
    element.addEventListener('contextmenu', (event) => event.preventDefault());
  }

  window.addEventListener('keydown', (event) => {
    const bit = KEYS[event.code];
    // Panah di dalam <select> tetap untuk memilih demo.
    if (!bit || event.target instanceof HTMLSelectElement) return;
    event.preventDefault(); // panah/spasi tidak menggulung halaman
    keyboard |= bit;
    update();
  });
  window.addEventListener('keyup', (event) => {
    const bit = KEYS[event.code];
    if (!bit) return;
    keyboard &= ~bit;
    update();
  });

  // Tab disembunyikan atau jendela kehilangan fokus: keyup/pointerup bisa
  // tidak pernah datang, jadi lepaskan semua tombol supaya tidak "macet".
  function releaseAll() {
    keyboard = 0;
    pointers.clear();
    update();
  }
  window.addEventListener('blur', releaseAll);
  document.addEventListener('visibilitychange', releaseAll);

  return { get mask() { return current; } };
}
