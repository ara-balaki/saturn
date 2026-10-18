import { useEffect, useRef, useState } from 'react';

import {
  isProgramStatusProbe,
  parseProgramStatus,
  PROGRAM_STATUS_OSC,
  PROGRAM_STATUS_PROBE_REPLY,
  type ProgramStatus,
} from '@shared/program-status';

import { attachWebgl, createTerminal } from './create-terminal';

type TerminalPaneProps = {
  index: number;
  cwd?: string;
  command?: string;
};

// Claude Code reads ESC + CR as a newline in the prompt (what /terminal-setup configures for VS Code).
const SHIFT_ENTER = '\u001b\r';

export const TerminalPane = ({ index, cwd, command }: TerminalPaneProps) => {
  const host = useRef<HTMLDivElement>(null);
  const [title, setTitle] = useState('');
  const [status, setStatus] = useState<ProgramStatus>();
  const [bells, setBells] = useState(0);
  const [renderer, setRenderer] = useState('webgl');

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const { terminal, fit } = createTerminal();
    terminal.open(element);
    attachWebgl(terminal, () => setRenderer('dom'));
    fit.fit();

    let ptyId: string | undefined;
    let disposed = false;
    const log = (event: string, detail: unknown) =>
      window.saturn.log(event, { pane: index, detail });

    terminal.parser.registerOscHandler(PROGRAM_STATUS_OSC, (data) => {
      if (isProgramStatusProbe(data)) {
        if (ptyId) window.saturn.pty.write(ptyId, PROGRAM_STATUS_PROBE_REPLY);
        log('osc7501-probe', data);
        return true;
      }
      const parsed = parseProgramStatus(data);
      log('osc7501', parsed ?? data);
      if (parsed) setStatus(parsed);
      return true;
    });
    terminal.onTitleChange((next) => {
      setTitle(next);
      log('title', next);
    });
    terminal.onBell(() => {
      setBells((count) => count + 1);
      log('bell', null);
    });
    terminal.attachCustomKeyEventHandler((event) => {
      if (event.type === 'keydown' && event.key === 'Enter' && event.shiftKey) {
        if (ptyId) window.saturn.pty.write(ptyId, SHIFT_ENTER);
        return false;
      }
      return true;
    });

    const offData = window.saturn.pty.onData((id, data) => {
      if (id === ptyId) terminal.write(data);
    });
    const offExit = window.saturn.pty.onExit((id) => {
      if (id === ptyId) terminal.write('\r\n[process exited]\r\n');
    });
    terminal.onData((data) => {
      if (ptyId) window.saturn.pty.write(ptyId, data);
    });
    terminal.onResize(({ cols, rows }) => {
      if (ptyId) window.saturn.pty.resize(ptyId, cols, rows);
    });

    void window.saturn.pty.create({ cwd, cols: terminal.cols, rows: terminal.rows }).then((id) => {
      if (disposed) {
        window.saturn.pty.kill(id);
        return;
      }
      ptyId = id;
      if (command) window.saturn.pty.write(id, `${command}\r`);
    });

    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(element);

    return () => {
      disposed = true;
      observer.disconnect();
      offData();
      offExit();
      if (ptyId) window.saturn.pty.kill(ptyId);
      terminal.dispose();
    };
  }, [index, cwd, command]);

  return (
    <section className="pane">
      <header className="pane-header">
        <span className="pane-title">{title || `Terminal ${index + 1}`}</span>
        <span className="pane-meta">
          {status ? `${status.state}${status.kind ? ` · ${status.kind}` : ''}` : '—'} · bell {bells}{' '}
          · {renderer}
        </span>
      </header>
      <div className="pane-body" ref={host} />
    </section>
  );
};
