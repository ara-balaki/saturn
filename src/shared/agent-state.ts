import type { ProgramStatus } from './program-status';

/** What a terminal tab shows about the program in it. Matches AgentStatus in the design system. */
export type AgentState = 'shell' | 'idle' | 'working' | 'needs-you' | 'your-turn' | 'error';

/** States that ask for the user, and so light a pane up until it has been looked at. */
export const ATTENTION_STATES: readonly AgentState[] = ['needs-you', 'your-turn', 'error'];

export const AGENT_STATE_LABEL: Record<AgentState, string> = {
  shell: 'Shell',
  idle: 'Idle',
  working: 'Working',
  'needs-you': 'Needs you',
  'your-turn': 'Your turn',
  error: 'Error',
};

/** Maps Claude Code's OSC 7501 report to a tab state. Unknown future states read as working. */
export const agentStateFromProgramStatus = (status: ProgramStatus): AgentState => {
  switch (status.state) {
    case 'clear':
      return 'shell';
    case 'idle':
      return 'idle';
    case 'blocked':
      return 'needs-you';
    case 'done':
      return 'your-turn';
    case 'error':
      return 'error';
    default:
      return 'working';
  }
};

// Spinner glyphs Claude Code may put in front of its terminal title.
const SPINNER = /^[·✢✳✶✻✽*⠁-⣿]\s*/u;
// What oh-my-zsh and most shell themes set: `user@host: ~/some/path`.
const SHELL_TITLE = /^[^\s@]+@[^\s:]+:\s*(.*)$/;

/** A terminal title as a short tab name: Claude's session name, or the shell's folder. */
export const tabTitleFromTerminalTitle = (title: string): string | undefined => {
  const trimmed = title.trim().replace(SPINNER, '').trim();
  if (trimmed === '') return undefined;
  const shell = SHELL_TITLE.exec(trimmed);
  if (shell) {
    const path = shell[1]!.trim().replace(/\/+$/, '');
    return path.split('/').at(-1) || path || undefined;
  }
  return trimmed;
};
