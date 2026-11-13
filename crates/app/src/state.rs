//! Application state, focus targets, and actions for the native shell.

use archiveplayer_tv_gfx::Rect;
use archiveplayer_tv_ui::{self as ui, FocusMove, Screen};

use crate::demo::{DemoItem, DemoLibrary};
use crate::layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Go(Screen),
    Open(usize),
    Play(usize),
    Back,
    SetCategory(usize),
    FocusSearch,
    SearchSuggest(usize),
    Pair,
    PairSimulate,
    Reconnect,
    TogglePlay,
    Seek(i64),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget {
    pub rect: Rect,
    pub action: Action,
}

pub const NAV_SCREENS: [Screen; 4] = [
    Screen::Home,
    Screen::Library,
    Screen::Search,
    Screen::Settings,
];

pub const SEARCH_SUGGESTIONS: [&str; 4] = ["Documentary", "Noir", "Silent film", "Sci-Fi"];

pub struct App {
    pub screen: Screen,
    pub previous: Option<Screen>,
    pub focus_index: usize,
    pub library: DemoLibrary,
    pub selected: usize,
    pub category: usize,
    pub search_query: String,
    pub search_editing: bool,
    pub player_position: f64,
    pub player_duration: f64,
    pub player_playing: bool,
    pub paired: bool,
    pub clock: String,
    pub date: String,
    pub toast: Option<String>,
    pub toast_frames: u32,
    pub frame: u64,
}

impl App {
    pub fn demo() -> Self {
        let library = DemoLibrary::demo();
        let featured = library.featured();
        let (player_position, player_duration) = featured.progress.unwrap_or((0, 0));
        Self {
            screen: Screen::Home,
            previous: None,
            focus_index: 0,
            library,
            selected: 0,
            category: 0,
            search_query: String::new(),
            search_editing: false,
            player_position: player_position as f64,
            player_duration: player_duration.max(1) as f64,
            player_playing: false,
            paired: true,
            clock: "20:41".into(),
            date: "Fri 13 Nov".into(),
            toast: None,
            toast_frames: 0,
            frame: 0,
        }
    }

    pub fn is_focused(&self, rect: Rect) -> bool {
        self.targets()
            .get(self.focus_index)
            .map(|target| target.rect == rect)
            .unwrap_or(false)
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(message.into());
        self.toast_frames = 180;
    }

    pub fn selected_item(&self) -> &DemoItem {
        self.library.item(self.selected)
    }

