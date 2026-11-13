//! Shared palette and spacing for the 10-foot UI.
//!
//! The values mirror the web client's dark theme so the TV app feels like the
//! same product without copying its DOM layout.

use archiveplayer_tv_gfx::Color;

pub struct Theme;

impl Theme {
    pub const BG: Color = Color::rgb(9, 11, 16);
    pub const BG_SOFT: Color = Color::rgb(13, 16, 23);
    pub const RAIL: Color = Color::rgb(7, 9, 14);
    pub const PANEL: Color = Color::rgb(20, 23, 32);
    pub const PANEL_RAISED: Color = Color::rgb(27, 31, 43);
    pub const PANEL_HOVER: Color = Color::rgb(35, 40, 55);
    pub const BORDER: Color = Color::rgb(38, 43, 58);
    pub const BORDER_SOFT: Color = Color::rgb(29, 33, 45);
    pub const TEXT: Color = Color::rgb(244, 245, 248);
    pub const TEXT_SOFT: Color = Color::rgb(201, 205, 216);
    pub const MUTED: Color = Color::rgb(139, 145, 161);
    pub const DIM: Color = Color::rgb(91, 97, 113);
    pub const PRIMARY: Color = Color::rgb(229, 72, 58);
    pub const PRIMARY_LIGHT: Color = Color::rgb(255, 122, 103);
    pub const PRIMARY_DARK: Color = Color::rgb(164, 43, 35);
    pub const SUCCESS: Color = Color::rgb(78, 201, 140);
    pub const AMBER: Color = Color::rgb(239, 174, 75);
    pub const DANGER: Color = Color::rgb(235, 94, 94);

    pub const SIDEBAR: i32 = 232;
    pub const CONTENT_X: i32 = 280;
    pub const CONTENT_W: i32 = 1576;
    pub const TOPBAR_H: i32 = 112;
    pub const WIDTH: i32 = 1920;
    pub const HEIGHT: i32 = 1080;
    pub const RADIUS: f32 = 16.0;
}

pub const SIDEBAR_ITEMS: [(&str, &str); 4] = [
    ("home", "Home"),
    ("library", "Library"),
    ("search", "Search"),
    ("settings", "Settings"),
];
