// Live terminals, kept outside React. Each one owns an xterm.js instance mounted in its own host
// element; a grid cell borrows that element while the tab is shown, so moving a tab between cells,
// switching tabs or maximising never restarts the terminal or loses its screen.
import { FitAddon } from '@xterm/addon-fit';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal } from '@xterm/xterm';

import {
  isProgramStatusProbe,
  parseProgramStatus,
  PROGRAM_STATUS_OSC,
  PROGRAM_STATUS_PROBE_REPLY,
  type ProgramStatus,
} from '@shared/program-status';

import { terminalFonts, terminalTheme } from '../design/terminal-theme';

// Chromium keeps about 16 WebGL contexts per page and silently drops the oldest beyond that
// (Phase 0). Visible terminals get WebGL up to this budget; the rest use the DOM renderer.
const WEBGL_BUDGET = 14;

// Keys a Mac terminal is expected to translate, sent as the sequences shells and Claude understand.
const KEY_SEQUENCES: { match: (event: KeyboardEvent) => boolean; send: string }[] = [
  // Claude Code reads ESC CR as a newline in its prompt.
  { match: (e) => e.key === 'Enter' && e.shiftKey && !e.metaKey, send: '\u001b\r' },
  { match: (e) => e.key === 'ArrowLeft' && e.altKey && !e.metaKey, send: '\u001bb' },
  { match: (e) => e.key === 'ArrowRight' && e.altKey && !e.metaKey, send: '\u001bf' },
  { match: (e) => e.key === 'ArrowLeft' && e.metaKey && !e.altKey, send: '\u0001' },
  { match: (e) => e.key === 'ArrowRight' && e.metaKey && !e.altKey, send: '\u0005' },
  { match: (e) => e.key === 'Backspace' && e.metaKey, send: '\u0015' },
];

export type TerminalEvents = {
  title: (id: string, title: string) => void;
  status: (id: string, status: ProgramStatus) => void;
  bell: (id: string) => void;
  focus: (id: string) => void;
  exit: (id: string, exitCode: number) => void;
};

type Entry = {
  terminal: Terminal;
  fit: FitAddon;
  host: HTMLDivElement;
  webgl: WebglAddon | undefined;
  /** False once the WebGL context was lost: this terminal stays on the DOM renderer. */
  webglAllowed: boolean;
  observer: ResizeObserver;
};

const entries = new Map<string, Entry>();
// Output that arrives before its terminal is registered (the pty can start writing first).
const early = new Map<string, string>();
let listeners: TerminalEvents | undefined;
let parking: HTMLDivElement | undefined;
let wired = false;

/** Off-screen home for terminals no cell is showing: they keep running and keep their size. */
const parkingLot = () => {
  if (!parking) {
    parking = document.createElement('div');
    parking.setAttribute('aria-hidden', 'true');
    parking.style.cssText =
      'position:fixed;left:-10000px;top:0;width:800px;height:480px;visibility:hidden';
    document.body.append(parking);
  }
  return parking;
};

const write = (id: string, data: string) => {
  const entry = entries.get(id);
  if (!entry) {
    early.set(id, (early.get(id) ?? '') + data);
    return;
  }
  // The ack after xterm has parsed the data is what lets the main process resume a paused pty.
  entry.terminal.write(data, () => window.saturn.pty.ack(id, data.length));
};

const wire = () => {
  if (wired) return;
  wired = true;
  // One listener for every terminal, rather than one per pane.
  window.saturn.pty.onData(write);
  window.saturn.pty.onExit((id, exitCode) => listeners?.exit(id, exitCode));
};

export const setTerminalEvents = (events: TerminalEvents) => {
  listeners = events;
  wire();
};

const fitNow = (id: string, entry: Entry) => {
  if (entry.host.clientWidth === 0 || entry.host.clientHeight === 0) return;
  entry.fit.fit();
  window.saturn.pty.resize(id, entry.terminal.cols, entry.terminal.rows);
};