    pub fn visible_items(&self) -> Vec<usize> {
        match self.category {
            0 => (0..self.library.items.len()).collect(),
            1 => self
                .library
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| !item.is_series())
                .map(|(index, _)| index)
                .collect(),
            2 => self
                .library
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.is_series())
                .map(|(index, _)| index)
                .collect(),
            3 => self.find_genre("Documentaries"),
            4 => self.find_genre("Noir"),
            5 => self.find_genre("Sci-Fi"),
            _ => (0..self.library.items.len()).collect(),
        }
    }

    fn find_genre(&self, genre: &str) -> Vec<usize> {
        let needle = genre.trim_end_matches('s').to_lowercase();
        self.library
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.genres
                    .iter()
                    .any(|item_genre| item_genre.to_lowercase().contains(&needle))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn search_results(&self) -> Vec<usize> {
        self.library
            .search(&self.search_query)
            .into_iter()
            .take(6)
            .collect()
    }

    pub fn targets(&self) -> Vec<FocusTarget> {
        let mut targets = Vec::new();
        if !matches!(self.screen, Screen::Pair | Screen::Player | Screen::Detail) {
            for (index, screen) in NAV_SCREENS.iter().enumerate() {
                targets.push(FocusTarget {
                    rect: layout::nav_items()[index],
                    action: Action::Go(*screen),
                });
            }
        }
        match self.screen {
            Screen::Home => {
                let continue_items = self.library.continue_watching();
                let recent_items = self.library.recently_added();
                let home = layout::home(continue_items.len(), recent_items.len());
                targets.push(FocusTarget {
                    rect: home.play,
                    action: Action::Play(0),
                });
                targets.push(FocusTarget {
                    rect: home.details,
                    action: Action::Open(0),
                });
                for (rect, index) in home.continue_cards.iter().zip(continue_items) {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::Open(index),
                    });
                }
                for (rect, index) in home.recent_cards.iter().zip(recent_items) {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::Open(index),
                    });
                }
            }
            Screen::Library => {
                let items = self.visible_items();
                let library = layout::library(self.library.categories.len(), items.len());
                for (index, rect) in library.chips.iter().enumerate() {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::SetCategory(index),
                    });
                }
                for (rect, index) in library.cards.iter().zip(items) {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::Open(index),
                    });
                }
            }
            Screen::Detail => {
                let detail = layout::detail(self.selected_item().episodes.len());
                targets.push(FocusTarget {
                    rect: detail.back,
                    action: Action::Back,
                });
                targets.push(FocusTarget {
                    rect: detail.play,
                    action: Action::Play(self.selected),
                });
                for (rect, episode) in detail.episodes.iter().zip(&self.selected_item().episodes) {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::Play(self.selected),
                    });
                    let _ = episode;
                }
            }
            Screen::Search => {
                let results = self.search_results();
                let search = layout::search(SEARCH_SUGGESTIONS.len(), results.len());
                targets.push(FocusTarget {
                    rect: search.input,
                    action: Action::FocusSearch,
                });
                for (index, rect) in search.chips.iter().enumerate() {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::SearchSuggest(index),
                    });
                }
                for (rect, index) in search.results.iter().zip(results) {
                    targets.push(FocusTarget {
                        rect: *rect,
                        action: Action::Open(index),
                    });
                }
            }
            Screen::Settings => {
                let settings = layout::settings();
                targets.push(FocusTarget {
                    rect: settings.pair,
                    action: Action::Pair,
                });
                targets.push(FocusTarget {
                    rect: settings.reconnect,
                    action: Action::Reconnect,
                });
            }
            Screen::Pair => {
                let pair = layout::pair();
                targets.push(FocusTarget {
                    rect: pair.simulate,
                    action: Action::PairSimulate,
                });
                targets.push(FocusTarget {
                    rect: pair.back,
                    action: Action::Back,
                });
            }
            Screen::Player => {
                let player = layout::player();
                targets.push(FocusTarget {
                    rect: player.back,
                    action: Action::Back,
                });
                targets.push(FocusTarget {
                    rect: player.rewind,
                    action: Action::Seek(-10),
                });
                targets.push(FocusTarget {
                    rect: player.play,
                    action: Action::TogglePlay,
                });
                targets.push(FocusTarget {
                    rect: player.forward,
                    action: Action::Seek(10),
                });
            }
        }
        targets
    }

    pub fn set_screen(&mut self, screen: Screen) {
        if self.screen != screen {
            self.previous = Some(self.screen);
            self.screen = screen;
            self.focus_index = 0;
            self.search_editing = false;
        }
        if screen == Screen::Player {
            let item = self.selected_item();
            let (position, duration) = item
                .progress
                .unwrap_or((0, item.runtime_minutes as i64 * 60));
            self.player_position = position as f64;
            self.player_duration = duration.max(1) as f64;
            self.player_playing = true;
        }
    }

    pub fn back(&mut self) {
        if self.search_editing {
            self.search_editing = false;
            return;
        }
        if self.screen == Screen::Home {
            return;
        }
        let target = self.previous.unwrap_or(Screen::Home);
        self.screen = target;
        self.previous = None;
        self.focus_index = 0;
    }

    pub fn move_focus(&mut self, direction: FocusMove) {
        if self.search_editing {
            return;
        }
        let targets = self.targets();
        let rects: Vec<Rect> = targets.iter().map(|target| target.rect).collect();
        self.focus_index = ui::navigate(&rects, self.focus_index, direction);
    }

    pub fn activate(&mut self) {
        let targets = self.targets();
        let Some(target) = targets.get(self.focus_index) else {
            return;
        };
        match target.action {
            Action::Go(screen) => self.set_screen(screen),
            Action::Open(index) => {
                self.selected = index;
                self.set_screen(Screen::Detail);
            }
            Action::Play(index) => {
                self.selected = index;
                self.set_screen(Screen::Player);
            }
            Action::Back => self.back(),
            Action::SetCategory(index) => self.category = index,
            Action::FocusSearch => self.search_editing = true,
            Action::SearchSuggest(index) => {
                if let Some(suggestion) = SEARCH_SUGGESTIONS.get(index) {
                    self.search_query = (*suggestion).to_string();
                    self.search_editing = false;
                }
            }
            Action::Pair => self.set_screen(Screen::Pair),
            Action::PairSimulate => {
                self.paired = true;
                self.show_toast("Device paired with Living room");
                self.set_screen(Screen::Home);
            }
            Action::Reconnect => {
                self.show_toast("Connection is healthy");
            }
            Action::TogglePlay => self.player_playing = !self.player_playing,
            Action::Seek(delta) => {
                self.player_position =
                    (self.player_position + delta as f64).clamp(0.0, self.player_duration);
            }
            Action::None => {}
        }
    }

    pub fn on_text(&mut self, text: &str) {
        if self.search_editing {
            for character in text.chars().filter(|character| !character.is_control()) {
                self.search_query.push(character);
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if self.search_editing {
            self.search_query.pop();
        }
    }

    pub fn tick(&mut self, seconds: f64) {
        self.frame = self.frame.wrapping_add(1);
        if self.toast_frames > 0 {
            self.toast_frames -= 1;
            if self.toast_frames == 0 {
                self.toast = None;
            }
        }
        if self.screen == Screen::Player && self.player_playing {
            self.player_position += seconds;
            if self.player_position >= self.player_duration {
                self.player_position = self.player_duration;
                self.player_playing = false;
            }
        }
    }

    pub fn player_fraction(&self) -> f32 {
        (self.player_position / self.player_duration.max(1.0)).clamp(0.0, 1.0) as f32
    }

    pub fn player_time_label(&self) -> String {
        let position = self.player_position.round() as i64;
        format!("{}:{:02}", position / 60, position.rem_euclid(60))
    }

    pub fn player_duration_label(&self) -> String {
        let duration = self.player_duration.round() as i64;
        format!("{}:{:02}", duration / 60, duration.rem_euclid(60))
    }
}
