//! The terminal grid as data: rows of cells, each cell holding tabs. Operations change the layout
//! only, never the terminals, so the grid can be rearranged freely while terminals keep running.

/// A terminal tab. Saturn uses the terminal's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowId(u64);

impl CellId {
    /// A number that stays the same for the cell's whole life, e.g. for UI element ids.
    pub fn key(self) -> u64 {
        self.0
    }
}

impl RowId {
    /// A number that stays the same for the row's whole life, e.g. for UI element ids.
    pub fn key(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub id: CellId,
    pub tabs: Vec<TabId>,
    pub active: TabId,
    /// Relative width inside its row.
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: RowId,
    pub cells: Vec<Cell>,
    /// Relative height inside the grid.
    pub size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Where a cell sits: row index and position in the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    row: usize,
    cell: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub rows: Vec<Row>,
    pub focused: Option<CellId>,
    pub maximized: Option<CellId>,
    next_id: u64,
}

impl Layout {
    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn new_cell(&mut self, tab: TabId, size: f32) -> Cell {
        Cell { id: CellId(self.new_id()), tabs: vec![tab], active: tab, size }
    }

    /// Every cell, row by row, left to right: the order ⌘1…⌘9 count in.
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.rows.iter().flat_map(|row| row.cells.iter())
    }

    pub fn tabs(&self) -> impl Iterator<Item = TabId> + '_ {
        self.cells().flat_map(|cell| cell.tabs.iter().copied())
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn position(&self, cell: CellId) -> Option<Position> {
        self.rows.iter().enumerate().find_map(|(row_index, row)| {
            let cell_index = row.cells.iter().position(|c| c.id == cell)?;
            Some(Position { row: row_index, cell: cell_index })
        })
    }

    fn position_of_tab(&self, tab: TabId) -> Option<Position> {
        self.rows.iter().enumerate().find_map(|(row_index, row)| {
            let cell_index = row.cells.iter().position(|c| c.tabs.contains(&tab))?;
            Some(Position { row: row_index, cell: cell_index })
        })
    }

    fn cell_at(&mut self, at: Position) -> &mut Cell {
        &mut self.rows[at.row].cells[at.cell]
    }

    pub fn cell(&self, id: CellId) -> Option<&Cell> {
        self.cells().find(|cell| cell.id == id)
    }

    pub fn cell_of_tab(&self, tab: TabId) -> Option<&Cell> {
        self.cells().find(|cell| cell.tabs.contains(&tab))
    }

    /// The focused cell, or the first one when focus was lost.
    fn target(&self, cell: Option<CellId>) -> Option<Position> {
        cell.or(self.focused)
            .and_then(|id| self.position(id))
            .or_else(|| (!self.rows.is_empty()).then_some(Position { row: 0, cell: 0 }))
    }

    /// The tab the user is looking at: the focused cell's active tab.
    pub fn focused_tab(&self) -> Option<TabId> {
        self.target(None).map(|at| self.rows[at.row].cells[at.cell].active)
    }

    /// Tabs drawn on screen right now: each visible cell's active tab.
    pub fn visible_tabs(&self) -> Vec<TabId> {
        self.cells().filter(|cell| self.maximized.is_none_or(|max| max == cell.id)).map(|cell| cell.active).collect()
    }

    /// Adds a tab to a cell (the focused one by default) right after its active tab.
    pub fn add_tab(&mut self, tab: TabId, cell: Option<CellId>) {
        let Some(at) = self.target(cell) else {
            let cell = self.new_cell(tab, 1.0);
            let id = RowId(self.new_id());
            self.focused = Some(cell.id);
            self.maximized = None;
            self.rows = vec![Row { id, cells: vec![cell], size: 1.0 }];
            return;
        };
        let target = self.cell_at(at);
        let index = target.tabs.iter().position(|t| *t == target.active).map_or(0, |i| i + 1);
        target.tabs.insert(index, tab);
        target.active = tab;
        self.focused = Some(target.id);
    }

