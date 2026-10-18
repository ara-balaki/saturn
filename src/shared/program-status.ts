// Claude Code's OSC 7501 "Program Status Protocol" (Claude Code >= 2.1.295).
//
// Claude asks `ESC ] 7501 ; ? ST` once at startup; a terminal that answers with any OSC 7501 whose
// data starts with `?` then receives `state=<state>:app=claude-code[:id=..][:kind=..][:progress=N]
// [:title=<base64>][:msg=<base64>]` as the session changes, and `state=clear` when it exits.
// Undocumented beyond the changelog: the shape was read from the CLI, so parse defensively.

export const PROGRAM_STATUS_OSC = 7501;

/** The reply that tells Claude this terminal understands the protocol. */
export const PROGRAM_STATUS_PROBE_REPLY = '\u001b]7501;?\u001b\\';

export type ProgramStatus = {
  state: string;
  app?: string;
  id?: string;
  kind?: string;
  progress?: number;
  title?: string;
  msg?: string;
};

const decodeBase64 = (value: string): string | undefined => {
  try {
    const bytes = Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return undefined;
  }
};

export const isProgramStatusProbe = (data: string) => data.startsWith('?');

export const parseProgramStatus = (data: string): ProgramStatus | undefined => {
  const fields = new Map<string, string>();
  for (const part of data.split(':')) {
    const at = part.indexOf('=');
    if (at > 0) fields.set(part.slice(0, at), part.slice(at + 1));
  }
  const state = fields.get('state');
  if (!state) return undefined;

  const progress = Number(fields.get('progress'));
  const title = fields.get('title');
  const msg = fields.get('msg');
  return {
    state,
    app: fields.get('app'),
    id: fields.get('id'),
    kind: fields.get('kind'),
    progress: Number.isFinite(progress) && fields.has('progress') ? progress : undefined,
    title: title === undefined ? undefined : decodeBase64(title),
    msg: msg === undefined ? undefined : decodeBase64(msg),
  };
};
