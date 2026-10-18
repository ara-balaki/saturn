// Phase 0 harness: launch the built app with a grid of panes, let them run, take a screenshot.
// Usage: node spikes/launch.mts --panes 4 --command 'echo hi' --cwd ~/x --seconds 5 --out name
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import { _electron as electron } from 'playwright';

const { values } = parseArgs({
  options: {
    panes: { type: 'string', default: '4' },
    command: { type: 'string' },
    cwd: { type: 'string' },
    seconds: { type: 'string', default: '5' },
    out: { type: 'string', default: 'launch' },
  },
});

const root = resolve(import.meta.dirname, '..');
const logs = join(root, 'spikes', 'logs');
mkdirSync(logs, { recursive: true });

const app = await electron.launch({
  args: [join(root, 'out', 'main', 'index.js')],
  env: { ...process.env, SATURN_LOG_FILE: join(logs, `${values.out}.jsonl`) },
});
const page = await app.firstWindow();
page.on('console', (message) => console.log(`[renderer ${message.type()}] ${message.text()}`));
page.on('pageerror', (error) => console.log(`[renderer error] ${error.message}`));
app
  .process()
  .stdout?.on('data', (chunk: Buffer) => process.stdout.write(`[main] ${chunk.toString()}`));
app
  .process()
  .stderr?.on('data', (chunk: Buffer) => process.stdout.write(`[main] ${chunk.toString()}`));
const query = new URLSearchParams({ panes: values.panes });
if (values.command) query.set('command', values.command);
if (values.cwd) query.set('cwd', values.cwd);
await page.goto(`${page.url().split('?')[0]}?${query.toString()}`);

const started = Date.now();
// Frame pacing of the renderer while the panes run: long frames are what makes typing feel slow.
const frames = await page.evaluate(
  async (ms) => {
    const deltas: number[] = [];
    let last = performance.now();
    const end = last + ms;
    await new Promise<void>((done) => {
      const tick = (now: number) => {
        deltas.push(now - last);
        last = now;
        if (now < end) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    deltas.sort((a, b) => a - b);
    const at = (q: number) => Math.round(deltas[Math.floor(q * (deltas.length - 1))] ?? 0);
    return {
      count: deltas.length,
      p50: at(0.5),
      p95: at(0.95),
      max: at(1),
      over50ms: deltas.filter((d) => d > 50).length,
    };
  },
  Number(values.seconds) * 1000,
);
const metrics = await app.evaluate(({ app: electronApp }) =>
  electronApp.getAppMetrics().map((metric) => ({
    type: metric.type,
    cpu: Math.round(metric.cpu.percentCPUUsage),
    memoryMB: Math.round(metric.memory.workingSetSize / 1024),
  })),
);
await page.screenshot({ path: join(logs, `${values.out}.png`) });
console.log(JSON.stringify({ seconds: (Date.now() - started) / 1000, frames, metrics }, null, 2));
await app.close();
