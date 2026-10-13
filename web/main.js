// Jembatan halaman ↔ WebAssembly.
//
// `./pkg/gb_web.js` dibuat oleh wasm-bindgen (tools/build-web.sh) dan tidak
// di-commit. Fungsi `greeting` dan `coreVersion` berasal dari crates/gb-web,
// yang meneruskan panggilan ke gb-core. JavaScript di sini hanya menampilkan.
import init, { greeting, coreVersion } from './pkg/gb_web.js';

const screenText = document.getElementById('screen-text');
const status = document.getElementById('status');

try {
  await init(); // mengunduh dan menyiapkan gb_web_bg.wasm
  screenText.textContent = greeting();
  status.textContent = `gb-core ${coreVersion()} · WebAssembly aktif`;
} catch (error) {
  screenText.textContent = 'Gagal memuat emulator';
  status.textContent = `Error: ${error.message ?? error}`;
  console.error(error);
}
