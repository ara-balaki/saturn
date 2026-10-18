import { describe, expect, it } from 'vitest';

import { isProgramStatusProbe, parseProgramStatus } from './program-status';

const base64 = (text: string) => Buffer.from(text, 'utf8').toString('base64');

describe('parseProgramStatus', () => {
  it('reads state, kind and base64 title and message', () => {
    const data = `state=blocked:app=claude-code:id=7:kind=permission:title=${base64('ABC-236 ✳')}:msg=${base64('Run pnpm test?')}`;
    expect(parseProgramStatus(data)).toEqual({
      state: 'blocked',
      app: 'claude-code',
      id: '7',
      kind: 'permission',
      progress: undefined,
      title: 'ABC-236 ✳',
      msg: 'Run pnpm test?',
    });
  });

  it('reads progress only when present', () => {
    expect(parseProgramStatus('state=working:app=claude-code:progress=40')?.progress).toBe(40);
    expect(parseProgramStatus('state=working:app=claude-code')?.progress).toBeUndefined();
  });

  it('reads the clear message Claude sends on exit', () => {
    expect(parseProgramStatus('state=clear')?.state).toBe('clear');
  });

  it('ignores data without a state', () => {
    expect(parseProgramStatus('app=claude-code')).toBeUndefined();
  });

  it('drops a title that is not valid base64 instead of throwing', () => {
    expect(parseProgramStatus('state=working:title=%%%')?.title).toBeUndefined();
  });
});

describe('isProgramStatusProbe', () => {
  it('recognises the probe', () => {
    expect(isProgramStatusProbe('?')).toBe(true);
    expect(isProgramStatusProbe('state=working')).toBe(false);
  });
});
