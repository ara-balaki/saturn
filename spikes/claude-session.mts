// Phase 0 spike: run a real Claude Code session in a pty behind a headless xterm.js, the way a
// Saturn pane will, and record what it tells the terminal (OSC 0 titles, OSC 7501 status, bells)
// and the hooks, while it runs a ticket command, asks for a permission and gets interrupted.
//
// Usage: node spikes/claude-session.mts
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

import xtermHeadless from '@xterm/headless';
import { spawn } from 'node-pty';

const { Terminal } = xtermHeadless;
const root = resolve(import.meta.dirname, '..');
const logs = join(root, 'spikes', 'logs');
mkdirSync(logs, { recursive: true });
const hookLog = join(logs, 'claude-hooks.jsonl');
const eventLog = join(logs, 'claude-terminal.jsonl');
rmSync(hookLog, { force: true });
rmSync(join(logs, 'permission-test.txt'), { force: true });
writeFileSync(eventLog, '');

const started = Date.now();
const record = (event: string, detail: unknown) => {
  const line = { t: ((Date.now() - started) / 1000).toFixed(1), event, detail };
  writeFileSync(eventLog, `${JSON.stringify(line)}\n`, { flag: 'a' });
  console.log(line.t, event, typeof detail === 'string' ? detail : JSON.stringify(detail));
};

const cols = 120;
const rows = 40;
const terminal = new Terminal({ cols, rows, allowProposedApi: true });
const pty = spawn('claude', ['--model', 'haiku', '--permission-mode', 'default'], {
  name: 'xterm-256color',
  cols,
  rows,
  cwd: root,
  env: {
    // Markers of the Claude session this spike may be started from must not leak into the child.
    ...Object.fromEntries(
      Object.entries(process.env).filter(
        ([key]) => !/^(CLAUDECODE|CLAUDE_PID|CLAUDE_EFFORT|CLAUDE_CODE_)/.test(key),
      ),
    ),
    TERM: 'xterm-256color',
    COLORTERM: 'truecolor',
    TERM_PROGRAM: 'Saturn',
    CLAUDE_CODE_PLUGIN_DIRS: join(root, 'spikes', 'plugin'),
    SATURN_SPIKE_HOOK_LOG: hookLog,
  },
});

terminal.parser.registerOscHandler(7501, (data) => {
  if (data.startsWith('?')) {
    pty.write('\u001b]7501;?\u001b\\');
    record('osc7501-probe', data);
    return true;
  }
  const fields = Object.fromEntries(
    data.split(':').map((part) => {
      const at = part.indexOf('=');
      const key = part.slice(0, at);
      const value = part.slice(at + 1);
      return [
        key,
        key === 'title' || key === 'msg' ? Buffer.from(value, 'base64').toString() : value,
      ];
    }),
  );
  record('osc7501', fields);
  return true;
});
terminal.onTitleChange((title) => record('title', title));
terminal.onBell(() => record('bell', null));
// The emulator's replies (device attributes, cursor position...) go back to the program.
terminal.onData((data) => pty.write(data));
pty.onData((data) => terminal.write(data));
pty.onExit(({ exitCode }) => record('exit', exitCode));

const screen = () => {
  const buffer = terminal.buffer.active;
  const lines: string[] = [];
  for (let y = 0; y < buffer.length; y++)
    lines.push(buffer.getLine(y)?.translateToString(true) ?? '');
  return lines.join('\n').replace(/\n+$/, '');
};

const waitFor = async (label: string, test: () => boolean, seconds: number) => {
  const deadline = Date.now() + seconds * 1000;
  while (Date.now() < deadline) {
    if (test()) {
      record('reached', label);
      return true;
    }
    await sleep(250);
  }
  record('timeout', label);
  return false;
};

const type = async (text: string) => {
  for (const char of text) {
    pty.write(char);
    await sleep(15);
  }
};

const visibleTail = () => screen().split('\n').slice(-12).join('\n');

// 1. Startup: accept the folder-trust question if Claude asks it, then wait for the input box.
await sleep(3000);
if (screen().includes('Yes, I trust this folder')) {
  pty.write('\u001b[B');
  await sleep(300);
  pty.write('\r');
  record('trusted folder', root);
}
await waitFor('prompt ready', () => /\? for shortcuts|bypass|shift\+tab/i.test(screen()), 30);
await sleep(1500);

// 2. Shift+Enter: two lines in the input box, then clear it.
await type('line one');
pty.write('\u001b\r');
await type('line two');
await sleep(800);
record('shift-enter screen', visibleTail());
pty.write('\u0003');
await sleep(500);

// 3. A ticket command: does the hook's sessionTitle rename the session and the title?
await type('/saturn-spike:ticket abc-999');
await sleep(400);
pty.write('\r');
await waitFor('ticket command answered', () => /\bok\b/.test(screen()), 60);
await sleep(3000);
record('after ticket screen', visibleTail());

// 4. A permission prompt, then Esc to refuse it.
await type('Use the Bash tool to run exactly: touch spikes/logs/permission-test.txt');
pty.write('\r');
await waitFor(
  'permission prompt',
  () => /Do you want to proceed|Allow|permission/i.test(screen()),
  60,
);
await sleep(8000); // past the ~6s Notification(permission_prompt) delay
record('permission screen', visibleTail());
pty.write('\u001b');
await sleep(4000);
record('after esc screen', visibleTail());

// 5. Exit.
await type('/exit');
pty.write('\r');
await sleep(4000);
pty.kill();

const hooks = (existsSync(hookLog) ? readFileSync(hookLog, 'utf8') : '')
  .trim()
  .split('\n')
  .filter(Boolean)
  .map((line) => JSON.parse(line) as Record<string, unknown>);
record(
  'hook summary',
  hooks.map((hook) => ({
    event: hook.hook_event_name,
    type: hook.notification_type,
    tool: hook.tool_name,
    title: hook.session_title,
  })),
);
const sessionId = hooks[0]?.session_id;
record('session', { sessionId, transcript: hooks[0]?.transcript_path });
