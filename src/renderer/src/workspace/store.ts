import { useSyncExternalStore } from 'react';

import type { AgentState } from '@shared/agent-state';
import type { Project } from '@shared/ipc';

import { emptyLayout, type Layout } from './layout';

export type TabInfo = {
  id: string;
  /** The terminal's own title, cleaned: Claude's session name or the shell's folder. */
  title: string | undefined;
  /** A name typed by hand; wins over the terminal's title. */
  manualTitle: string | undefined;
  state: AgentState;
  /** Claude's current activity or question (OSC 7501 msg). */
  message: string | undefined;
  /** Changed into a state that asks for the user since they last looked at this tab. */
  unseen: boolean;
};

export type WorkspaceState = {
  project: Project | undefined;
  projectLoaded: boolean;
  layout: Layout;
  tabs: Record<string, TabInfo>;
  sidebarOpen: boolean;
  editorOpen: boolean;
  sidebarWidth: number;
  editorHeight: number;
  windowFocused: boolean;
  renaming: string | undefined;
};

let state: WorkspaceState = {
  project: undefined,
  projectLoaded: false,
  layout: emptyLayout,
  tabs: {},
  sidebarOpen: true,
  editorOpen: true,
  sidebarWidth: 270,
  editorHeight: 260,
  windowFocused: document.hasFocus(),
  renaming: undefined,
};

const subscribers = new Set<() => void>();

export const getState = () => state;

export const setState = (change: (current: WorkspaceState) => WorkspaceState) => {
  const next = change(state);
  if (next === state) return;
  state = next;
  for (const subscriber of subscribers) subscriber();
};

const subscribe = (subscriber: () => void) => {
  subscribers.add(subscriber);
  return () => {
    subscribers.delete(subscriber);
  };
};

export const useWorkspace = <T>(select: (current: WorkspaceState) => T): T =>
  useSyncExternalStore(subscribe, () => select(state));

export const tabName = (tab: TabInfo | undefined) => tab?.manualTitle ?? tab?.title ?? 'zsh';
