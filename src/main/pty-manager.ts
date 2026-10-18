import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { promisify } from 'node:util';

import { spawn, type IPty } from 'node-pty';

import type { PtyCreateOptions, PtyInfo } from '@shared/ipc';
import { shellEnv } from '@shared/shell-env';

const run = promisify(execFile);

// Output is collected and flushed on a short timer, so a busy terminal sends one IPC message per
// frame instead of one per write.
const FLUSH_MS = 8;

// Flow control: when the renderer falls this far behind drawing a terminal's output, the pty is
// paused until it catches up, so one runaway program can't flood the app.
const HIGH_WATER = 512 * 1024;
const LOW_WATER = 128 * 1024;

type Sink = {
  data: (id: string, data: string) => void;
  exit: (id: string, exitCode: number) => void;
};

type Session = {
  pty: IPty;
  pending: string;
  timer: NodeJS.Timeout | undefined;
  unacked: number;
  paused: boolean;
};

/** The working directory of a process, read with lsof (macOS has no /proc). */
const processCwd = async (pid: number): Promise<string | undefined> => {
  try {
    const { stdout } = await run('lsof', ['-a', '-p', String(pid), '-d', 'cwd', '-Fn']);
    return stdout
      .split('\n')
      .find((line) => line.startsWith('n'))
      ?.slice(1);
  } catch {
    return undefined;
  }
};

export const createPtyManager = (sink: Sink) => {
  const sessions = new Map<string, Session>();

  const flush = (id: string) => {
    const session = sessions.get(id);
    if (!session) return;
    session.timer = undefined;
    if (session.pending === '') return;
    const data = session.pending;
    session.pending = '';
    session.unacked += data.length;
    if (!session.paused && session.unacked > HIGH_WATER) {
      session.paused = true;
      session.pty.pause();
    }
    sink.data(id, data);
  };

  const create = ({ cwd, cols, rows }: PtyCreateOptions): string => {
    const id = randomUUID();
    const shell = process.env.SHELL ?? '/bin/zsh';
    const pty = spawn(shell, ['-l'], {
      name: 'xterm-256color',
      cols,
      rows,
      cwd: cwd ?? homedir(),
      env: shellEnv(process.env, {
        TERM: 'xterm-256color',
        COLORTERM: 'truecolor',
        TERM_PROGRAM: 'Saturn',
        SATURN_PANE_ID: id,
      }),
    });
    const session: Session = { pty, pending: '', timer: undefined, unacked: 0, paused: false };
    sessions.set(id, session);

    pty.onData((data) => {
      session.pending += data;
      session.timer ??= setTimeout(() => flush(id), FLUSH_MS);
    });
    pty.onExit(({ exitCode }) => {
      flush(id);
      sessions.delete(id);
      sink.exit(id, exitCode);
    });
    return id;
  };

  const ack = (id: string, length: number) => {
    const session = sessions.get(id);
    if (!session) return;
    session.unacked = Math.max(0, session.unacked - length);
    if (session.paused && session.unacked < LOW_WATER) {
      session.paused = false;
      session.pty.resume();
    }
  };

  const info = async (id: string): Promise<PtyInfo | undefined> => {
    const session = sessions.get(id);
    if (!session) return undefined;
    return { process: session.pty.process, cwd: await processCwd(session.pty.pid) };
  };

  const write = (id: string, data: string) => sessions.get(id)?.pty.write(data);

  const resize = (id: string, cols: number, rows: number) => {
    if (cols > 0 && rows > 0) sessions.get(id)?.pty.resize(cols, rows);
  };

  const kill = (id: string) => sessions.get(id)?.pty.kill();

  /**
   * Stops every shell and resolves once all have exited. The app must not quit before that: a pty
   * that reports its exit while Electron tears Node down aborts the whole process.
   */
  const shutdown = async (graceMs = 1500) => {
    const exited = () => sessions.size === 0;
    const waitFor = (ms: number) =>
      new Promise<void>((resolve) => {
        const started = Date.now();
        const poll = setInterval(() => {
          if (exited() || Date.now() - started > ms) {
            clearInterval(poll);
            resolve();
          }
        }, 20);
      });
    for (const session of sessions.values()) session.pty.kill();
    await waitFor(graceMs);
    for (const session of sessions.values()) session.pty.kill('SIGKILL');
    await waitFor(graceMs);
  };

  return { create, write, resize, kill, ack, info, shutdown };
};
