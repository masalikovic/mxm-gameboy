// Debugger: register CPU, instruksi berikutnya, jeda/langkah (M8).
//
// Semua isi berasal dari gb-web (registers, disassembly, stepInstruction);
// modul ini hanya menggambar teks dan meneruskan tombol ke main.js.
// Saat panel tertutup, tidak ada yang dihitung (tidak membebani HP).

const hex = (value, digits) => value.toString(16).toUpperCase().padStart(digits, '0');

export function createDebugger({ panel, registersView, codeView, buttons, actions }) {
  /** Menggambar ulang panel dari keadaan emulator saat ini. */
  function render(emulator, paused) {
    buttons.pause.textContent = paused ? 'LANJUT' : 'JEDA';
    buttons.step.disabled = !paused;
    buttons.frame.disabled = !paused;
    if (!panel.open || !emulator) return;
    const [af, bc, de, hl, sp, pc, status] = emulator.registers();
    const f = af & 0xff;
    const flag = (bit, name) => ((f >> bit) & 1 ? name : '-');
    registersView.textContent =
      `AF ${hex(af, 4)}  BC ${hex(bc, 4)}  DE ${hex(de, 4)}  HL ${hex(hl, 4)}\n` +
      `SP ${hex(sp, 4)}  PC ${hex(pc, 4)}  ` +
      `${flag(7, 'Z')}${flag(6, 'N')}${flag(5, 'H')}${flag(4, 'C')}  ` +
      `IME ${status & 1}  HALT ${(status >> 1) & 1}`;
    codeView.textContent = emulator
      .disassembly(8)
      .map((line, i) => (i === 0 ? '▶ ' : '  ') + line)
      .join('\n');
  }

  buttons.pause.addEventListener('click', actions.togglePause);
  buttons.step.addEventListener('click', actions.step);
  buttons.frame.addEventListener('click', actions.frame);
  panel.addEventListener('toggle', actions.refresh);

  return { render };
}