    /// Opens a tab in a new cell to the right of a cell, sharing its width.
    pub fn split_right(&mut self, tab: TabId, cell: Option<CellId>) {
        let Some(at) = self.target(cell) else { return self.add_tab(tab, None) };
        let half = self.rows[at.row].cells[at.cell].size / 2.0;
        let new_cell = self.new_cell(tab, half);
        self.focused = Some(new_cell.id);
        self.maximized = None;
        let row = &mut self.rows[at.row];
        row.cells[at.cell].size = half;
        row.cells.insert(at.cell + 1, new_cell);
    }

    /// Opens a tab in a new row below a cell's row, sharing its height.
    pub fn split_down(&mut self, tab: TabId, cell: Option<CellId>) {
        let Some(at) = self.target(cell) else { return self.add_tab(tab, None) };
        let half = self.rows[at.row].size / 2.0;
        let new_cell = self.new_cell(tab, 1.0);
        let id = RowId(self.new_id());
        self.focused = Some(new_cell.id);
        self.maximized = None;
        self.rows[at.row].size = half;
        self.rows.insert(at.row + 1, Row { id, cells: vec![new_cell], size: half });
    }

    /// Takes a tab out of the grid; an emptied cell or row goes with it.
    pub fn remove_tab(&mut self, tab: TabId) {
        let Some(at) = self.position_of_tab(tab) else { return };
        let cell = self.cell_at(at);
        let index = cell.tabs.iter().position(|t| *t == tab).unwrap_or(0);
        cell.tabs.remove(index);
        if !cell.tabs.is_empty() {
            if cell.active == tab {
                cell.active = cell.tabs[index.saturating_sub(1).min(cell.tabs.len() - 1)];
            }
            return;
        }

        let removed = cell.id;
        let order: Vec<CellId> = self.cells().map(|c| c.id).collect();
        let place = order.iter().position(|id| *id == removed).unwrap_or(0);
        let neighbour = place.checked_sub(1).and_then(|i| order.get(i)).or_else(|| order.get(place + 1)).copied();
        self.rows[at.row].cells.remove(at.cell);
        self.rows.retain(|row| !row.cells.is_empty());
        if self.focused == Some(removed) {
            self.focused = neighbour;
        }
        if self.maximized == Some(removed) {
            self.maximized = None;
        }
    }

    /// Moves a tab into a cell (or to another position in its own), making it active there.
    pub fn move_tab(&mut self, tab: TabId, to: CellId, index: Option<usize>) {
        let (Some(from), Some(_)) = (self.position_of_tab(tab), self.position(to)) else { return };
        if self.rows[from.row].cells[from.cell].id == to {
            self.cell_at(from).tabs.retain(|t| *t != tab);
        } else {
            self.remove_tab(tab);
        }
        let Some(at) = self.position(to) else { return };
        let cell = self.cell_at(at);
        let index = index.unwrap_or(cell.tabs.len()).min(cell.tabs.len());
        cell.tabs.insert(index, tab);
        cell.active = tab;
        self.focused = Some(to);
    }

    pub fn activate_tab(&mut self, tab: TabId) {
        if let Some(at) = self.position_of_tab(tab) {
            let cell = self.cell_at(at);
            cell.active = tab;
            self.focused = Some(cell.id);
        }
    }

    pub fn focus_cell(&mut self, cell: CellId) {
        if self.position(cell).is_some() {
            self.focused = Some(cell);
        }
    }

    /// ⌘1…⌘9: the nth cell in reading order (1-based).
    pub fn focus_nth(&mut self, n: usize) {
        let Some(id) = n.checked_sub(1).and_then(|i| self.cells().nth(i)).map(|c| c.id) else { return };
        self.focused = Some(id);
        if self.maximized.is_some() {
            self.maximized = Some(id);
        }
    }

    /// Next or previous tab in the focused cell, wrapping around.
    pub fn cycle_tab(&mut self, forward: bool) {
        let Some(at) = self.target(None) else { return };
        let cell = self.cell_at(at);
        let len = cell.tabs.len();
        let index = cell.tabs.iter().position(|t| *t == cell.active).unwrap_or(0);
        let next = if forward { (index + 1) % len } else { (index + len - 1) % len };
        cell.active = cell.tabs[next];
    }

