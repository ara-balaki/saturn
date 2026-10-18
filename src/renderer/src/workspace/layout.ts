// The terminal grid as data: rows of cells, each cell holding tabs (terminal ids). Every operation
// returns a new layout and never touches the terminals themselves, so the grid can be rearranged
// freely while the terminals keep running.

export type Cell = {
  id: string;
  tabs: string[];
  active: string;
  /** Relative width inside its row. */
  size: number;
};

type Row = {
  id: string;
  cells: Cell[];
  /** Relative height inside the grid. */
  size: number;
};

export type Layout = {
  rows: Row[];
  focused: string | undefined;
  maximized: string | undefined;
};

export type Direction = 'left' | 'right' | 'up' | 'down';

type Located = { row: Row; rowIndex: number; cell: Cell; cellIndex: number };

export const emptyLayout: Layout = { rows: [], focused: undefined, maximized: undefined };

const newId = () => crypto.randomUUID();

const newCell = (tabId: string, size = 1): Cell => ({
  id: newId(),
  tabs: [tabId],
  active: tabId,
  size,
});

/** Every cell, row by row, left to right: the order ⌘1…⌘9 count in. */
export const cellsInOrder = (layout: Layout): Cell[] => layout.rows.flatMap((row) => row.cells);

export const allTabs = (layout: Layout): string[] =>
  cellsInOrder(layout).flatMap((cell) => cell.tabs);

const locateCell = (layout: Layout, cellId: string | undefined): Located | undefined => {
  for (const [rowIndex, row] of layout.rows.entries()) {
    const cellIndex = row.cells.findIndex((cell) => cell.id === cellId);
    const cell = row.cells[cellIndex];
    if (cell) return { row, rowIndex, cell, cellIndex };
  }
  return undefined;
};

export const locateTab = (layout: Layout, tabId: string): Located | undefined => {
  const cell = cellsInOrder(layout).find((candidate) => candidate.tabs.includes(tabId));
  return cell ? locateCell(layout, cell.id) : undefined;
};

const mapCell = (layout: Layout, cellId: string, change: (cell: Cell) => Cell): Layout => ({
  ...layout,
  rows: layout.rows.map((row) => ({
    ...row,
    cells: row.cells.map((cell) => (cell.id === cellId ? change(cell) : cell)),
  })),
});

const focusedOrFirst = (layout: Layout) =>
  locateCell(layout, layout.focused) ?? locateCell(layout, cellsInOrder(layout)[0]?.id);

/** Adds a tab to a cell (the focused one by default) right after its active tab. */
export const addTab = (layout: Layout, tabId: string, cellId?: string): Layout => {
  const target = cellId ? locateCell(layout, cellId) : focusedOrFirst(layout);
  if (!target) {
    const cell = newCell(tabId);
    return {
      rows: [{ id: newId(), cells: [cell], size: 1 }],
      focused: cell.id,
      maximized: undefined,
    };
  }
  const tabs = [...target.cell.tabs];
  tabs.splice(tabs.indexOf(target.cell.active) + 1, 0, tabId);
  return {
    ...mapCell(layout, target.cell.id, (cell) => ({ ...cell, tabs, active: tabId })),
    focused: target.cell.id,
  };
};

/** Opens a tab in a new cell to the right of a cell, sharing its width. */
export const splitRight = (layout: Layout, tabId: string, cellId?: string): Layout => {
  const target = cellId ? locateCell(layout, cellId) : focusedOrFirst(layout);
  if (!target) return addTab(layout, tabId);
  const half = target.cell.size / 2;
  const cell = newCell(tabId, half);
  const cells = [...target.row.cells];
  cells.splice(target.cellIndex, 1, { ...target.cell, size: half }, cell);
  return {
    rows: layout.rows.map((row) => (row.id === target.row.id ? { ...row, cells } : row)),
    focused: cell.id,
    maximized: undefined,
  };
};

/** Opens a tab in a new row below a cell's row, sharing its height. */
export const splitDown = (layout: Layout, tabId: string, cellId?: string): Layout => {
  const target = cellId ? locateCell(layout, cellId) : focusedOrFirst(layout);
  if (!target) return addTab(layout, tabId);
  const half = target.row.size / 2;
  const cell = newCell(tabId);
  const rows = [...layout.rows];
  rows.splice(
    target.rowIndex,
    1,
    { ...target.row, size: half },
    { id: newId(), cells: [cell], size: half },
  );
  return { rows, focused: cell.id, maximized: undefined };
};

/** Takes a tab out of the grid; an emptied cell or row goes with it. */
export const removeTab = (layout: Layout, tabId: string): Layout => {
  const source = locateTab(layout, tabId);
  if (!source) return layout;
  const tabs = source.cell.tabs.filter((tab) => tab !== tabId);

  if (tabs.length > 0) {
    const index = source.cell.tabs.indexOf(tabId);
    const active =
      source.cell.active === tabId
        ? (tabs[Math.max(0, index - 1)] ?? tabs[0]!)
        : source.cell.active;
    return mapCell(layout, source.cell.id, (cell) => ({ ...cell, tabs, active }));
  }

  const order = cellsInOrder(layout);
  const position = order.findIndex((cell) => cell.id === source.cell.id);
  const neighbour = order[position - 1] ?? order[position + 1];
  const rows = layout.rows
    .map((row) => ({ ...row, cells: row.cells.filter((cell) => cell.id !== source.cell.id) }))
    .filter((row) => row.cells.length > 0);
  return {
    rows,
    focused: layout.focused === source.cell.id ? neighbour?.id : layout.focused,
    maximized: layout.maximized === source.cell.id ? undefined : layout.maximized,
  };
};

