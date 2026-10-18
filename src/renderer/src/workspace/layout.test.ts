import { describe, expect, it } from 'vitest';

import {
  activateTab,
  addTab,
  allTabs,
  cellsInOrder,
  cycleTab,
  emptyLayout,
  focusDirection,
  focusedTab,
  focusNth,
  locateTab,
  moveTab,
  removeTab,
  splitDown,
  splitRight,
  toggleMaximize,
  visibleTabs,
  type Layout,
} from './layout';

/** The grid as tab names per cell, row by row: [['a', 'b'], ['c']] is one row of two cells... */
const shape = (layout: Layout) =>
  layout.rows.map((row) => row.cells.map((cell) => cell.tabs.join(',')));

const build = (...steps: ((layout: Layout) => Layout)[]) =>
  steps.reduce((layout, step) => step(layout), emptyLayout);

describe('adding and splitting', () => {
  it('starts a grid with the first tab', () => {
    const layout = addTab(emptyLayout, 'a');
    expect(shape(layout)).toEqual([['a']]);
    expect(focusedTab(layout)).toBe('a');
  });

  it('adds a tab after the active one in the focused cell', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => addTab(l, 'b'),
      (l) => activateTab(l, 'a'),
      (l) => addTab(l, 'c'),
    );
    expect(shape(layout)).toEqual([['a,c,b']]);
    expect(focusedTab(layout)).toBe('c');
  });

  it('splits right into a cell that shares the width', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
    );
    expect(shape(layout)).toEqual([['a', 'b']]);
    expect(layout.rows[0]!.cells.map((cell) => cell.size)).toEqual([0.5, 0.5]);
    expect(focusedTab(layout)).toBe('b');
  });

  it('splits down into a new row below', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
      (l) => splitDown(l, 'c'),
    );
    expect(shape(layout)).toEqual([['a', 'b'], ['c']]);
    expect(layout.rows.map((row) => row.size)).toEqual([0.5, 0.5]);
  });
});

describe('removing', () => {
  it('activates the previous tab when the active one closes', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => addTab(l, 'b'),
      (l) => addTab(l, 'c'),
      (l) => activateTab(l, 'b'),
      (l) => removeTab(l, 'b'),
    );
    expect(shape(layout)).toEqual([['a,c']]);
    expect(focusedTab(layout)).toBe('a');
  });

  it('drops an emptied cell and row, and focuses the neighbour', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
      (l) => splitDown(l, 'c'),
      (l) => removeTab(l, 'c'),
    );
    expect(shape(layout)).toEqual([['a', 'b']]);
    expect(focusedTab(layout)).toBe('b');
  });

  it('empties the grid when the last tab closes', () => {
    const layout = removeTab(addTab(emptyLayout, 'a'), 'a');
    expect(layout.rows).toEqual([]);
    expect(layout.focused).toBeUndefined();
  });

  it('clears maximise when the maximised cell goes', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
      toggleMaximize,
      (l) => removeTab(l, 'b'),
    );
    expect(layout.maximized).toBeUndefined();
  });
});

describe('moving', () => {
  it('moves a tab into another cell and focuses it there', () => {
    const start = build(
      (l) => addTab(l, 'a'),
      (l) => addTab(l, 'b'),
      (l) => splitRight(l, 'c'),
    );
    const target = locateTab(start, 'c')!.cell.id;
    const layout = moveTab(start, 'a', target, 0);
    expect(shape(layout)).toEqual([['b', 'a,c']]);
    expect(focusedTab(layout)).toBe('a');
  });

  it('removes the source cell when its last tab moves away', () => {
    const start = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
    );
    const target = locateTab(start, 'b')!.cell.id;
    const layout = moveTab(start, 'a', target);
    expect(shape(layout)).toEqual([['b,a']]);
    expect(layout.focused).toBe(target);
  });

  it('reorders within a cell', () => {
    const start = build(
      (l) => addTab(l, 'a'),
      (l) => addTab(l, 'b'),
      (l) => addTab(l, 'c'),
    );
    const layout = moveTab(start, 'c', locateTab(start, 'c')!.cell.id, 0);
    expect(shape(layout)).toEqual([['c,a,b']]);
  });

  it('keeps every tab exactly once', () => {
    const start = build(
      (l) => addTab(l, 'a'),
      (l) => splitRight(l, 'b'),
      (l) => splitDown(l, 'c'),
      (l) => addTab(l, 'd'),
    );
    const layout = moveTab(start, 'b', locateTab(start, 'd')!.cell.id, 1);
    expect(allTabs(layout).toSorted()).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('focus', () => {
  // a | b | c
  //   d   | e
  const grid = build(
    (l) => addTab(l, 'a'),
    (l) => splitRight(l, 'b'),
    (l) => splitRight(l, 'c'),
    (l) => splitDown(l, 'd'),
    (l) => splitRight(l, 'e'),
    (l) => focusNth(l, 1),
  );

  it('counts cells in reading order for ⌘1…⌘9', () => {
    expect(cellsInOrder(grid).map((cell) => cell.active)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(focusedTab(focusNth(grid, 4))).toBe('d');
    expect(focusNth(grid, 9)).toBe(grid);
  });

  it('moves left and right within a row and stops at the edge', () => {
    expect(focusedTab(focusDirection(grid, 'right'))).toBe('b');
    expect(focusDirection(grid, 'left')).toBe(grid);
  });

  it('moves down to the cell most directly below', () => {
    expect(focusedTab(focusDirection(focusNth(grid, 1), 'down'))).toBe('d');
    expect(focusedTab(focusDirection(focusNth(grid, 3), 'down'))).toBe('e');
    expect(focusedTab(focusDirection(focusNth(grid, 4), 'up'))).toBe('a');
  });

  it('cycles tabs in the focused cell', () => {
    const layout = build(
      (l) => addTab(l, 'a'),
      (l) => addTab(l, 'b'),
    );
    expect(focusedTab(cycleTab(layout, 1))).toBe('a');
    expect(focusedTab(cycleTab(layout, -1))).toBe('a');
  });

  it('shows only the maximised cell', () => {
    expect(visibleTabs(grid)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(visibleTabs(toggleMaximize(focusNth(grid, 2)))).toEqual(['b']);
  });
});
