//! Platform-independent UI scaffolding.
//!
//! The renderer and SDL layer consume this model; keeping focus and theme here
//! makes them testable without a television or GLES context.

pub mod theme;

use archiveplayer_tv_gfx::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// A simple row-major focus grid. Screens with irregular layouts use
/// [`navigate`] instead; this covers posters, episode lists, and settings rows.
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

/// Pick the nearest focusable rectangle in a direction.
///
/// This is deliberately geometric rather than grid-based: the home screen,
/// sidebar, and player controls do not share a column count. Candidates must be
/// at least partly in the requested direction, and the perpendicular distance
/// is weighted so a same-row neighbour wins over a diagonal one.
pub fn navigate(rects: &[Rect], current: usize, direction: FocusMove) -> usize {
    if rects.is_empty() || current >= rects.len() {
        return current;
    }
    let source = rects[current];
    let (sx, sy) = source.center();
    let mut best: Option<(usize, f32)> = None;
    for (index, rect) in rects.iter().enumerate() {
        if index == current {
            continue;
        }
        let (cx, cy) = rect.center();
        let dx = cx - sx;
        let dy = cy - sy;
        let (primary, secondary) = match direction {
            FocusMove::Right if dx > 8 => (dx as f32, dy.abs() as f32),
            FocusMove::Left if dx < -8 => ((-dx) as f32, dy.abs() as f32),
            FocusMove::Down if dy > 8 => (dy as f32, dx.abs() as f32),
            FocusMove::Up if dy < -8 => ((-dy) as f32, dx.abs() as f32),
            _ => continue,
        };
        // Prefer rectangles that overlap the source on the perpendicular axis.
        let overlap = match direction {
            FocusMove::Left | FocusMove::Right => {
                source.y < rect.bottom() && source.bottom() > rect.y
            }
            FocusMove::Up | FocusMove::Down => source.x < rect.right() && source.right() > rect.x,
        };
        let overlap_penalty = if overlap { 0.0 } else { 240.0 };
        let score = primary + secondary * 2.5 + overlap_penalty;
        if best.map(|(_, best)| score < best).unwrap_or(true) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index).unwrap_or(current)
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

    #[test]
    fn spatial_navigation_prefers_the_same_row() {
        let rects = [
            Rect::new(0, 0, 100, 100),
            Rect::new(140, 0, 100, 100),
            Rect::new(140, 180, 100, 100),
            Rect::new(0, 180, 100, 100),
        ];
        assert_eq!(navigate(&rects, 0, FocusMove::Right), 1);
        assert_eq!(navigate(&rects, 1, FocusMove::Down), 2);
        assert_eq!(navigate(&rects, 2, FocusMove::Left), 3);
        assert_eq!(navigate(&rects, 3, FocusMove::Up), 0);
    }

    #[test]
    fn spatial_navigation_reaches_the_sidebar() {
        let rects = [Rect::new(24, 180, 184, 56), Rect::new(280, 132, 900, 420)];
        assert_eq!(navigate(&rects, 1, FocusMove::Left), 0);
        assert_eq!(navigate(&rects, 0, FocusMove::Right), 1);
    }
}
