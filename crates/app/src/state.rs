//! Application state, focus targets, and actions for the native shell.

use archiveplayer_tv_gfx::Rect;
use archiveplayer_tv_ui::{self as ui, FocusMove, Screen};

use crate::catalog::{Catalog, MediaItem};
use crate::layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingStatus {
    Connecting,
    Waiting,
    Approved,
    Denied,
    Expired,
    Error,
}

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
    PairRefresh,
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

const DEFAULT_SEARCH_SUGGESTIONS: [&str; 4] = ["Documentary", "Noir", "Silent film", "Sci-Fi"];

pub struct App {
    pub screen: Screen,
    pub previous: Option<Screen>,
    pub focus_index: usize,
    pub library: Catalog,
    pub selected: usize,
    pub category: usize,
    pub search_query: String,
    pub search_editing: bool,
    pub player_position: f64,
    pub player_duration: f64,
    pub player_playing: bool,
    pub paired: bool,
    pub live: bool,
    pub server_name: String,
    pub server_url: String,
    pub server_version: String,
    pub server_lan: bool,
    pub pairing_code: String,
    pub pairing_uri: String,
    pub pairing_complete_uri: String,
    pub pairing_status: PairingStatus,
    pub pairing_seconds_left: i64,
    pub clock: String,
    pub date: String,
    pub toast: Option<String>,
    pub toast_frames: u32,
    pub frame: u64,
}

impl App {
    pub fn demo() -> Self {
        let library = Catalog::demo();
        let featured = library.featured().expect("the demo catalogue is not empty");
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
            live: false,
            server_name: "Living room".into(),
            server_url: "http://192.168.1.20:8080".into(),
            server_version: "0.5.0".into(),
            server_lan: true,
            pairing_code: "ABCD–EFGH".into(),
            pairing_uri: "http://192.168.1.20:8080/#/activate".into(),
            pairing_complete_uri: "http://192.168.1.20:8080/#/activate?code=ABCD-EFGH".into(),
            pairing_status: PairingStatus::Waiting,
            pairing_seconds_left: 582,
            clock: "20:41".into(),
            date: "Fri 13 Nov".into(),
            toast: None,
            toast_frames: 0,
            frame: 0,
        }
    }

    /// Switch from the demo catalogue to a server catalogue. The selected index
    /// is clamped so a refresh cannot leave the Detail screen pointing at a
    /// removed title.
    pub fn set_catalog(&mut self, catalog: Catalog) {
        let was_pairing = self.screen == Screen::Pair;
        self.library = catalog;
        self.selected = self
            .selected
            .min(self.library.items.len().saturating_sub(1));
        if was_pairing && self.paired {
            self.set_screen(Screen::Home);
        }
        if self.library.items.is_empty() && matches!(self.screen, Screen::Detail | Screen::Player) {
            self.set_screen(Screen::Home);
        }
    }

    pub fn set_connection(
        &mut self,
        name: impl Into<String>,
        url: impl Into<String>,
        version: impl Into<String>,
        lan: bool,
        paired: bool,
    ) {
        self.server_name = name.into();
        self.server_url = url.into();
        self.server_version = version.into();
        self.server_lan = lan;
        self.paired = paired;
        if !paired {
            self.pairing_status = PairingStatus::Waiting;
            if self.screen != Screen::Pair {
                self.previous = Some(self.screen);
                self.screen = Screen::Pair;
                self.focus_index = 0;
            }
        } else if self.screen == Screen::Pair {
            self.screen = Screen::Home;
            self.previous = None;
            self.focus_index = 0;
        }
    }

    pub fn set_pairing_code(
        &mut self,
        code: impl Into<String>,
        uri: impl Into<String>,
        complete_uri: impl Into<String>,
        expires_in_seconds: i64,
    ) {
        self.pairing_code = code.into();
        self.pairing_uri = uri.into();
        self.pairing_complete_uri = complete_uri.into();
        self.pairing_seconds_left = expires_in_seconds;
        self.pairing_status = PairingStatus::Waiting;
        if self.screen != Screen::Pair {
            self.previous = Some(self.screen);
            self.screen = Screen::Pair;
            self.focus_index = 0;
        }
    }

    pub fn set_pairing_status(&mut self, status: PairingStatus) {
        self.pairing_status = status;
    }

    /// An empty live client. Pairing is the first screen; the worker fills the
    /// catalogue as soon as the session is approved.
    pub fn live(server_name: impl Into<String>, server_url: impl Into<String>) -> Self {
        Self {
            screen: Screen::Pair,
            previous: Some(Screen::Home),
            focus_index: 0,
            library: Catalog::empty(),
            selected: 0,
            category: 0,
            search_query: String::new(),
            search_editing: false,
            player_position: 0.0,
            player_duration: 1.0,
            player_playing: false,
            paired: false,
            live: true,
            server_name: server_name.into(),
            server_url: server_url.into(),
            server_version: String::new(),
            server_lan: true,
            pairing_code: String::new(),
            pairing_uri: String::new(),
            pairing_complete_uri: String::new(),
            pairing_status: PairingStatus::Connecting,
            pairing_seconds_left: 0,
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

    pub fn selected_item(&self) -> &MediaItem {
        self.library.item(self.selected)
    }

    pub fn visible_items(&self) -> Vec<usize> {
        if self.category == 0 {
            return (0..self.library.items.len()).collect();
        }
        let Some(name) = self.library.categories.get(self.category) else {
            return (0..self.library.items.len()).collect();
        };
        let name = name.trim();
        if name.eq_ignore_ascii_case("movies") {
            return self
                .library
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.media_type == crate::catalog::MediaType::Movie)
                .map(|(index, _)| index)
                .collect();
        }
        if name.eq_ignore_ascii_case("series") {
            return self
                .library
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.is_series())
                .map(|(index, _)| index)
                .collect();
        }
        let needle = name.trim_end_matches('s').to_lowercase();
        self.library
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.category.to_lowercase().contains(&needle)
                    || item
                        .genres
                        .iter()
                        .any(|genre| genre.to_lowercase().contains(&needle))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn search_suggestions(&self) -> Vec<String> {
        let suggestions = self.library.suggestions();
        if suggestions.is_empty() {
            DEFAULT_SEARCH_SUGGESTIONS
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        } else {
            suggestions
        }
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
                let suggestions = self.search_suggestions();
                let search = layout::search(suggestions.len(), results.len());
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
                    action: if self.live {
                        Action::PairRefresh
                    } else {
                        Action::PairSimulate
                    },
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

    pub fn activate(&mut self) -> Action {
        let targets = self.targets();
        let Some(target) = targets.get(self.focus_index) else {
            return Action::None;
        };
        let action = target.action;
        match action {
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
                if let Some(suggestion) = self.search_suggestions().get(index) {
                    self.search_query = suggestion.clone();
                    self.search_editing = false;
                }
            }
            Action::Pair => self.set_screen(Screen::Pair),
            Action::PairSimulate => {
                self.paired = true;
                self.show_toast("Device paired with Living room");
                self.set_screen(Screen::Home);
            }
            Action::PairRefresh => {
                self.pairing_status = PairingStatus::Connecting;
                self.pairing_code.clear();
                self.pairing_complete_uri.clear();
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
        action
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
        if self.pairing_seconds_left > 0 {
            self.pairing_seconds_left -= 1;
        }
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
