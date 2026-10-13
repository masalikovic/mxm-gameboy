// Jembatan halaman ↔ WebAssembly.
//
// `./pkg/gb_web.js` dibuat oleh wasm-bindgen (tools/build-web.sh) dan tidak
// di-commit. `Emulator` berasal dari crates/gb-web, yang meneruskan panggilan
// ke gb-core. JavaScript di sini hanya mengatur waktu dan menggambar.
//
// Penanda versi pada URL di bawah diganti versi build oleh tools/build-web.sh (ADR-0009):
// URL berubah setiap build, jadi browser tidak memakai file lama dari cache.
import init, { coreVersion, Emulator } from './pkg/gb_web.js?v=__BUILD__';

const wasmUrl = new URL('./pkg/gb_web_bg.wasm?v=__BUILD__', import.meta.url);
// ROM demo: dibuild dari homebrew/layar/main.c di CI (ADR-0006).
const demoRomUrl = new URL('./roms/layar.gb?v=__BUILD__', import.meta.url);

// Versi build yang sedang tampil, supaya mudah dicek di HP tanpa membuka
// kode sumber. Contoh: "14bd9ac" (hash commit), atau "14bd9ac+lokal" kalau
// dibuild dengan perubahan yang belum di-commit.
const [commit, localStamp] = '__BUILD__'.split('-');
const buildLabel = commit.slice(0, 7) + (localStamp ? '+lokal' : '');

const WIDTH = 160;
const HEIGHT = 144;
// Satu frame Game Boy = 70.224 siklus pada 4.194.304 Hz ≈ 16,74 ms.
const FRAME_MS = (70224 / 4194304) * 1000;

const canvas = document.getElementById('lcd');
const context = canvas.getContext('2d');
const image = context.createImageData(WIDTH, HEIGHT);
const screenText = document.getElementById('screen-text');
const status = document.getElementById('status');
const build = document.getElementById('build');
const perf = document.getElementById('perf');
const romInput = document.getElementById('rom-input');

/** Palet 4 tingkat dari CSS (--lcd-0 gelap … --lcd-3 terang, ADR-0008). */
function readPalette() {
  const css = getComputedStyle(document.documentElement);
  const rgb = (name) => {
    const hex = css.getPropertyValue(name).trim().replace('#', '');
    return [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  };
  // Framebuffer: 0 = terang … 3 = gelap. CSS: --lcd-3 = terang … --lcd-0 = gelap.
  return [rgb('--lcd-3'), rgb('--lcd-2'), rgb('--lcd-1'), rgb('--lcd-0')];
}
const palette = readPalette();

let emulator = null;
let running = false;
let lastTime = 0;
let backlog = 0;
let emulationMs = 0;
let framesSinceReport = 0;

/** Framebuffer (tingkat 0–3) → piksel RGBA di canvas. */
function draw() {
  const shades = emulator.framebuffer();
  const data = image.data;
  for (let i = 0; i < shades.length; i++) {
    const [r, g, b] = palette[shades[i]];
    data[i * 4] = r;
    data[i * 4 + 1] = g;
    data[i * 4 + 2] = b;
    data[i * 4 + 3] = 255;
  }
  context.putImageData(image, 0, 0);
}

/**
 * Dipanggil browser setiap refresh layar (60/90/120 Hz). Game Boy berjalan
 * ±59,7 frame/detik, jadi jumlah frame yang dijalankan dihitung dari waktu
 * nyata yang lewat, bukan dari jumlah refresh layar HP.
 */
function tick(now) {
  if (!running) return;
  backlog += Math.min(now - lastTime, 100); // tab yang tertidur tidak "mengejar"
  lastTime = now;
  let ran = 0;
  while (backlog >= FRAME_MS && ran < 3) {
    const start = performance.now();
    try {
      emulator.runFrame();
    } catch (error) {
      stop(`Emulator berhenti: ${error}`);
      return;
    }
    emulationMs = emulationMs * 0.9 + (performance.now() - start) * 0.1;
    backlog -= FRAME_MS;
    ran++;
  }
  if (ran > 0) {
    draw();
    framesSinceReport += ran;
    if (framesSinceReport >= 30) {
      framesSinceReport = 0;
      perf.textContent = `emulasi ${emulationMs.toFixed(1)} ms/frame · target < 8 ms`;
    }
  }
  requestAnimationFrame(tick);
}

function stop(message) {
  running = false;
  screenText.textContent = message;
  screenText.hidden = false;
  status.textContent = message;
}

/** Memasang ROM dan mulai berjalan. */
function start(bytes, name) {
  running = false;
  emulator?.free();
  try {
    emulator = new Emulator(bytes);
  } catch (error) {
    emulator = null;
    stop(`${error}`);
    return;
  }
  screenText.hidden = true;
  status.textContent = `ROM: ${name}`;
  running = true;
  backlog = 0;
  lastTime = performance.now();
  requestAnimationFrame(tick);
}

romInput.addEventListener('change', async () => {
  const file = romInput.files?.[0];
  if (file) start(new Uint8Array(await file.arrayBuffer()), file.name);
});

try {
  await init({ module_or_path: wasmUrl }); // mengunduh dan menyiapkan .wasm
  build.textContent = `gb-core ${coreVersion()} · build ${buildLabel}`;
  const response = await fetch(demoRomUrl);
  if (response.ok) {
    start(new Uint8Array(await response.arrayBuffer()), 'layar.gb (demo, homebrew C)');
  } else {
    stop('ROM demo belum ada di build ini. Muat file .gb di bawah.');
  }
} catch (error) {
  stop(`Error: ${error.message ?? error}`);
  console.error(error);
}