/** Starts a shell in a new terminal and returns its id. */
export const createTerminal = async (cwd: string | undefined): Promise<string> => {
  wire();
  await document.fonts.load(`13px ${terminalFonts.family}`);

  const terminal = new Terminal({
    fontFamily: terminalFonts.family,
    fontSize: 13,
    lineHeight: 1.15,
    macOptionIsMeta: true,
    macOptionClickForcesSelection: true,
    allowProposedApi: true,
    scrollback: 5000,
    cursorBlink: true,
    theme: terminalTheme,
  });
  const fit = new FitAddon();
  terminal.loadAddon(fit);
  terminal.loadAddon(new WebLinksAddon());
  terminal.loadAddon(new Unicode11Addon());
  terminal.unicode.activeVersion = '11';

  const host = document.createElement('div');
  host.className = 'terminal-host';
  parkingLot().append(host);
  terminal.open(host);
  fit.fit();

  const id = await window.saturn.pty.create({ cwd, cols: terminal.cols, rows: terminal.rows });

  const entry: Entry = {
    terminal,
    fit,
    host,
    webgl: undefined,
    webglAllowed: true,
    observer: new ResizeObserver(() => fitNow(id, entry)),
  };
  entries.set(id, entry);
  entry.observer.observe(host);

  terminal.onData((data) => window.saturn.pty.write(id, data));
  terminal.onTitleChange((title) => listeners?.title(id, title));
  terminal.onBell(() => listeners?.bell(id));
  terminal.textarea?.addEventListener('focus', () => listeners?.focus(id));
  terminal.parser.registerOscHandler(PROGRAM_STATUS_OSC, (data) => {
    if (isProgramStatusProbe(data)) {
      window.saturn.pty.write(id, PROGRAM_STATUS_PROBE_REPLY);
      return true;
    }
    const status = parseProgramStatus(data);
    if (status) listeners?.status(id, status);
    return true;
  });
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== 'keydown') return true;
    const sequence = KEY_SEQUENCES.find((key) => key.match(event));
    if (sequence) {
      event.preventDefault();
      window.saturn.pty.write(id, sequence.send);
      return false;
    }
    // ⌘-shortcuts belong to the app menu (copy, paste, tabs, splits), never to the program.
    return !event.metaKey;
  });

  const pending = early.get(id);
  if (pending !== undefined) {
    early.delete(id);
    write(id, pending);
  }
  return id;
};

/** Shows a terminal inside a cell's slot. */
export const attachTerminal = (id: string, slot: HTMLElement) => {
  const entry = entries.get(id);
  if (!entry || entry.host.parentElement === slot) return;
  slot.append(entry.host);
  requestAnimationFrame(() => fitNow(id, entry));
};

/** Takes a terminal off screen without stopping it. */
export const parkTerminal = (id: string, slot: HTMLElement) => {
  const entry = entries.get(id);
  if (entry && entry.host.parentElement === slot) parkingLot().append(entry.host);
};

export const focusTerminal = (id: string) => entries.get(id)?.terminal.focus();

/** Gives WebGL to the terminals on screen, within the budget, and takes it from the rest. */
export const setVisibleTerminals = (visible: string[]) => {
  const wanted = new Set(
    visible.filter((id) => entries.get(id)?.webglAllowed).slice(0, WEBGL_BUDGET),
  );
  for (const [id, entry] of entries) {
    if (!wanted.has(id) && entry.webgl) {
      entry.webgl.dispose();
      entry.webgl = undefined;
    }
  }
  for (const id of wanted) {
    const entry = entries.get(id)!;
    if (entry.webgl) continue;
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => {
        webgl.dispose();
        entry.webgl = undefined;
        entry.webglAllowed = false;
      });
      entry.terminal.loadAddon(webgl);
      entry.webgl = webgl;
    } catch {
      entry.webglAllowed = false;
    }
  }
};

/** Stops the shell; the terminal itself goes when the pty reports its exit. */
export const killTerminal = (id: string) => window.saturn.pty.kill(id);

export const disposeTerminal = (id: string) => {
  const entry = entries.get(id);
  if (!entry) return;
  entries.delete(id);
  entry.observer.disconnect();
  entry.webgl?.dispose();
  entry.terminal.dispose();
  entry.host.remove();
};

/** The visible screen of every terminal as text, for the end-to-end test (read-only). */
export const screenTexts = (): Record<string, string> => {
  const texts: Record<string, string> = {};
  for (const [id, { terminal }] of entries) {
    const buffer = terminal.buffer.active;
    const lines: string[] = [];
    for (let y = 0; y < buffer.length; y++)
      lines.push(buffer.getLine(y)?.translateToString(true) ?? '');
    texts[id] = lines.join('\n');
  }
  return texts;
};
