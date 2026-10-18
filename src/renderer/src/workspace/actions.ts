// What the user can do to the workspace, from shortcuts, buttons or drag and drop. Each action
// changes the layout and tab state, and starts or stops the terminals that go with it.
import {
  agentStateFromProgramStatus,
  ATTENTION_STATES,
  tabTitleFromTerminalTitle,
  type AgentState,
} from '@shared/agent-state';
import type { Command } from '@shared/ipc';

import {
  createTerminal,
  disposeTerminal,
  focusTerminal,
  killTerminal,
  setTerminalEvents,
  setVisibleTerminals,
} from '../terminal/registry';
import {
  activateTab,
  addTab,
  allTabs,
  cellsInOrder,
  cycleTab,
  focusCell,
  focusDirection,
  focusedTab,
  focusNth,
  locateTab,
  moveTab,
  removeTab,
  resizeCells,
  resizeRows,
  splitDown,
  splitRight,
  toggleMaximize,
  visibleTabs,
  type Layout,
} from './layout';
import { getState, setState, tabName, type TabInfo, type WorkspaceState } from './store';

const SHELLS = new Set(['zsh', '-zsh', 'bash', '-bash', 'fish', 'sh', 'login']);

const isLooking = (current: WorkspaceState, tabId: string) =>
  current.windowFocused && focusedTab(current.layout) === tabId;

/** Applies a layout change, then clears the "unseen" mark of the tab now in front of the user. */
const updateLayout = (change: (layout: Layout) => Layout) => {
  setState((current) => {
    const layout = change(current.layout);
    if (layout === current.layout) return current;
    const front = focusedTab(layout);
    const tab = front ? current.tabs[front] : undefined;
    const tabs =
      tab?.unseen && current.windowFocused
        ? { ...current.tabs, [tab.id]: { ...tab, unseen: false } }
        : current.tabs;
    return { ...current, layout, tabs };
  });
  syncTerminals();
};

/** Keeps the terminals in step with the layout: WebGL for those on screen, focus on the front one. */
const syncTerminals = () => {
  const { layout } = getState();
  setVisibleTerminals(visibleTabs(layout));
  const front = focusedTab(layout);
  if (!front) return;
  requestAnimationFrame(() => {
    // Typing in a field (renaming a tab) keeps the keyboard; terminals take it otherwise.
    if (document.activeElement instanceof HTMLInputElement) return;
    focusTerminal(front);
  });
};

const updateTab = (id: string, change: (tab: TabInfo) => TabInfo) => {
  setState((current) => {
    const tab = current.tabs[id];
    return tab ? { ...current, tabs: { ...current.tabs, [id]: change(tab) } } : current;
  });
};

const setTabState = (id: string, state: AgentState, message: string | undefined) => {
  setState((current) => {
    const tab = current.tabs[id];
    if (!tab) return current;
    const asks = ATTENTION_STATES.includes(state) && state !== tab.state;
    const unseen = asks ? !isLooking(current, id) : tab.unseen && ATTENTION_STATES.includes(state);
    return { ...current, tabs: { ...current.tabs, [id]: { ...tab, state, message, unseen } } };
  });
};

/** The folder a new terminal starts in: the focused terminal's, else the project's. */
const startFolder = async () => {
  const { layout, project } = getState();
  const front = focusedTab(layout);
  const info = front ? await window.saturn.pty.info(front) : undefined;
  return info?.cwd ?? project?.path;
};

type Placement = 'tab' | 'right' | 'down';

export const openTerminal = async (placement: Placement = 'tab', cellId?: string) => {
  const id = await createTerminal(await startFolder());
  setState((current) => ({
    ...current,
    tabs: {
      ...current.tabs,
      [id]: {
        id,
        title: undefined,
        manualTitle: undefined,
        state: 'shell',
        message: undefined,
        unseen: false,
      },
    },
  }));
  updateLayout((layout) => {
    if (placement === 'right') return splitRight(layout, id, cellId);
    if (placement === 'down') return splitDown(layout, id, cellId);
    return addTab(layout, id, cellId);
  });
};

/** Closes a tab, asking first when something other than the shell is running in it. */
export const closeTab = async (id: string | undefined = focusedTab(getState().layout)) => {
  if (!id) return;
  const info = await window.saturn.pty.info(id);
  if (info && !SHELLS.has(info.process)) {
    const name = tabName(getState().tabs[id]);
    const confirmed = await window.saturn.confirm({
      message: `Close ${name}?`,
      detail: `${info.process} is still running in it and will be stopped.`,
      confirmLabel: 'Close',
    });
    if (!confirmed) return;
  }
  if (info) killTerminal(id);
  else forgetTab(id);
};