    /// Moves focus to the neighbouring cell; up and down pick the cell most directly above or below.
    pub fn focus_direction(&mut self, direction: Direction) {
        let Some(from) = self.target(None) else { return };
        let target = match direction {
            Direction::Left => from.cell.checked_sub(1).map(|cell| Position { cell, ..from }),
            Direction::Right => {
                (from.cell + 1 < self.rows[from.row].cells.len()).then_some(Position { cell: from.cell + 1, ..from })
            }
            Direction::Up | Direction::Down => {
                let row = if direction == Direction::Up { from.row.checked_sub(1) } else { Some(from.row + 1) };
                row.filter(|r| *r < self.rows.len()).map(|row| {
                    let x = centres(&self.rows[from.row])[from.cell];
                    let candidates = centres(&self.rows[row]);
                    let mut best = 0;
                    for (index, centre) in candidates.iter().enumerate() {
                        if (centre - x).abs() < (candidates[best] - x).abs() {
                            best = index;
                        }
                    }
                    Position { row, cell: best }
                })
            }
        };
        if let Some(at) = target {
            self.focused = Some(self.rows[at.row].cells[at.cell].id);
        }
    }

    pub fn toggle_maximize(&mut self) {
        self.maximized = if self.maximized.is_some() { None } else { self.focused };
    }

    pub fn resize_rows(&mut self, sizes: &[f32]) {
        for (row, size) in self.rows.iter_mut().zip(sizes) {
            row.size = *size;
        }
    }

    pub fn resize_cells(&mut self, row: RowId, sizes: &[f32]) {
        if let Some(row) = self.rows.iter_mut().find(|r| r.id == row) {
            for (cell, size) in row.cells.iter_mut().zip(sizes) {
                cell.size = *size;
            }
        }
    }
}

