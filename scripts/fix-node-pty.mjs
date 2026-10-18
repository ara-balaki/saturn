// node-pty 1.1.0 ships its macOS spawn-helper without the execute bit, which makes every
// spawn fail with "posix_spawnp failed" (node-pty issue #850).
import { chmodSync, existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const prebuilds = join(import.meta.dirname, '..', 'node_modules', 'node-pty', 'prebuilds');
if (existsSync(prebuilds)) {
  for (const dir of readdirSync(prebuilds)) {
    const helper = join(prebuilds, dir, 'spawn-helper');
    if (existsSync(helper)) chmodSync(helper, 0o755);
  }
}
