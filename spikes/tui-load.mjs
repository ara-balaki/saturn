// Phase 0 load generator: redraws the whole screen like a busy TUI (Claude streaming with a
// spinner) at a fixed frame rate. Usage: node spikes/tui-load.mjs <fps> <seconds>
const fps = Number(process.argv[2] ?? 30);
const seconds = Number(process.argv[3] ?? 10);
const spinner = ['·', '✢', '✳', '✶', '✻', '✽'];
const words = ['permission', 'worktree', 'stocktake', 'pipeline', 'review', 'ticket', 'branch'];
let frame = 0;

const draw = () => {
  const { columns = 80, rows = 24 } = process.stdout;
  let out = '\u001b[?2026h\u001b[H';
  for (let y = 0; y < rows - 1; y++) {
    let line = `\u001b[3${(y + frame) % 7}m${spinner[frame % spinner.length]} `;
    while (line.length < columns) line += `${words[(y * 7 + frame + line.length) % words.length]} `;
    out += `${line.slice(0, columns)}\u001b[0m\u001b[K\r\n`;
  }
  out += `frame ${frame}\u001b[K\u001b[?2026l`;
  process.stdout.write(out);
  frame++;
};

const timer = setInterval(draw, 1000 / fps);
setTimeout(() => {
  clearInterval(timer);
  process.stdout.write('\r\ndone\r\n');
}, seconds * 1000);