/// The horizontal centre of each cell in a row, from 0 to 1.
fn centres(row: &Row) -> Vec<f32> {
    let total: f32 = row.cells.iter().map(|c| c.size).sum();
    let mut start = 0.0;
    row.cells
        .iter()
        .map(|cell| {
            let centre = (start + cell.size / 2.0) / total;
            start += cell.size;
            centre
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: TabId = TabId(1);
    const B: TabId = TabId(2);
    const C: TabId = TabId(3);
    const D: TabId = TabId(4);
    const E: TabId = TabId(5);

    /// The grid as tab numbers per cell, row by row.
    fn shape(layout: &Layout) -> Vec<Vec<Vec<u64>>> {
        layout
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.tabs.iter().map(|t| t.0).collect()).collect())
            .collect()
    }

    fn cell_of(layout: &Layout, tab: TabId) -> CellId {
        layout.cell_of_tab(tab).map(|c| c.id).expect("tab is in the grid")
    }

    #[test]
    fn starts_a_grid_with_the_first_tab() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        assert_eq!(shape(&layout), [[[1]]]);
        assert_eq!(layout.focused_tab(), Some(A));
    }

    #[test]
    fn adds_a_tab_after_the_active_one() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.add_tab(B, None);
        layout.activate_tab(A);
        layout.add_tab(C, None);
        assert_eq!(shape(&layout), [[[1, 3, 2]]]);
        assert_eq!(layout.focused_tab(), Some(C));
    }

    #[test]
    fn splits_right_into_a_cell_sharing_the_width() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        assert_eq!(shape(&layout), [[[1], [2]]]);
        assert_eq!(layout.rows[0].cells.iter().map(|c| c.size).collect::<Vec<_>>(), [0.5, 0.5]);
        assert_eq!(layout.focused_tab(), Some(B));
    }

    #[test]
    fn splits_down_into_a_new_row() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        layout.split_down(C, None);
        assert_eq!(shape(&layout), vec![vec![vec![1], vec![2]], vec![vec![3]]]);
        assert_eq!(layout.rows.iter().map(|r| r.size).collect::<Vec<_>>(), [0.5, 0.5]);
    }

    #[test]
    fn activates_the_previous_tab_when_the_active_one_closes() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.add_tab(B, None);
        layout.add_tab(C, None);
        layout.activate_tab(B);
        layout.remove_tab(B);
        assert_eq!(shape(&layout), [[[1, 3]]]);
        assert_eq!(layout.focused_tab(), Some(A));
    }

    #[test]
    fn drops_an_emptied_cell_and_row_and_focuses_the_neighbour() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        layout.split_down(C, None);
        layout.remove_tab(C);
        assert_eq!(shape(&layout), [[[1], [2]]]);
        assert_eq!(layout.focused_tab(), Some(B));
    }

    #[test]
    fn empties_the_grid_when_the_last_tab_closes() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.remove_tab(A);
        assert!(layout.is_empty());
        assert_eq!(layout.focused, None);
    }

    #[test]
    fn clears_maximise_when_the_maximised_cell_goes() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        layout.toggle_maximize();
        layout.remove_tab(B);
        assert_eq!(layout.maximized, None);
    }

    #[test]
    fn moves_a_tab_into_another_cell_and_focuses_it_there() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.add_tab(B, None);
        layout.split_right(C, None);
        let target = cell_of(&layout, C);
        layout.move_tab(A, target, Some(0));
        assert_eq!(shape(&layout), vec![vec![vec![2], vec![1, 3]]]);
        assert_eq!(layout.focused_tab(), Some(A));
    }

    #[test]
    fn removes_the_source_cell_when_its_last_tab_moves_away() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        let target = cell_of(&layout, B);
        layout.move_tab(A, target, None);
        assert_eq!(shape(&layout), [[[2, 1]]]);
        assert_eq!(layout.focused, Some(target));
    }

    #[test]
    fn reorders_within_a_cell() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.add_tab(B, None);
        layout.add_tab(C, None);
        let cell = cell_of(&layout, C);
        layout.move_tab(C, cell, Some(0));
        assert_eq!(shape(&layout), [[[3, 1, 2]]]);
    }

    #[test]
    fn keeps_every_tab_exactly_once() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        layout.split_down(C, None);
        layout.add_tab(D, None);
        let target = cell_of(&layout, D);
        layout.move_tab(B, target, Some(1));
        let mut tabs: Vec<u64> = layout.tabs().map(|t| t.0).collect();
        tabs.sort_unstable();
        assert_eq!(tabs, [1, 2, 3, 4]);
    }

    // a | b | c
    //   d   | e
    fn grid() -> Layout {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.split_right(B, None);
        layout.split_right(C, None);
        layout.split_down(D, None);
        layout.split_right(E, None);
        layout.focus_nth(1);
        layout
    }

    #[test]
    fn counts_cells_in_reading_order() {
        let layout = grid();
        assert_eq!(layout.cells().map(|c| c.active.0).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        let mut fourth = grid();
        fourth.focus_nth(4);
        assert_eq!(fourth.focused_tab(), Some(D));
        let mut ninth = grid();
        ninth.focus_nth(9);
        assert_eq!(ninth, layout);
    }

    #[test]
    fn moves_left_and_right_within_a_row_and_stops_at_the_edge() {
        let mut layout = grid();
        layout.focus_direction(Direction::Right);
        assert_eq!(layout.focused_tab(), Some(B));
        let mut edge = grid();
        edge.focus_direction(Direction::Left);
        assert_eq!(edge, grid());
    }

    #[test]
    fn moves_up_and_down_to_the_cell_most_directly_across() {
        let mut layout = grid();
        layout.focus_direction(Direction::Down);
        assert_eq!(layout.focused_tab(), Some(D));
        layout.focus_nth(3);
        layout.focus_direction(Direction::Down);
        assert_eq!(layout.focused_tab(), Some(E));
        layout.focus_nth(4);
        layout.focus_direction(Direction::Up);
        assert_eq!(layout.focused_tab(), Some(A));
    }

    #[test]
    fn cycles_tabs_in_the_focused_cell() {
        let mut layout = Layout::default();
        layout.add_tab(A, None);
        layout.add_tab(B, None);
        layout.cycle_tab(true);
        assert_eq!(layout.focused_tab(), Some(A));
        layout.cycle_tab(false);
        assert_eq!(layout.focused_tab(), Some(B));
    }

    #[test]
    fn shows_only_the_maximised_cell() {
        let mut layout = grid();
        assert_eq!(layout.visible_tabs(), [A, B, C, D, E]);
        layout.focus_nth(2);
        layout.toggle_maximize();
        assert_eq!(layout.visible_tabs(), [B]);
    }
}
