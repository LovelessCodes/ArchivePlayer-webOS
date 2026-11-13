//! Native ArchivePlayer client for LG webOS.
//!
//! This crate owns the startup state machine, catalogue and live worker, screen state, and
//! software-rendered 10-foot UI. The host simulator drives it with SDL2; the
//! webOS shell can drive the same state with its ARMv7 SDL2 build.

pub mod catalog;
pub mod layout;
pub mod live;
pub mod pairing;
pub mod poster;
pub mod progress;
pub mod render;
pub mod state;
pub mod video;

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
    fn search_focus_reaches_the_box_and_results() {
        let mut app = App::demo();
        app.set_screen(Screen::Search);
        assert_eq!(app.focus_index, 4);
        app.focus_index = 2;
        app.move_focus(archiveplayer_tv_ui::FocusMove::Right);
        assert_eq!(app.focus_index, 4);
        app.search_query = "noir".into();
        app.move_focus(archiveplayer_tv_ui::FocusMove::Down);
        assert_eq!(app.focus_index, 9);
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

    fn archive_response(identifiers: &[&str]) -> archiveplayer_tv_protocol::ArchiveSearchResponse {
        archiveplayer_tv_protocol::ArchiveSearchResponse {
            results: identifiers
                .iter()
                .map(
                    |identifier| archiveplayer_tv_protocol::ArchiveSearchResult {
                        identifier: (*identifier).into(),
                        title: format!("Title {identifier}"),
                        media_type: "movies".into(),
                        thumbnail_url: format!("https://archive.org/services/img/{identifier}"),
                    },
                )
                .collect(),
            has_more_results: true,
            current_page: 1,
            total_results: 42,
        }
    }

    #[test]
    fn live_search_submits_and_builds_addable_cards() {
        let mut app = App::live("ArchivePlayer", "http://127.0.0.1:8080");
        app.screen = Screen::Search;
        app.search_query = " night ".into();
        assert!(app.start_search());
        assert_eq!(app.archive_query, "night");
        assert_eq!(app.search_status, state::SearchStatus::Loading);

        app.apply_archive_results("night", archive_response(&["night_a", "night_b"]));
        assert_eq!(app.archive_results.len(), 2);
        assert_eq!(app.archive_total, 42);
        assert!(app.archive_has_more);
        assert_eq!(app.search_status, state::SearchStatus::Ready);
        assert!(app
            .targets()
            .iter()
            .any(|target| target.action == Action::AddResult(0)));
    }

    #[test]
    fn stale_search_results_are_ignored() {
        let mut app = App::live("ArchivePlayer", "http://127.0.0.1:8080");
        app.screen = Screen::Search;
        app.search_query = "night".into();
        app.start_search();
        app.apply_archive_results("other", archive_response(&["other"]));
        assert!(app.archive_results.is_empty());
        assert_eq!(app.search_status, state::SearchStatus::Loading);
        app.apply_archive_results("night", archive_response(&["night"]));
        assert_eq!(app.archive_results.len(), 1);
    }

    #[test]
    fn archive_results_add_or_open_existing_titles() {
        let mut app = App::live("ArchivePlayer", "http://127.0.0.1:8080");
        app.set_catalog(catalog::Catalog::demo());
        app.screen = Screen::Search;
        app.search_query = "night".into();
        app.start_search();
        app.apply_archive_results("night", archive_response(&["new_title"]));
        app.focus_index = 9;
        assert_eq!(app.activate(), Action::AddResult(0));
        assert_eq!(app.search_add.as_deref(), Some("new_title"));

        app.search_add = None;
        app.start_search();
        app.apply_archive_results("night", archive_response(&["nosferatu"]));
        app.focus_index = 9;
        assert!(matches!(app.activate(), Action::Open(_)));
        assert_eq!(app.screen, Screen::Detail);
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
    fn player_uses_the_plan_length_instead_of_the_saved_duration() {
        let mut app = App::demo();
        app.selected = 0;
        app.library.items[0].playback_plan = Some(archiveplayer_tv_protocol::PlaybackPlan {
            platform: "webos".into(),
            file_name: "movie.mp4".into(),
            url: "https://example.test/movie.mp4".into(),
            mime_type: None,
            format: None,
            container: None,
            video_codec: None,
            size_bytes: None,
            length_seconds: Some(1_234.0),
            width: None,
            height: None,
            compatible: true,
            reason: None,
        });
        app.set_screen(Screen::Player);
        assert_eq!(app.player_duration, 1_234.0);
        assert!(app.player_position <= app.player_duration);
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
