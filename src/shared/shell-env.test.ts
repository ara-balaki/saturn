import { describe, expect, it } from 'vitest';

import { shellEnv } from './shell-env';

describe('shellEnv', () => {
  it('drops the markers a parent Claude session sets', () => {
    const env = shellEnv(
      {
        PATH: '/usr/bin',
        CLAUDECODE: '1',
        CLAUDE_PID: '42',
        CLAUDE_EFFORT: 'high',
        CLAUDE_CODE_ENTRYPOINT: 'cli',
        CLAUDE_CODE_EXECPATH: '/x',
        CLAUDE_CODE_SESSION_ID: 'abc',
        CLAUDE_CODE_SESSION_ATTENDED: '1',
        CLAUDE_CODE_CHILD_SESSION: '1',
        CLAUDE_CODE_MESSAGING_SOCKET: '/tmp/s',
        CLAUDE_CODE_MESSAGING_TOKEN: 't',
      },
      {},
    );
    expect(env).toEqual({ PATH: '/usr/bin' });
  });

  it('keeps Claude settings a user sets on purpose', () => {
    const env = shellEnv({ CLAUDE_CONFIG_DIR: '/c', CLAUDE_CODE_DISABLE_MOUSE: '1' }, {});
    expect(env).toEqual({ CLAUDE_CONFIG_DIR: '/c', CLAUDE_CODE_DISABLE_MOUSE: '1' });
  });

  it('adds the extras last and skips undefined values', () => {
    expect(shellEnv({ TERM: 'dumb', HOME: undefined }, { TERM: 'xterm-256color' })).toEqual({
      TERM: 'xterm-256color',
    });
  });
});
