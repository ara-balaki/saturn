import { Menu, type MenuItemConstructorOptions } from 'electron';

import type { Command } from '@shared/ipc';

type MenuActions = {
  command: (command: Command) => void;
  openProject: () => void;
};

/**
 * The app menu. Its accelerators are Saturn's shortcuts: macOS handles them before the focused
 * terminal sees the key, so they work no matter what runs in it.
 */
export const buildMenu = ({ command, openProject }: MenuActions) => {
  const item = (
    label: string,
    accelerator: string,
    action: Command,
  ): MenuItemConstructorOptions => ({
    label,
    accelerator,
    click: () => command(action),
  });

  const template: MenuItemConstructorOptions[] = [
    { role: 'appMenu' },
    {
      label: 'File',
      submenu: [
        { label: 'Open Project…', accelerator: 'CmdOrCtrl+O', click: openProject },
        { type: 'separator' },
        item('New Terminal Tab', 'CmdOrCtrl+T', 'new-tab'),
        item('Close Tab', 'CmdOrCtrl+W', 'close-tab'),
      ],
    },
    { role: 'editMenu' },
    {
      label: 'View',
      submenu: [
        item('Toggle Sidebar', 'CmdOrCtrl+B', 'toggle-sidebar'),
        item('Toggle Editor', 'CmdOrCtrl+J', 'toggle-editor'),
        { type: 'separator' },
        item('Maximise Pane', 'CmdOrCtrl+Shift+Enter', 'toggle-maximize'),
        { type: 'separator' },
        { role: 'reload' },
        { role: 'toggleDevTools' },
        { role: 'togglefullscreen' },
      ],
    },
    {
      label: 'Terminal',
      submenu: [
        item('Split Right', 'CmdOrCtrl+D', 'split-right'),
        item('Split Down', 'CmdOrCtrl+Shift+D', 'split-down'),
        { type: 'separator' },
        item('Next Tab', 'CmdOrCtrl+Shift+]', 'next-tab'),
        item('Previous Tab', 'CmdOrCtrl+Shift+[', 'previous-tab'),
        { type: 'separator' },
        item('Focus Pane Left', 'CmdOrCtrl+Alt+Left', 'focus-left'),
        item('Focus Pane Right', 'CmdOrCtrl+Alt+Right', 'focus-right'),
        item('Focus Pane Above', 'CmdOrCtrl+Alt+Up', 'focus-up'),
        item('Focus Pane Below', 'CmdOrCtrl+Alt+Down', 'focus-down'),
        { type: 'separator' },
        item('Next Agent Needing You', 'CmdOrCtrl+Shift+A', 'next-attention'),
        { type: 'separator' },
        ...Array.from({ length: 9 }, (_, index) =>
          item(
            `Focus Pane ${index + 1}`,
            `CmdOrCtrl+${index + 1}`,
            `focus-cell-${index + 1}` as Command,
          ),
        ),
      ],
    },
    { role: 'windowMenu' },
  ];

  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
};
