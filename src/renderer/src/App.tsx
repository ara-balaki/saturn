import { TerminalPane } from './terminal/TerminalPane';

// Phase 0 harness: `?panes=12&command=...&cwd=...` sets the grid size and what each pane runs.
const params = new URLSearchParams(window.location.search);
const paneCount = Number(params.get('panes') ?? '4');
const command = params.get('command') ?? undefined;
const cwd = params.get('cwd') ?? undefined;
const columns = Math.ceil(paneCount / (paneCount <= 3 ? 1 : paneCount <= 8 ? 2 : 3));

export const App = () => (
  <main className="grid" style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}>
    {Array.from({ length: paneCount }, (_, index) => (
      <TerminalPane key={index} index={index} cwd={cwd} command={command} />
    ))}
  </main>
);
