import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';

import { IPC, type SaturnApi } from '@shared/ipc';

const subscribe = <Args extends unknown[]>(channel: string, listener: (...args: Args) => void) => {
  const handler = (_event: IpcRendererEvent, ...args: unknown[]) => listener(...(args as Args));
  ipcRenderer.on(channel, handler);
  return () => {
    ipcRenderer.removeListener(channel, handler);
  };
};

const api: SaturnApi = {
  pty: {
    create: (options) => ipcRenderer.invoke(IPC.ptyCreate, options) as Promise<string>,
    write: (id, data) => ipcRenderer.send(IPC.ptyWrite, id, data),
    resize: (id, cols, rows) => ipcRenderer.send(IPC.ptyResize, id, cols, rows),
    kill: (id) => ipcRenderer.send(IPC.ptyKill, id),
    onData: (listener) => subscribe(IPC.ptyData, listener),
    onExit: (listener) => subscribe(IPC.ptyExit, listener),
  },
  log: (event, detail) => ipcRenderer.send(IPC.log, event, detail),
};

contextBridge.exposeInMainWorld('saturn', api);
