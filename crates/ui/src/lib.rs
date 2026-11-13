//! Platform-independent UI scaffolding.
//!
//! The renderer and SDL layer will consume this model; keeping focus logic here
//! makes it testable without a television or GLES context.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Pair,
    Home,
    Library,
    Search,
    Detail,
    Player,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMove {
    Up,
    Down,
    Left,
    Right,
}

/// A simple row-major focus grid. Screens with irregular layouts can own their
/// own geometry later; this covers posters, episode lists, and settings rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusGrid {
    columns: usize,
    len: usize,
    index: usize,
}

impl FocusGrid {
    pub fn new(columns: usize, len: usize) -> Self {
        Self {
            columns: columns.max(1),
            len,
            index: 0,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        if self.index >= len {
            self.index = len.saturating_sub(1);
        }
    }

    pub fn focus(&mut self, index: usize) {
        if self.len > 0 {
            self.index = index.min(self.len - 1);
        }
    }

    pub fn mv(&mut self, direction: FocusMove) {
        if self.len == 0 {
            return;
        }
        let columns = self.columns.min(self.len).max(1);
        self.index = match direction {
            FocusMove::Left => self.index.saturating_sub(1),
            FocusMove::Right => (self.index + 1).min(self.len - 1),
            FocusMove::Up => self.index.saturating_sub(columns),
            FocusMove::Down => (self.index + columns).min(self.len - 1),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_in_a_row_major_grid() {
        let mut grid = FocusGrid::new(4, 10);
        grid.mv(FocusMove::Right);
        assert_eq!(grid.index(), 1);
        grid.mv(FocusMove::Down);
        assert_eq!(grid.index(), 5);
        grid.mv(FocusMove::Left);
        assert_eq!(grid.index(), 4);
        grid.mv(FocusMove::Up);
        assert_eq!(grid.index(), 0);
    }

    #[test]
    fn clamps_at_edges_and_shrinks() {
        let mut grid = FocusGrid::new(3, 5);
        grid.focus(4);
        grid.mv(FocusMove::Right);
        assert_eq!(grid.index(), 4);
        grid.mv(FocusMove::Down);
        assert_eq!(grid.index(), 4);
        grid.set_len(2);
        assert_eq!(grid.index(), 1);
        grid.set_len(0);
        assert_eq!(grid.index(), 0);
        grid.mv(FocusMove::Down);
        assert_eq!(grid.index(), 0);
    }
}
