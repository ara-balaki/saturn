import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';

import { IPC, type Command, type PtyInfo, type Project, type SaturnApi } from '@shared/ipc';

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
    ack: (id, length) => ipcRenderer.send(IPC.ptyAck, id, length),
    info: (id) => ipcRenderer.invoke(IPC.ptyInfo, id) as Promise<PtyInfo | undefined>,
    onData: (listener) => subscribe(IPC.ptyData, listener),
    onExit: (listener) => subscribe(IPC.ptyExit, listener),
  },
  project: {
    get: () => ipcRenderer.invoke(IPC.projectGet) as Promise<Project | undefined>,
    open: () => ipcRenderer.invoke(IPC.projectOpen) as Promise<Project | undefined>,
    onChanged: (listener) => subscribe(IPC.projectChanged, listener),
  },
  confirm: (options) => ipcRenderer.invoke(IPC.confirm, options) as Promise<boolean>,
  onCommand: (listener) => subscribe<[Command]>(IPC.command, listener),
  log: (event, detail) => ipcRenderer.send(IPC.log, event, detail),
};

contextBridge.exposeInMainWorld('saturn', api);
