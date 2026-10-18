// The channels between the main process and the renderer, in one place so both sides agree.

export const IPC = {
  ptyCreate: 'pty:create',
  ptyWrite: 'pty:write',
  ptyResize: 'pty:resize',
  ptyKill: 'pty:kill',
  ptyAck: 'pty:ack',
  ptyInfo: 'pty:info',
  ptyData: 'pty:data',
  ptyExit: 'pty:exit',
  projectGet: 'project:get',
  projectOpen: 'project:open',
  projectChanged: 'project:changed',
  confirm: 'dialog:confirm',
  command: 'command',
  log: 'log',
} as const;

/** What the app menu (and its shortcuts) asks the renderer to do. */
const COMMANDS = [
  'toggle-sidebar',
  'toggle-editor',
  'new-tab',
  'close-tab',
  'split-right',
  'split-down',
  'next-tab',
  'previous-tab',
  'focus-left',
  'focus-right',
  'focus-up',
  'focus-down',
  'toggle-maximize',
  'next-attention',
  'focus-cell-1',
  'focus-cell-2',
  'focus-cell-3',
  'focus-cell-4',
  'focus-cell-5',
  'focus-cell-6',
  'focus-cell-7',
  'focus-cell-8',
  'focus-cell-9',
] as const;

export type Command = (typeof COMMANDS)[number];

export type PtyCreateOptions = {
  cwd?: string;
  cols: number;
  rows: number;
};

export type PtyInfo = {
  /** The program in the foreground, e.g. `zsh` or `claude`. */
  process: string;
  /** The shell's working directory, when it can be read. */
  cwd: string | undefined;
};

export type Project = {
  path: string;
  name: string;
};

export type ConfirmOptions = {
  message: string;
  detail?: string;
  confirmLabel: string;
};

export type SaturnApi = {
  pty: {
    create: (options: PtyCreateOptions) => Promise<string>;
    write: (id: string, data: string) => void;
    resize: (id: string, cols: number, rows: number) => void;
    kill: (id: string) => void;
    /** Tells the main process the renderer has drawn this many characters of a terminal's output. */
    ack: (id: string, length: number) => void;
    info: (id: string) => Promise<PtyInfo | undefined>;
    onData: (listener: (id: string, data: string) => void) => () => void;
    onExit: (listener: (id: string, exitCode: number) => void) => () => void;
  };
  project: {
    get: () => Promise<Project | undefined>;
    open: () => Promise<Project | undefined>;
    onChanged: (listener: (project: Project) => void) => () => void;
  };
  confirm: (options: ConfirmOptions) => Promise<boolean>;
  onCommand: (listener: (command: Command) => void) => () => void;
  log: (event: string, detail: unknown) => void;
};
