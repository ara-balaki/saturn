import { describe, expect, it } from 'vitest';

import { agentStateFromProgramStatus, tabTitleFromTerminalTitle } from './agent-state';

describe('agentStateFromProgramStatus', () => {
  it.each([
    ['idle', 'idle'],
    ['working', 'working'],
    ['blocked', 'needs-you'],
    ['done', 'your-turn'],
    ['clear', 'shell'],
    ['error', 'error'],
    ['something-new', 'working'],
  ])('reads %s as %s', (state, expected) => {
    expect(agentStateFromProgramStatus({ state })).toBe(expected);
  });
});

describe('tabTitleFromTerminalTitle', () => {
  it('keeps a Claude session name', () => {
    expect(tabTitleFromTerminalTitle('ABC-236')).toBe('ABC-236');
  });

  it('drops a leading spinner glyph', () => {
    expect(tabTitleFromTerminalTitle('✳ ABC-236')).toBe('ABC-236');
  });

  it('turns a shell prompt title into its folder', () => {
    expect(tabTitleFromTerminalTitle('jonasarden@Mac: ~/Documents/Code/Acme/acme-web')).toBe(
      'acme-web',
    );
    expect(tabTitleFromTerminalTitle('jonasarden@Mac:~')).toBe('~');
  });

  it('gives nothing for an empty title', () => {
    expect(tabTitleFromTerminalTitle('  ')).toBeUndefined();
  });
});
