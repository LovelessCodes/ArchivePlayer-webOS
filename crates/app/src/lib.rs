//! Native ArchivePlayer client for LG webOS.
//!
//! This crate owns the startup state machine, catalogue and live worker, screen state, and
//! software-rendered 10-foot UI. The host simulator drives it with SDL2; the
//! webOS shell can drive the same state with its ARMv7 SDL2 build.

pub mod catalog;
pub mod layout;
pub mod live;
pub mod pairing;
pub mod render;
pub mod state;

pub use live::{LiveClient, LiveCommand, LiveEvent, PairingView};
pub use pairing::{PairingFlow, PairingPoll};
pub use state::{Action, App, FocusTarget};

use archiveplayer_tv_api::{ServerTarget, StoredSession};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupState {
    /// Reading the saved session and probing the server.
    Loading,
    /// No server configured yet.
    Setup,
    /// A server was selected but there is no valid token.
    Pairing { target: Option<ServerTarget> },
    /// A stored bearer token is available.
    Ready(StoredSession),
}

impl StartupState {
    /// The Android TV client's rule, kept identical here: saved token means
    /// Ready; a server without one means Pairing; nothing means Setup.
    pub fn from_session(target: Option<ServerTarget>, session: Option<StoredSession>) -> Self {
        match (target, session) {
            (_, Some(session)) => Self::Ready(session),
            (Some(target), None) => Self::Pairing {
                target: Some(target),
            },
            (None, None) => Self::Setup,
        }
    }

    pub fn needs_pairing(&self) -> bool {
        matches!(self, Self::Pairing { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// A 401 from any authenticated endpoint returns the app to pairing
    /// without discarding the remembered server.
    pub fn on_unauthorized(&self) -> Self {
        match self {
            Self::Ready(session) => Self::Pairing {
                target: Some(session.target.clone()),
            },
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archiveplayer_tv_protocol::User;

    fn target() -> ServerTarget {
        ServerTarget::new("id", "Living room", "http://192.168.1.20:8080").unwrap()
    }

    fn session() -> StoredSession {
        StoredSession {
            target: target(),
            token: "0123456789abcdef".into(),
            user: User {
                id: 1,
                email: "owner@example.com".into(),
                display_name: None,
                created_at: None,
            },
            device_name: "webOS TV".into(),
        }
    }

    #[test]
    fn startup_follows_server_and_session() {
        assert_eq!(StartupState::from_session(None, None), StartupState::Setup);
        assert!(StartupState::from_session(Some(target()), None).needs_pairing());
        assert!(StartupState::from_session(Some(target()), Some(session())).is_ready());
    }

    #[test]
    fn unauthorized_returns_to_pairing_and_keeps_target() {
        let state = StartupState::Ready(session()).on_unauthorized();
        assert!(matches!(
            state,
            StartupState::Pairing {
                target: Some(ref target)
            } if target.id == "id"
        ));
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use archiveplayer_tv_ui::Screen;

    #[test]
    fn home_targets_cover_hero_and_rails() {
        let app = App::demo();
        let targets = app.targets();
        assert!(targets.len() >= 9);
        assert!(targets
            .iter()
            .any(|target| target.action == Action::Play(0)));
        assert!(targets
            .iter()
            .any(|target| target.action == Action::Go(Screen::Library)));
    }

    #[test]
    fn detail_targets_expose_episodes_for_series() {
        let mut app = App::demo();
        app.selected = 5;
        app.set_screen(Screen::Detail);
        let targets = app.targets();
        assert!(targets.iter().any(|target| target.action == Action::Back));
        assert!(
            targets
                .iter()
                .filter(|target| target.action == Action::Play(5))
                .count()
                >= 2
        );
    }

    #[test]
    fn category_chips_filter_the_catalogue() {
        let mut app = App::demo();
        app.category = 3; // Horror
        let horror = app.visible_items();
        assert!(!horror.is_empty());
        assert!(horror.iter().all(|index| app
            .library
            .item(*index)
            .genres
            .iter()
            .any(|genre| genre == "Horror")));
        app.category = 1; // Movies
        assert!(app
            .visible_items()
            .iter()
            .all(|index| !app.library.item(*index).is_series()));
    }

    #[test]
    fn search_filters_the_catalogue() {
        let mut app = App::demo();
        app.search_query = "nosferatu".into();
        assert_eq!(app.search_results(), vec![2]);
        app.search_query = "noir".into();
        assert!(app.search_results().contains(&10));
    }
    #[test]
    fn spatial_focus_moves_from_hero_to_buttons() {
        let mut app = App::demo();
        app.focus_index = 4;
        app.move_focus(archiveplayer_tv_ui::FocusMove::Right);
        assert_eq!(app.focus_index, 5);
        app.activate();
        assert_eq!(app.screen, Screen::Detail);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn library_activation_opens_the_selected_title() {
        let mut app = App::demo();
        app.screen = Screen::Library;
        app.focus_index = 10;
        app.activate();
        assert_eq!(app.screen, Screen::Detail);
        assert_eq!(app.selected, 0);
        app.back();
        assert_eq!(app.screen, Screen::Library);
    }

    #[test]
    fn search_text_edits_and_backspace_removes() {
        let mut app = App::demo();
        app.screen = Screen::Search;
        app.search_editing = true;
        app.on_text("nosf");
        assert_eq!(app.search_query, "nosf");
        app.on_backspace();
        assert_eq!(app.search_query, "nos");
        app.search_editing = false;
        app.back();
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn player_tick_advances_and_stops_at_the_end() {
        let mut app = App::demo();
        app.selected = 0;
        app.screen = Screen::Player;
        app.player_position = 5758.0;
        app.player_duration = 5760.0;
        app.player_playing = true;
        app.tick(5.0);
        assert_eq!(app.player_position, 5760.0);
        assert!(!app.player_playing);
    }
}
