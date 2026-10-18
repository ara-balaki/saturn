import { Fragment, useRef } from 'react';

import { PlusIcon } from '../chrome/icons';
import { Resizer } from '../chrome/Resizer';
import { openTerminal, resizeGridCells, resizeGridRows } from '../workspace/actions';
import type { Layout } from '../workspace/layout';
import { useWorkspace } from '../workspace/store';
import { Cell } from './Cell';

// No pane is dragged smaller than this share of its row or of the grid.
const MIN_SHARE = 0.08;

/** Resizes two neighbouring weights by a pixel distance, keeping their sum and a minimum share. */
const shift = (sizes: number[], index: number, deltaPx: number, extentPx: number) => {
  const total = sizes.reduce((sum, size) => sum + size, 0);
  const minimum = total * MIN_SHARE;
  const pair = sizes[index]! + sizes[index + 1]!;
  const first = Math.min(
    pair - minimum,
    Math.max(minimum, sizes[index]! + (deltaPx / extentPx) * total),
  );
  const next = [...sizes];
  next[index] = first;
  next[index + 1] = pair - first;
  return next;
};

const EmptyGrid = () => (
  <div className="sat-pane empty-grid">
    <div className="sat-dots empty-grid__dots" />
    <div className="empty-grid__content">
      <button
        type="button"
        className="sat-icon-tile sat-icon-tile--accent"
        aria-label="New terminal"
        onClick={() => void openTerminal()}
      >
        <PlusIcon />
      </button>
      <div className="empty-grid__title">New terminal</div>
      <div className="empty-grid__hint">⌘T opens a shell in the project folder</div>
    </div>
  </div>
);

const containsCell = (layout: Layout, rowIndex: number, cellId: string | undefined) =>
  layout.rows[rowIndex]?.cells.some((cell) => cell.id === cellId) ?? false;

export const Grid = () => {
  const layout = useWorkspace((state) => state.layout);
  const grid = useRef<HTMLDivElement>(null);
  const start = useRef<number[]>([]);

  if (layout.rows.length === 0) return <EmptyGrid />;

  const maximized = layout.maximized;

  return (
    <div className="grid" ref={grid}>
      {layout.rows.map((row, rowIndex) => {
        const rowHidden = maximized !== undefined && !containsCell(layout, rowIndex, maximized);
        return (
          <Fragment key={row.id}>
            {rowIndex > 0 && !maximized && (
              <Resizer
                axis="row"
                label="Resize rows"
                onStart={() => {
                  start.current = layout.rows.map((current) => current.size);
                }}
                onDrag={(delta) =>
                  resizeGridRows(
                    shift(start.current, rowIndex - 1, delta, grid.current?.clientHeight ?? 1),
                  )
                }
              />
            )}
            <div
              className="grid-row"
              hidden={rowHidden}
              style={{ flexGrow: maximized ? 1 : row.size }}
            >
              {row.cells.map((cell, cellIndex) => (
                <Fragment key={cell.id}>
                  {cellIndex > 0 && !maximized && (
                    <Resizer
                      axis="col"
                      label="Resize panes"
                      onStart={() => {
                        start.current = row.cells.map((current) => current.size);
                      }}
                      onDrag={(delta) =>
                        resizeGridCells(
                          row.id,
                          shift(
                            start.current,
                            cellIndex - 1,
                            delta,
                            grid.current?.clientWidth ?? 1,
                          ),
                        )
                      }
                    />
                  )}
                  <div
                    className="grid-cell"
                    hidden={maximized !== undefined && cell.id !== maximized}
                    style={{ flexGrow: maximized ? 1 : cell.size }}
                  >
                    <Cell
                      cell={cell}
                      focused={cell.id === layout.focused}
                      hidden={maximized !== undefined && cell.id !== maximized}
                    />
                  </div>
                </Fragment>
              ))}
            </div>
          </Fragment>
        );
      })}
    </div>
  );
};
