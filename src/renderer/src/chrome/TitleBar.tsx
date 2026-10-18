import { openTerminal, runCommand } from '../workspace/actions';
import { useWorkspace } from '../workspace/store';
import { EditorIcon, SidebarIcon } from './icons';

type Counts = { needsYou: number; yourTurn: number; working: number };

const countStates = (tabs: { state: string }[]): Counts => ({
  needsYou: tabs.filter((tab) => tab.state === 'needs-you').length,
  yourTurn: tabs.filter((tab) => tab.state === 'your-turn' || tab.state === 'error').length,
  working: tabs.filter((tab) => tab.state === 'working').length,
});

export const TitleBar = () => {
  // Serialised so the selector returns a stable value between renders.
  const summary = useWorkspace((state) => JSON.stringify(countStates(Object.values(state.tabs))));
  const sidebarOpen = useWorkspace((state) => state.sidebarOpen);
  const editorOpen = useWorkspace((state) => state.editorOpen);
  const counts = JSON.parse(summary) as Counts;

  return (
    <header className="titlebar">
      <div className="titlebar__toggles">
        <button
          type="button"
          className="sat-icon-tile sat-icon-tile--sm"
          aria-label="Toggle sidebar"
          aria-pressed={sidebarOpen}
          title="Toggle sidebar (⌘B)"
          onClick={() => runCommand('toggle-sidebar')}
        >
          <SidebarIcon />
        </button>
        <button
          type="button"
          className="sat-icon-tile sat-icon-tile--sm"
          aria-label="Toggle editor"
          aria-pressed={editorOpen}
          title="Toggle editor (⌘J)"
          onClick={() => runCommand('toggle-editor')}
        >
          <EditorIcon />
        </button>
      </div>
      <div className="titlebar__status" aria-live="polite">
        {counts.needsYou > 0 && (
          <button
            type="button"
            className="titlebar__count"
            onClick={() => runCommand('next-attention')}
          >
            <span className="sat-status" data-state="needs-you">
              <span className="sat-status__dot" />
              {counts.needsYou} need{counts.needsYou === 1 ? 's' : ''} you
            </span>
          </button>
        )}
        {counts.yourTurn > 0 && (
          <button
            type="button"
            className="titlebar__count"
            onClick={() => runCommand('next-attention')}
          >
            <span className="sat-status" data-state="your-turn">
              <span className="sat-status__dot" />
              {counts.yourTurn} your turn
            </span>
          </button>
        )}
        {counts.working > 0 && (
          <span className="sat-status" data-state="working">
            <span className="sat-status__dot" />
            {counts.working} working
          </span>
        )}
        <button
          type="button"
          className="sat-button sat-button--primary"
          onClick={() => void openTerminal()}
        >
          New terminal <kbd>⌘T</kbd>
        </button>
      </div>
    </header>
  );
};