/** Removes a tab whose shell has ended. */
const forgetTab = (id: string) => {
  disposeTerminal(id);
  updateLayout((layout) => removeTab(layout, id));
  setState((current) => {
    const { [id]: _gone, ...tabs } = current.tabs;
    return { ...current, tabs };
  });
};

export const selectTab = (id: string) => updateLayout((layout) => activateTab(layout, id));

export const selectCell = (cellId: string) => updateLayout((layout) => focusCell(layout, cellId));

export const dropTab = (id: string, cellId: string, index?: number) =>
  updateLayout((layout) => moveTab(layout, id, cellId, index));

export const resizeGridRows = (sizes: number[]) =>
  updateLayout((layout) => resizeRows(layout, sizes));

export const resizeGridCells = (rowId: string, sizes: number[]) =>
  updateLayout((layout) => resizeCells(layout, rowId, sizes));

export const renameTab = (id: string, name: string) => {
  const trimmed = name.trim();
  updateTab(id, (tab) => ({ ...tab, manualTitle: trimmed === '' ? undefined : trimmed }));
  setState((current) => ({ ...current, renaming: undefined }));
  syncTerminals();
};

export const startRename = (id: string) => setState((current) => ({ ...current, renaming: id }));

export const setSidebarWidth = (width: number) =>
  setState((current) => ({
    ...current,
    sidebarWidth: Math.round(Math.min(480, Math.max(200, width))),
  }));

export const setEditorHeight = (height: number) =>
  setState((current) => ({ ...current, editorHeight: Math.round(Math.max(120, height)) }));

/** The next tab, in reading order after the focused one, that asks for the user. */
const nextAttention = (layout: Layout, tabs: Record<string, TabInfo>) => {
  const order = allTabs(layout);
  const start = order.indexOf(focusedTab(layout) ?? '');
  const rotated = [...order.slice(start + 1), ...order.slice(0, start + 1)];
  return rotated.find((id) => tabs[id] && ATTENTION_STATES.includes(tabs[id].state));
};

export const runCommand = (command: Command) => {
  const { layout, tabs } = getState();
  switch (command) {
    case 'toggle-sidebar':
      setState((current) => ({ ...current, sidebarOpen: !current.sidebarOpen }));
      return;
    case 'toggle-editor':
      setState((current) => ({ ...current, editorOpen: !current.editorOpen }));
      return;
    case 'new-tab':
      void openTerminal('tab');
      return;
    case 'split-right':
      void openTerminal('right');
      return;
    case 'split-down':
      void openTerminal('down');
      return;
    case 'close-tab':
      void closeTab();
      return;
    case 'next-tab':
      updateLayout((current) => cycleTab(current, 1));
      return;
    case 'previous-tab':
      updateLayout((current) => cycleTab(current, -1));
      return;
    case 'focus-left':
    case 'focus-right':
    case 'focus-up':
    case 'focus-down':
      updateLayout((current) => focusDirection(current, command.slice('focus-'.length) as 'left'));
      return;
    case 'toggle-maximize':
      updateLayout(toggleMaximize);
      return;
    case 'next-attention': {
      const target = nextAttention(layout, tabs);
      if (target) updateLayout((current) => activateTab(current, target));
      return;
    }
    default: {
      const n = Number(command.slice('focus-cell-'.length));
      if (n > 0 && n <= cellsInOrder(layout).length)
        updateLayout((current) => focusNth(current, n));
    }
  }
};

/** Connects terminal events to tab state; called once at start. */
export const wireWorkspace = () => {
  setTerminalEvents({
    title: (id, title) =>
      updateTab(id, (tab) => ({ ...tab, title: tabTitleFromTerminalTitle(title) })),
    status: (id, status) => setTabState(id, agentStateFromProgramStatus(status), status.msg),
    bell: (id) =>
      setState((current) => {
        const tab = current.tabs[id];
        if (!tab || isLooking(current, id)) return current;
        return { ...current, tabs: { ...current.tabs, [id]: { ...tab, unseen: true } } };
      }),
    focus: (id) => {
      if (focusedTab(getState().layout) !== id && locateTab(getState().layout, id)) selectTab(id);
    },
    exit: (id) => forgetTab(id),
  });
  window.saturn.onCommand(runCommand);
  window.addEventListener('focus', () => {
    setState((current) => ({ ...current, windowFocused: true }));
    updateLayout((layout) => ({ ...layout }));
  });
  window.addEventListener('blur', () =>
    setState((current) => ({ ...current, windowFocused: false })),
  );
};
