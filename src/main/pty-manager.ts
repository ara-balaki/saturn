import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';

import { spawn, type IPty } from 'node-pty';

import type { PtyCreateOptions } from '@shared/ipc';

// Output is collected and flushed on a short timer, so a busy terminal sends one IPC message per
// frame instead of one per write.
const FLUSH_MS = 8;

type Sink = {
  data: (id: string, data: string) => void;
  exit: (id: string, exitCode: number) => void;
};

type Session = {
  pty: IPty;
  pending: string;
  timer: NodeJS.Timeout | undefined;
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
      env: {
        ...process.env,
        TERM: 'xterm-256color',
        COLORTERM: 'truecolor',
        TERM_PROGRAM: 'Saturn',
        SATURN_PANE_ID: id,
      },
    });
    const session: Session = { pty, pending: '', timer: undefined };
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

  const write = (id: string, data: string) => sessions.get(id)?.pty.write(data);

  const resize = (id: string, cols: number, rows: number) => {
    if (cols > 0 && rows > 0) sessions.get(id)?.pty.resize(cols, rows);
  };

  const kill = (id: string) => sessions.get(id)?.pty.kill();

  const killAll = () => {
    for (const session of sessions.values()) session.pty.kill();
  };

  return { create, write, resize, kill, killAll };
};
