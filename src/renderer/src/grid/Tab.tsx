import type { DragEvent, KeyboardEvent } from 'react';

import { CloseIcon } from '../chrome/icons';
import { closeTab, renameTab, selectTab, startRename } from '../workspace/actions';
import { tabName, useWorkspace } from '../workspace/store';

export const TAB_MIME = 'application/x-saturn-tab';

type TabProps = {
  id: string;
  selected: boolean;
};

export const Tab = ({ id, selected }: TabProps) => {
  const tab = useWorkspace((state) => state.tabs[id]);
  const renaming = useWorkspace((state) => state.renaming === id);
  if (!tab) return null;

  const name = tabName(tab);
  // A tab hidden behind another that needs you turns yellow, so it can't be missed.
  const calling = !selected && tab.unseen && tab.state === 'needs-you';
  const dotState = tab.state === 'shell' ? 'idle' : tab.state;

  const onDragStart = (event: DragEvent) => {
    event.dataTransfer.setData(TAB_MIME, id);
    event.dataTransfer.effectAllowed = 'move';
  };

  const onRenameKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') renameTab(id, event.currentTarget.value);
    if (event.key === 'Escape') renameTab(id, tab.manualTitle ?? '');
  };

  return (
    <div
      className="sat-tab"
      role="tab"
      tabIndex={-1}
      aria-selected={selected}
      data-attention={calling ? 'needs-you' : undefined}
      data-tab-id={id}
      draggable={!renaming}
      onDragStart={onDragStart}
      onMouseDown={(event) => {
        if (event.button === 1) void closeTab(id);
        else selectTab(id);
      }}
      onDoubleClick={() => startRename(id)}
      title={tab.message ? `${name} · ${tab.message}` : name}
    >
      <span className="sat-status" data-state={dotState}>
        <span className="sat-status__dot" />
      </span>
      {renaming ? (
        <input
          className="tab-rename"
          aria-label="Tab name"
          defaultValue={name}
          autoFocus
          onFocus={(event) => event.currentTarget.select()}
          onBlur={(event) => renameTab(id, event.currentTarget.value)}
          onKeyDown={onRenameKey}
        />
      ) : (
        <span className="sat-tab__name">{name}</span>
      )}
      <button
        type="button"
        className="tab-close"
        aria-label={`Close ${name}`}
        onMouseDown={(event) => event.stopPropagation()}
        onClick={() => void closeTab(id)}
      >
        <CloseIcon />
      </button>
    </div>
  );
};
