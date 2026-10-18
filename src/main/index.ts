import { appendFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

import { app, BrowserWindow, ipcMain } from 'electron';

import { IPC, type PtyCreateOptions } from '@shared/ipc';

import { createPtyManager } from './pty-manager';

let window: BrowserWindow | undefined;

const ptys = createPtyManager({
  data: (id, data) => window?.webContents.send(IPC.ptyData, id, data),
  exit: (id, exitCode) => window?.webContents.send(IPC.ptyExit, id, exitCode),
});

// Phase 0: events the renderer reports are appended to a log file, so spikes can be read afterwards.
const logFile = process.env.SATURN_LOG_FILE;
if (logFile) mkdirSync(join(logFile, '..'), { recursive: true });

const createWindow = () => {
  window = new BrowserWindow({
    width: 1600,
    height: 1000,
    backgroundColor: '#14161a',
    titleBarStyle: 'hiddenInset',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: false,
    },
  });
  window.on('closed', () => {
    window = undefined;
  });

  const devServer = process.env.ELECTRON_RENDERER_URL;
  if (devServer) void window.loadURL(devServer);
  else void window.loadFile(join(__dirname, '../renderer/index.html'));
};

ipcMain.handle(IPC.ptyCreate, (_event, options: PtyCreateOptions) => ptys.create(options));
ipcMain.on(IPC.ptyWrite, (_event, id: string, data: string) => ptys.write(id, data));
ipcMain.on(IPC.ptyResize, (_event, id: string, cols: number, rows: number) =>
  ptys.resize(id, cols, rows),
);
ipcMain.on(IPC.ptyKill, (_event, id: string) => ptys.kill(id));
ipcMain.on(IPC.log, (_event, event: string, detail: unknown) => {
  if (logFile) appendFileSync(logFile, `${JSON.stringify({ at: Date.now(), event, detail })}\n`);
});

void app.whenReady().then(createWindow);

app.on('window-all-closed', () => {
  ptys.killAll();
  app.quit();
});
