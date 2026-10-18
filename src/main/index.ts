import { appendFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

import { app, BrowserWindow, dialog, ipcMain } from 'electron';

import { IPC, type Command, type ConfirmOptions, type PtyCreateOptions } from '@shared/ipc';

import { buildMenu } from './menu';
import { lastProject, rememberProject, toProject } from './project-store';
import { createPtyManager } from './pty-manager';

// Tests run the app against their own data folder instead of the user's.
if (process.env.SATURN_USER_DATA) app.setPath('userData', process.env.SATURN_USER_DATA);

let window: BrowserWindow | undefined;

const send = (channel: string, ...args: unknown[]) => {
  if (window && !window.isDestroyed()) window.webContents.send(channel, ...args);
};

const ptys = createPtyManager({
  data: (id, data) => send(IPC.ptyData, id, data),
  exit: (id, exitCode) => send(IPC.ptyExit, id, exitCode),
});

// Events the renderer reports go to this file when it is set (spikes and smoke tests read it).
const logFile = process.env.SATURN_LOG_FILE;
if (logFile) mkdirSync(join(logFile, '..'), { recursive: true });

const openProject = async () => {
  if (!window) return undefined;
  const result = await dialog.showOpenDialog(window, {
    title: 'Open Project',
    properties: ['openDirectory'],
  });
  const path = result.filePaths[0];
  if (result.canceled || !path) return undefined;
  const project = toProject(path);
  rememberProject(project);
  send(IPC.projectChanged, project);
  return project;
};

const createWindow = () => {
  window = new BrowserWindow({
    width: 1600,
    height: 1000,
    minWidth: 720,
    minHeight: 480,
    backgroundColor: '#080808',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 16, y: 16 },
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
ipcMain.handle(IPC.ptyInfo, (_event, id: string) => ptys.info(id));
ipcMain.on(IPC.ptyWrite, (_event, id: string, data: string) => ptys.write(id, data));
ipcMain.on(IPC.ptyResize, (_event, id: string, cols: number, rows: number) =>
  ptys.resize(id, cols, rows),
);
ipcMain.on(IPC.ptyKill, (_event, id: string) => ptys.kill(id));
ipcMain.on(IPC.ptyAck, (_event, id: string, length: number) => ptys.ack(id, length));

ipcMain.handle(IPC.projectGet, () => lastProject());
ipcMain.handle(IPC.projectOpen, () => openProject());

ipcMain.handle(IPC.confirm, async (_event, options: ConfirmOptions) => {
  if (!window) return false;
  const { response } = await dialog.showMessageBox(window, {
    type: 'question',
    message: options.message,
    detail: options.detail,
    buttons: [options.confirmLabel, 'Cancel'],
    defaultId: 0,
    cancelId: 1,
  });
  return response === 0;
});

ipcMain.on(IPC.log, (_event, event: string, detail: unknown) => {
  if (logFile) appendFileSync(logFile, `${JSON.stringify({ at: Date.now(), event, detail })}\n`);
});

void app.whenReady().then(() => {
  buildMenu({
    command: (command: Command) => send(IPC.command, command),
    openProject: () => void openProject(),
  });
  createWindow();
});

app.on('window-all-closed', () => app.quit());

// Quitting waits for every shell to exit first (see ptys.shutdown).
let shellsStopped = false;
app.on('before-quit', (event) => {
  if (shellsStopped) return;
  event.preventDefault();
  void ptys.shutdown().then(() => {
    shellsStopped = true;
    app.quit();
  });
});
