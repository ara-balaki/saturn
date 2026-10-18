// The channels between the main process and the renderer, in one place so both sides agree.

export const IPC = {
  ptyCreate: 'pty:create',
  ptyWrite: 'pty:write',
  ptyResize: 'pty:resize',
  ptyKill: 'pty:kill',
  ptyData: 'pty:data',
  ptyExit: 'pty:exit',
  log: 'log',
} as const;

export type PtyCreateOptions = {
  cwd?: string;
  cols: number;
  rows: number;
};

export type SaturnApi = {
  pty: {
    create: (options: PtyCreateOptions) => Promise<string>;
    write: (id: string, data: string) => void;
    resize: (id: string, cols: number, rows: number) => void;
    kill: (id: string) => void;
    onData: (listener: (id: string, data: string) => void) => () => void;
    onExit: (listener: (id: string, exitCode: number) => void) => () => void;
  };
  log: (event: string, detail: unknown) => void;
};
