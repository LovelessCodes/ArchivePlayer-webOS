//! Fixed 1920x1080 layout for the 10-foot UI.
//!
//! Keeping geometry in one module means focus rectangles and painted
//! rectangles cannot drift apart as screens change.

use archiveplayer_tv_gfx::Rect;
use archiveplayer_tv_ui::theme::Theme;

pub const CARD_GAP: i32 = 24;
pub const POSTER_W: i32 = 236;
pub const POSTER_H: i32 = 315;
pub const SIDEBAR_ITEM_H: i32 = 56;

pub fn nav_items() -> [Rect; 4] {
    std::array::from_fn(|index| Rect::new(20, 176 + index as i32 * 64, 192, SIDEBAR_ITEM_H))
}

pub fn logo_mark() -> Rect {
    Rect::new(24, 28, 46, 46)
}

pub fn logo_text() -> Rect {
    Rect::new(84, 34, 130, 34)
}

pub fn server_pill() -> Rect {
    Rect::new(20, 944, 192, 72)
}

pub fn topbar_title() -> Rect {
    Rect::new(Theme::CONTENT_X, 34, 900, 48)
}

pub fn topbar_subtitle() -> Rect {
    Rect::new(Theme::CONTENT_X, 78, 900, 24)
}

pub fn topbar_clock() -> Rect {
    Rect::new(1630, 34, 226, 40)
}

pub fn topbar_status() -> Rect {
    Rect::new(1350, 42, 260, 40)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeLayout {
    pub hero: Rect,
    pub hero_poster: Rect,
    pub play: Rect,
    pub details: Rect,
    pub continue_cards: Vec<Rect>,
    pub recent_cards: Vec<Rect>,
    pub section_continue: Rect,
    pub section_recent: Rect,
}

pub fn home(continue_len: usize, recent_len: usize) -> HomeLayout {
    let continue_len = continue_len.min(4);
    let recent_len = recent_len.min(4);
    let (recent_y, card_y) = if continue_len == 0 {
        (570, 604)
    } else {
        (854, 888)
    };
    HomeLayout {
        hero: Rect::new(Theme::CONTENT_X, 124, Theme::CONTENT_W, 408),
        hero_poster: Rect::new(1368, 154, 424, 348),
        play: Rect::new(324, 438, 196, 62),
        details: Rect::new(540, 438, 196, 62),
        section_continue: Rect::new(Theme::CONTENT_X, 570, 420, 44),
        section_recent: Rect::new(Theme::CONTENT_X, recent_y, 420, 44),
        continue_cards: (0..continue_len)
            .map(|index| {
                Rect::new(
                    Theme::CONTENT_X + index as i32 * (370 + CARD_GAP),
                    604,
                    370,
                    190,
                )
            })
            .collect(),
        recent_cards: (0..recent_len)
            .map(|index| {
                Rect::new(
                    Theme::CONTENT_X + index as i32 * (370 + CARD_GAP),
                    card_y,
                    370,
                    148,
                )
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryLayout {
    pub chips: Vec<Rect>,
    pub cards: Vec<Rect>,
}

pub fn library(chip_count: usize, card_count: usize) -> LibraryLayout {
    LibraryLayout {
        chips: (0..chip_count)
            .map(|index| Rect::new(Theme::CONTENT_X + index as i32 * 168, 138, 156, 44))
            .collect(),
        cards: (0..card_count)
            .map(|index| {
                let column = index as i32 % 6;
                let row = index as i32 / 6;
                Rect::new(
                    Theme::CONTENT_X + column * (POSTER_W + CARD_GAP),
                    224 + row * 396,
                    POSTER_W,
                    POSTER_H,
                )
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailLayout {
    pub backdrop: Rect,
    pub back: Rect,
    pub poster: Rect,
    pub play: Rect,
    pub episodes: Vec<Rect>,
    pub info_cards: Vec<Rect>,
}

pub fn detail(episode_count: usize) -> DetailLayout {
    DetailLayout {
        backdrop: Rect::new(0, 0, Theme::WIDTH, 628),
        back: Rect::new(Theme::CONTENT_X, 48, 128, 46),
        poster: Rect::new(1352, 132, 360, 480),
        play: Rect::new(324, 496, 216, 62),
        episodes: (0..episode_count.min(4))
            .map(|index| {
                Rect::new(
                    Theme::CONTENT_X + index as i32 * (370 + CARD_GAP),
                    738,
                    370,
                    164,
                )
            })
            .collect(),
        info_cards: (0..3)
            .map(|index| Rect::new(Theme::CONTENT_X + index * 508, 738, 484, 164))
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLayout {
    pub input: Rect,
    pub chips: Vec<Rect>,
    pub results: Vec<Rect>,
}

pub fn search(chip_count: usize, result_count: usize) -> SearchLayout {
    SearchLayout {
        input: Rect::new(Theme::CONTENT_X, 128, 960, 64),
        chips: (0..chip_count)
            .map(|index| Rect::new(Theme::CONTENT_X + index as i32 * 180, 222, 168, 42))
            .collect(),
        results: (0..result_count)
            .map(|index| {
                let column = index as i32 % 6;
                let row = index as i32 / 6;
                Rect::new(
                    Theme::CONTENT_X + column * (POSTER_W + CARD_GAP),
                    330 + row * 380,
                    POSTER_W,
                    POSTER_H,
                )
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsLayout {
    pub cards: Vec<Rect>,
    pub pair: Rect,
    pub reconnect: Rect,
}

pub fn settings() -> SettingsLayout {
    SettingsLayout {
        cards: vec![
            Rect::new(Theme::CONTENT_X, 140, 760, 244),
            Rect::new(Theme::CONTENT_X + 784, 140, 760, 244),
            Rect::new(Theme::CONTENT_X, 412, 760, 244),
            Rect::new(Theme::CONTENT_X + 784, 412, 760, 244),
        ],
        pair: Rect::new(Theme::CONTENT_X, 716, 230, 58),
        reconnect: Rect::new(Theme::CONTENT_X + 254, 716, 230, 58),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairLayout {
    pub card: Rect,
    pub code: Rect,
    pub qr: Rect,
    pub simulate: Rect,
    pub back: Rect,
}

pub fn pair() -> PairLayout {
    PairLayout {
        card: Rect::new(360, 218, 1200, 644),
        code: Rect::new(444, 514, 400, 84),
        qr: Rect::new(1190, 320, 280, 280),
        simulate: Rect::new(444, 720, 260, 60),
        back: Rect::new(730, 720, 180, 60),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerLayout {
    pub back: Rect,
    pub rewind: Rect,
    pub play: Rect,
    pub forward: Rect,
    pub progress: Rect,
}

pub fn player() -> PlayerLayout {
    PlayerLayout {
        back: Rect::new(72, 64, 128, 50),
        rewind: Rect::new(818, 916, 76, 76),
        play: Rect::new(922, 906, 96, 96),
        forward: Rect::new(1046, 916, 76, 76),
        progress: Rect::new(280, 1022, 1360, 8),
    }
}
