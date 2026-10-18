// Variables a running Claude Code session sets for its children. Saturn may itself be started from
// a terminal where Claude runs; passed on, they make every Claude in a Saturn pane think it is a
// child session, which turns its transcript off (no /resume, no saved name).
const CLAUDE_SESSION_MARKERS =
  /^(CLAUDECODE|CLAUDE_PID|CLAUDE_EFFORT|CLAUDE_CODE_(ENTRYPOINT|EXECPATH|SESSION_.*|CHILD_SESSION|MESSAGING_.*))$/;

type Env = Record<string, string | undefined>;

/** The environment for a pane's shell: the app's own, minus Claude's session markers, plus extras. */
export const shellEnv = (base: Env, extra: Record<string, string>): Record<string, string> => {
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(base)) {
    if (value !== undefined && !CLAUDE_SESSION_MARKERS.test(key)) env[key] = value;
  }
  return { ...env, ...extra };
};
