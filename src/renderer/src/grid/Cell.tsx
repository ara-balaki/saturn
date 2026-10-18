import { useEffect, useRef, useState, type DragEvent } from 'react';

import { AGENT_STATE_LABEL, type AgentState } from '@shared/agent-state';

import { PlusIcon, SplitDownIcon, SplitRightIcon } from '../chrome/icons';
import { attachTerminal, focusTerminal, parkTerminal } from '../terminal/registry';
import { dropTab, openTerminal, selectCell } from '../workspace/actions';
import type { Cell as CellModel } from '../workspace/layout';
import { useWorkspace } from '../workspace/store';
import { Tab, TAB_MIME } from './Tab';

type CellProps = {
  cell: CellModel;
  focused: boolean;
  hidden: boolean;
};

/** Where a dragged tab lands in the tab bar: before the first tab whose middle is right of the pointer. */
const dropIndex = (bar: HTMLElement, clientX: number) => {
  const tabs = [...bar.querySelectorAll<HTMLElement>('[data-tab-id]')];
  const index = tabs.findIndex((tab) => {
    const box = tab.getBoundingClientRect();
    return clientX < box.left + box.width / 2;
  });
  return index === -1 ? tabs.length : index;
};

const hasTab = (event: DragEvent) => event.dataTransfer.types.includes(TAB_MIME);

export const Cell = ({ cell, focused, hidden }: CellProps) => {
  const slot = useRef<HTMLDivElement>(null);
  const [dropping, setDropping] = useState(false);

  // The ring shows the loudest unseen state among the cell's tabs.
  const attention = useWorkspace((state): AgentState | undefined => {
    const unseen = cell.tabs.map((id) => state.tabs[id]).filter((tab) => tab?.unseen);
    if (unseen.some((tab) => tab?.state === 'needs-you')) return 'needs-you';
    if (unseen.some((tab) => tab?.state === 'error')) return 'error';
    if (unseen.length > 0) return 'your-turn';
    return undefined;
  });
  const active = useWorkspace((state) => state.tabs[cell.active]);

  useEffect(() => {
    const element = slot.current;
    if (!element) return;
    for (const id of cell.tabs) {
      if (id === cell.active) attachTerminal(id, element);
      else parkTerminal(id, element);
    }
    // A terminal can only take the keyboard once it is on screen, so the focused cell focuses its
    // terminal here, after attaching it, unless the user is typing in a field.
    if (focused && !(document.activeElement instanceof HTMLInputElement))
      focusTerminal(cell.active);
  }, [cell.tabs, cell.active, focused]);

  const onDrop = (event: DragEvent, index?: number) => {
    const id = event.dataTransfer.getData(TAB_MIME);
    setDropping(false);
    if (!id) return;
    event.preventDefault();
    event.stopPropagation();
    dropTab(id, cell.id, index);
  };

  const showFooter = active && active.state !== 'shell';

  return (
    <section
      className={`sat-pane cell${focused ? ' is-focused' : ''}`}
      data-attention={attention === 'error' ? 'your-turn' : attention}
      hidden={hidden}
      onMouseDown={() => selectCell(cell.id)}
    >
      <header className="sat-pane__head">
        <div
          className="sat-tabs"
          role="tablist"
          onDragOver={(event) => {
            if (hasTab(event)) event.preventDefault();
          }}
          onDrop={(event) => onDrop(event, dropIndex(event.currentTarget, event.clientX))}
        >
          {cell.tabs.map((id) => (
            <Tab key={id} id={id} selected={id === cell.active} />
          ))}
        </div>
        <button
          type="button"
          className="sat-icon-tile sat-icon-tile--sm"
          aria-label="Split right"
          title="Split right (⌘D)"
          onClick={() => void openTerminal('right', cell.id)}
        >
          <SplitRightIcon />
        </button>
        <button
          type="button"
          className="sat-icon-tile sat-icon-tile--sm"
          aria-label="Split down"
          title="Split down (⌘⇧D)"
          onClick={() => void openTerminal('down', cell.id)}
        >
          <SplitDownIcon />
        </button>
        <button
          type="button"
          className="sat-icon-tile sat-icon-tile--sm"
          aria-label="New tab"
          title="New tab (⌘T)"
          onClick={() => void openTerminal('tab', cell.id)}
        >
          <PlusIcon />
        </button>
      </header>
      <div
        className="cell-body"
        onDragOver={(event) => {
          if (!hasTab(event)) return;
          event.preventDefault();
          setDropping(true);
        }}
        onDragLeave={() => setDropping(false)}
        onDrop={(event) => onDrop(event)}
      >
        <div className="cell-slot" ref={slot} />
        {dropping && <div className="cell-drop sat-guide" />}
      </div>
      {showFooter && (
        <footer className="sat-pane__foot">
          <span className="sat-pane__msg">{active.message ?? AGENT_STATE_LABEL[active.state]}</span>
          <span className="sat-status" data-state={active.state}>
            <span className="sat-status__dot" />
            {AGENT_STATE_LABEL[active.state]}
          </span>
        </footer>
      )}
    </section>
  );
};