/** Moves a tab into another cell (or to another position in its own), making it active there. */
export const moveTab = (
  layout: Layout,
  tabId: string,
  toCellId: string,
  index?: number,
): Layout => {
  const source = locateTab(layout, tabId);
  if (!source || !locateCell(layout, toCellId)) return layout;

  if (source.cell.id === toCellId) {
    const tabs = source.cell.tabs.filter((tab) => tab !== tabId);
    tabs.splice(index ?? tabs.length, 0, tabId);
    return {
      ...mapCell(layout, toCellId, (cell) => ({ ...cell, tabs, active: tabId })),
      focused: toCellId,
    };
  }

  const without = removeTab(layout, tabId);
  return {
    ...mapCell(without, toCellId, (cell) => {
      const tabs = [...cell.tabs];
      tabs.splice(index ?? tabs.length, 0, tabId);
      return { ...cell, tabs, active: tabId };
    }),
    focused: toCellId,
  };
};

export const activateTab = (layout: Layout, tabId: string): Layout => {
  const source = locateTab(layout, tabId);
  if (!source) return layout;
  return {
    ...mapCell(layout, source.cell.id, (cell) => ({ ...cell, active: tabId })),
    focused: source.cell.id,
  };
};

export const focusCell = (layout: Layout, cellId: string): Layout =>
  locateCell(layout, cellId) ? { ...layout, focused: cellId } : layout;

/** ⌘1…⌘9: the nth cell in reading order. */
export const focusNth = (layout: Layout, n: number): Layout => {
  const cell = cellsInOrder(layout)[n - 1];
  return cell
    ? { ...layout, focused: cell.id, maximized: layout.maximized ? cell.id : undefined }
    : layout;
};

/** Next or previous tab in the focused cell, wrapping around. */
export const cycleTab = (layout: Layout, delta: number): Layout => {
  const target = focusedOrFirst(layout);
  if (!target) return layout;
  const { tabs, active } = target.cell;
  const next = tabs[(tabs.indexOf(active) + delta + tabs.length) % tabs.length]!;
  return mapCell(layout, target.cell.id, (cell) => ({ ...cell, active: next }));
};

/** The horizontal centre of each cell in a row, from 0 to 1. */
const centres = (row: Row): number[] => {
  const total = row.cells.reduce((sum, cell) => sum + cell.size, 0);
  let start = 0;
  return row.cells.map((cell) => {
    const centre = (start + cell.size / 2) / total;
    start += cell.size;
    return centre;
  });
};

/** Moves focus to the neighbouring cell; up and down pick the cell most directly above or below. */
export const focusDirection = (layout: Layout, direction: Direction): Layout => {
  const from = focusedOrFirst(layout);
  if (!from) return layout;
  if (direction === 'left' || direction === 'right') {
    const cell = from.row.cells[from.cellIndex + (direction === 'left' ? -1 : 1)];
    return cell ? { ...layout, focused: cell.id } : layout;
  }
  const row = layout.rows[from.rowIndex + (direction === 'up' ? -1 : 1)];
  if (!row) return layout;
  const x = centres(from.row)[from.cellIndex]!;
  const rowCentres = centres(row);
  let best = 0;
  for (const [index, centre] of rowCentres.entries()) {
    if (Math.abs(centre - x) < Math.abs(rowCentres[best]! - x)) best = index;
  }
  return { ...layout, focused: row.cells[best]!.id };
};

export const toggleMaximize = (layout: Layout): Layout => ({
  ...layout,
  maximized: layout.maximized ? undefined : layout.focused,
});

export const resizeRows = (layout: Layout, sizes: number[]): Layout => ({
  ...layout,
  rows: layout.rows.map((row, index) => ({ ...row, size: sizes[index] ?? row.size })),
});

export const resizeCells = (layout: Layout, rowId: string, sizes: number[]): Layout => ({
  ...layout,
  rows: layout.rows.map((row) =>
    row.id === rowId
      ? {
          ...row,
          cells: row.cells.map((cell, index) => ({ ...cell, size: sizes[index] ?? cell.size })),
        }
      : row,
  ),
});

/** The tab the user is looking at: the focused cell's active tab. */
export const focusedTab = (layout: Layout): string | undefined =>
  focusedOrFirst(layout)?.cell.active;

/** Tabs drawn on screen right now: each visible cell's active tab. */
export const visibleTabs = (layout: Layout): string[] => {
  const cells = layout.maximized
    ? cellsInOrder(layout).filter((cell) => cell.id === layout.maximized)
    : cellsInOrder(layout);
  return cells.map((cell) => cell.active);
};
