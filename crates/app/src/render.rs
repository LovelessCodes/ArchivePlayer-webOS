//! Software-rendered screens for the host simulator.
//!
//! The same screen functions are intended to be driven by the GLES2 backend on
//! webOS. Keep drawing coordinates in the 1920x1080 logical space.

use archiveplayer_tv_gfx::{Canvas, Color, FontWeight, Rect, TextStyle};
use archiveplayer_tv_ui::theme::Theme;
use archiveplayer_tv_ui::Screen;

use crate::catalog::{EpisodeView, MediaItem};
use crate::layout;
use crate::state::{Action, App, PairingStatus, NAV_SCREENS};

const WHITE: Color = Color::WHITE;
const PANEL_SHADOW: Color = Color::rgba(0, 0, 0, 110);

pub fn render(app: &App, canvas: &mut Canvas) {
    canvas.clear(Theme::BG);
    canvas.fill_vertical_gradient(
        Rect::new(0, 0, Theme::WIDTH, Theme::HEIGHT),
        Theme::BG_SOFT,
        Theme::BG,
    );

    match app.screen {
        Screen::Home => {
            draw_sidebar(app, canvas);
            draw_topbar(
                app,
                "Good evening",
                "Your library, ready for the big screen",
                canvas,
            );
            draw_home(app, canvas);
        }
        Screen::Library => {
            draw_sidebar(app, canvas);
            let subtitle = format!(
                "{} titles  ·  {} shelves  ·  sorted by recently added",
                app.library.items.len(),
                app.library.categories.len()
            );
            draw_topbar(app, "Library", &subtitle, canvas);
            draw_library(app, canvas);
        }
        Screen::Search => {
            draw_sidebar(app, canvas);
            draw_topbar(app, "Search", "Find something to watch", canvas);
            draw_search(app, canvas);
        }
        Screen::Settings => {
            draw_sidebar(app, canvas);
            draw_topbar(app, "Settings", "Server, playback, and this device", canvas);
            draw_settings(app, canvas);
        }
        Screen::Detail => draw_detail(app, canvas),
        Screen::Pair => draw_pair(app, canvas),
        Screen::Player => draw_player(app, canvas),
    }

    draw_focus(app, canvas);
    draw_toast(app, canvas);
}

fn draw_sidebar(app: &App, canvas: &mut Canvas) {
    canvas.fill_rect(Rect::new(0, 0, Theme::SIDEBAR, Theme::HEIGHT), Theme::RAIL);
    canvas.fill_rect(
        Rect::new(Theme::SIDEBAR - 1, 0, 1, Theme::HEIGHT),
        Theme::BORDER_SOFT,
    );

    let mark = layout::logo_mark();
    canvas.fill_round_rect(mark, 13.0, Theme::PRIMARY);
    canvas.fill_triangle(
        (mark.x + 17, mark.y + 12),
        (mark.x + 17, mark.y + 34),
        (mark.x + 33, mark.y + 23),
        WHITE,
    );
    text(
        canvas,
        "ArchivePlayer",
        84,
        58,
        20.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        "NATIVE TV",
        84,
        74,
        9.0,
        Theme::DIM,
        FontWeight::SemiBold,
    );

    let nav = layout::nav_items();
    let focused_rect = app.targets().get(app.focus_index).map(|target| target.rect);
    for (index, (screen, label)) in NAV_SCREENS
        .iter()
        .zip(["Home", "Library", "Search", "Settings"])
        .enumerate()
    {
        let item = nav[index];
        let selected = app.screen == *screen;
        let focused = focused_rect == Some(item);
        if selected {
            canvas.fill_round_rect(item, 12.0, Theme::PRIMARY.with_alpha(28));
            canvas.fill_round_rect(Rect::new(item.x, item.y + 12, 4, 32), 2.0, Theme::PRIMARY);
        } else if focused {
            canvas.fill_round_rect(item, 12.0, Theme::PANEL_RAISED);
        }
        let color = if selected {
            Theme::TEXT
        } else if focused {
            Theme::TEXT_SOFT
        } else {
            Theme::MUTED
        };
        draw_icon(
            canvas,
            label,
            Rect::new(item.x + 18, item.y + 17, 22, 22),
            if selected {
                Theme::PRIMARY_LIGHT
            } else {
                color
            },
        );
        text(
            canvas,
            label,
            item.x + 58,
            item.y + 36,
            17.0,
            color,
            FontWeight::Medium,
        );
    }

    let pill = layout::server_pill();
    canvas.fill_round_rect(pill, 14.0, Theme::PANEL);
    canvas.stroke_round_rect(pill, 14.0, 1.0, Theme::BORDER_SOFT);
    canvas.fill_circle((pill.x + 22, pill.y + 24), 5.0, Theme::SUCCESS);
    let server_name = canvas.clipped(
        &app.server_name,
        112.0,
        TextStyle::new(14.0, Theme::TEXT).medium(),
    );
    text(
        canvas,
        &server_name,
        pill.x + 38,
        pill.y + 30,
        14.0,
        Theme::TEXT,
        FontWeight::Medium,
    );
    let host = app
        .server_url
        .split("://")
        .nth(1)
        .unwrap_or(&app.server_url)
        .trim_end_matches('/');
    let host = canvas.clipped(
        &format!(
            "{} · {}",
            if app.server_lan { "LAN" } else { "Remote" },
            host
        ),
        138.0,
        TextStyle::new(11.0, Theme::MUTED),
    );
    text(
        canvas,
        &host,
        pill.x + 38,
        pill.y + 50,
        11.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
    let version = if app.server_version.is_empty() {
        "API".to_string()
    } else {
        format!("v{}", app.server_version)
    };
    text(
        canvas,
        &version,
        pill.x + 150,
        pill.y + 30,
        10.0,
        Theme::DIM,
        FontWeight::Regular,
    );
}

fn draw_topbar(app: &App, title: &str, subtitle: &str, canvas: &mut Canvas) {
    let title_rect = layout::topbar_title();
    text(
        canvas,
        title,
        title_rect.x,
        title_rect.y + 36,
        36.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        subtitle,
        Theme::CONTENT_X,
        94,
        15.0,
        Theme::MUTED,
        FontWeight::Regular,
    );

    let clock = layout::topbar_clock();
    text_right(
        canvas,
        &app.clock,
        clock.right(),
        clock.y + 28,
        26.0,
        Theme::TEXT,
        FontWeight::Medium,
    );
    text_right(
        canvas,
        &app.date,
        clock.right(),
        clock.y + 50,
        13.0,
        Theme::MUTED,
        FontWeight::Regular,
    );

    let status = layout::topbar_status();
    canvas.fill_round_rect(status, 20.0, Theme::PANEL);
    canvas.stroke_round_rect(status, 20.0, 1.0, Theme::BORDER_SOFT);
    canvas.fill_circle((status.x + 22, status.y + 20), 4.0, Theme::SUCCESS);
    let name = canvas.clipped(
        &app.server_name,
        108.0,
        TextStyle::new(13.0, Theme::TEXT_SOFT).medium(),
    );
    text(
        canvas,
        &name,
        status.x + 36,
        status.y + 25,
        13.0,
        Theme::TEXT_SOFT,
        FontWeight::Medium,
    );
    text(
        canvas,
        if app.server_lan {
            "·  LAN"
        } else {
            "·  Remote"
        },
        status.x + 150,
        status.y + 25,
        12.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
}

fn draw_empty_library(app: &App, canvas: &mut Canvas, title: &str) {
    let card = Rect::new(Theme::CONTENT_X, 180, Theme::CONTENT_W, 420);
    canvas.fill_round_rect(card, 24.0, Theme::PANEL);
    canvas.stroke_round_rect(card, 24.0, 1.0, Theme::BORDER_SOFT);
    draw_icon(
        canvas,
        "library",
        Rect::new(card.center().0 - 34, card.y + 104, 68, 68),
        Theme::PRIMARY_LIGHT,
    );
    text_centered(
        canvas,
        title,
        card.center().0,
        card.y + 230,
        38.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    text_centered(
        canvas,
        "Add films or series in the ArchivePlayer web app, then refresh this screen.",
        card.center().0,
        card.y + 278,
        17.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
    text_centered(
        canvas,
        &format!("Server: {}", app.server_name),
        card.center().0,
        card.y + 326,
        14.0,
        Theme::DIM,
        FontWeight::Regular,
    );
}

fn draw_home(app: &App, canvas: &mut Canvas) {
    if app.library.items.is_empty() {
        draw_empty_library(app, canvas, "Your library is empty");
        return;
    }
    let continue_items = app.library.continue_watching();
    let recent_items = app.library.recently_added();
    let home = layout::home(continue_items.len(), recent_items.len());
    let featured = app
        .library
        .featured()
        .expect("the empty catalogue returned above");

    draw_hero(canvas, home.hero, home.hero_poster, featured, app.live);
    draw_button(
        canvas,
        home.play,
        "Play",
        if featured.progress.is_some() {
            "Resume"
        } else {
            "Play"
        },
        true,
        app.is_focused(home.play),
    );
    draw_button(
        canvas,
        home.details,
        "Details",
        "",
        false,
        app.is_focused(home.details),
    );

    if !continue_items.is_empty() {
        section_header(
            canvas,
            "Continue watching",
            "View all",
            home.section_continue,
        );
        for (rect, index) in home.continue_cards.iter().zip(&continue_items) {
            let item = app.library.item(*index);
            draw_landscape(
                canvas,
                *rect,
                item,
                item.progress_fraction(),
                true,
                app.is_focused(*rect),
            );
        }
    }

    section_header(
        canvas,
        "Recently added",
        "Browse library",
        home.section_recent,
    );
    for (rect, index) in home.recent_cards.iter().zip(&recent_items) {
        let item = app.library.item(*index);
        draw_landscape(canvas, *rect, item, None, false, app.is_focused(*rect));
    }
}

fn draw_library(app: &App, canvas: &mut Canvas) {
    let items = app.visible_items();
    let library = layout::library(app.library.categories.len(), items.len());

    for (index, rect) in library.chips.iter().enumerate() {
        let selected = app.category == index;
        let focused = app.is_focused(*rect);
        let fill = if selected {
            Theme::PRIMARY.with_alpha(32)
        } else if focused {
            Theme::PANEL_RAISED
        } else {
            Theme::PANEL
        };
        canvas.fill_round_rect(*rect, 12.0, fill);
        canvas.stroke_round_rect(
            *rect,
            12.0,
            1.0,
            if selected {
                Theme::PRIMARY.with_alpha(150)
            } else {
                Theme::BORDER_SOFT
            },
        );
        let label = &app.library.categories[index];
        let color = if selected {
            Theme::PRIMARY_LIGHT
        } else if focused {
            Theme::TEXT
        } else {
            Theme::MUTED
        };
        text_centered(
            canvas,
            label,
            rect.center().0,
            rect.y + 29,
            14.0,
            color,
            FontWeight::Medium,
        );
    }

    if items.is_empty() {
        let empty = Rect::new(Theme::CONTENT_X, 300, Theme::CONTENT_W, 420);
        canvas.fill_round_rect(empty, 20.0, Theme::PANEL.with_alpha(140));
        canvas.stroke_round_rect(empty, 20.0, 1.0, Theme::BORDER_SOFT);
        draw_icon(
            canvas,
            "library",
            Rect::new(empty.center().0 - 18, empty.y + 130, 36, 36),
            Theme::DIM,
        );
        text_centered(
            canvas,
            "Nothing on this shelf yet",
            empty.center().0,
            empty.y + 210,
            26.0,
            Theme::TEXT_SOFT,
            FontWeight::Medium,
        );
        text_centered(
            canvas,
            "Try another category, or add a title from the web app.",
            empty.center().0,
            empty.y + 244,
            15.0,
            Theme::MUTED,
            FontWeight::Regular,
        );
        return;
    }

    for (rect, index) in library.cards.iter().zip(items) {
        let item = app.library.item(index);
        draw_library_card(canvas, *rect, item, app.is_focused(*rect));
    }
}

fn draw_library_card(canvas: &mut Canvas, rect: Rect, item: &MediaItem, focused: bool) {
    draw_poster(canvas, rect, item, false);
    let title = canvas.clipped(
        &item.title,
        rect.width as f32,
        TextStyle::new(18.0, Theme::TEXT).semibold(),
    );
    text(
        canvas,
        &title,
        rect.x,
        rect.bottom() + 25,
        18.0,
        if focused {
            Theme::TEXT
        } else {
            Theme::TEXT_SOFT
        },
        FontWeight::SemiBold,
    );
    text(
        canvas,
        &item.card_meta(),
        rect.x,
        rect.bottom() + 47,
        13.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
    if let Some(fraction) = item.progress_fraction() {
        let track = Rect::new(rect.x, rect.bottom() + 58, rect.width, 3);
        canvas.fill_round_rect(track, 2.0, Color::rgba(255, 255, 255, 34));
        canvas.fill_round_rect(
            Rect::new(track.x, track.y, (track.width as f32 * fraction) as i32, 3),
            2.0,
            Theme::PRIMARY,
        );
    }
}

fn draw_search(app: &App, canvas: &mut Canvas) {
    let results = app.search_results();
    let suggestions = app.search_suggestions();
    let result_count = if app.live {
        app.archive_results.len()
    } else {
        results.len()
    };
    let search = if app.live {
        layout::search_remote(suggestions.len(), result_count)
    } else {
        layout::search(suggestions.len(), result_count)
    };

    let focused = app.is_focused(search.input);
    let editing = app.search_editing;
    canvas.fill_round_rect(
        search.input,
        14.0,
        if editing {
            Theme::PANEL_RAISED
        } else {
            Theme::PANEL
        },
    );
    canvas.stroke_round_rect(
        search.input,
        14.0,
        if editing { 2.0 } else { 1.0 },
        if editing {
            Theme::PRIMARY
        } else {
            Theme::BORDER
        },
    );
    draw_icon(
        canvas,
        "search",
        Rect::new(search.input.x + 24, search.input.y + 22, 22, 22),
        if editing {
            Theme::PRIMARY_LIGHT
        } else {
            Theme::MUTED
        },
    );
    let query = if app.search_query.is_empty() {
        if app.live {
            "Search Archive.org"
        } else {
            "Search your library"
        }
    } else {
        &app.search_query
    };
    text(
        canvas,
        query,
        search.input.x + 60,
        search.input.y + 40,
        18.0,
        if app.search_query.is_empty() {
            Theme::DIM
        } else {
            Theme::TEXT
        },
        FontWeight::Regular,
    );
    if editing && app.frame % 60 < 34 {
        let width = canvas.measure_text(&app.search_query, TextStyle::new(18.0, Theme::TEXT));
        canvas.fill_rect(
            Rect::new(
                search.input.x + 60 + width as i32 + 2,
                search.input.y + 17,
                2,
                30,
            ),
            Theme::PRIMARY_LIGHT,
        );
    }
    if app.search_query.is_empty() {
        text_right(
            canvas,
            "Type with a keyboard",
            search.input.right() - 24,
            search.input.y + 40,
            12.0,
            Theme::DIM,
            FontWeight::Regular,
        );
    } else if editing {
        text_right(
            canvas,
            "ENTER to search",
            search.input.right() - 24,
            search.input.y + 40,
            11.0,
            Theme::MUTED,
            FontWeight::Medium,
        );
    } else if focused {
        text_right(
            canvas,
            "OK to edit",
            search.input.right() - 24,
            search.input.y + 40,
            11.0,
            Theme::MUTED,
            FontWeight::Medium,
        );
    }

    for (index, rect) in search.chips.iter().enumerate() {
        let focused = app.is_focused(*rect);
        let suggestion = &suggestions[index];
        let selected = app.search_query.eq_ignore_ascii_case(suggestion);
        let fill = if selected {
            Theme::PRIMARY.with_alpha(30)
        } else if focused {
            Theme::PANEL_RAISED
        } else {
            Theme::PANEL
        };
        canvas.fill_round_rect(*rect, 18.0, fill);
        canvas.stroke_round_rect(
            *rect,
            18.0,
            1.0,
            if selected {
                Theme::PRIMARY.with_alpha(140)
            } else if focused {
                Theme::BORDER
            } else {
                Theme::BORDER_SOFT
            },
        );
        text_centered(
            canvas,
            suggestion,
            rect.center().0,
            rect.y + 27,
            13.0,
            if selected {
                Theme::PRIMARY_LIGHT
            } else if focused {
                Theme::TEXT
            } else {
                Theme::MUTED
            },
            FontWeight::Medium,
        );
    }

    let label = if app.live {
        match app.search_status {
            crate::state::SearchStatus::Loading => "Searching Archive.org",
            crate::state::SearchStatus::Failed(_) => "Search failed",
            _ if app.archive_query.is_empty() => "Find something on Archive.org",
            _ => "Archive.org results",
        }
    } else if app.search_query.is_empty() {
        "Popular right now"
    } else {
        "Results"
    };
    text(
        canvas,
        label,
        Theme::CONTENT_X,
        292,
        13.0,
        Theme::MUTED,
        FontWeight::SemiBold,
    );
    let count = if app.live {
        if app.archive_total > 0 {
            if app.archive_total > app.archive_results.len() as i64 {
                format!(
                    "{} found  ·  showing {}",
                    app.archive_total,
                    app.archive_results.len()
                )
            } else {
                format!("{} found", app.archive_total)
            }
        } else if !app.archive_results.is_empty() {
            format!("{} shown", app.archive_results.len())
        } else {
            String::new()
        }
    } else {
        format!(
            "{} {}",
            results.len(),
            if results.len() == 1 {
                "title"
            } else {
                "titles"
            }
        )
    };
    text_right(
        canvas,
        &count,
        Theme::CONTENT_X + Theme::CONTENT_W,
        292,
        12.0,
        Theme::DIM,
        FontWeight::Regular,
    );

    if app.live {
        draw_archive_results(app, canvas, &search);
        return;
    }

    if results.is_empty() {
        let empty = Rect::new(Theme::CONTENT_X, 330, Theme::CONTENT_W, 330);
        canvas.fill_round_rect(empty, 20.0, Theme::PANEL.with_alpha(120));
        text_centered(
            canvas,
            "No matches in your library",
            empty.center().0,
            empty.y + 140,
            26.0,
            Theme::TEXT_SOFT,
            FontWeight::Medium,
        );
        text_centered(
            canvas,
            "Try a title, a genre, or a broader phrase.",
            empty.center().0,
            empty.y + 174,
            15.0,
            Theme::MUTED,
            FontWeight::Regular,
        );
        return;
    }

    for (rect, index) in search.results.iter().zip(results) {
        let item = app.library.item(index);
        draw_library_card(canvas, *rect, item, app.is_focused(*rect));
    }
}

fn draw_archive_results(app: &App, canvas: &mut Canvas, search: &layout::SearchLayout) {
    use crate::state::SearchStatus;

    match &app.search_status {
        SearchStatus::Loading => {
            let panel = Rect::new(Theme::CONTENT_X, 330, Theme::CONTENT_W, 300);
            canvas.fill_round_rect(panel, 20.0, Theme::PANEL.with_alpha(120));
            text_centered(
                canvas,
                "Searching Archive.org…",
                panel.center().0,
                panel.y + 138,
                26.0,
                Theme::TEXT_SOFT,
                FontWeight::Medium,
            );
            text_centered(
                canvas,
                "The server is querying public-domain film, television, and animation.",
                panel.center().0,
                panel.y + 174,
                15.0,
                Theme::MUTED,
                FontWeight::Regular,
            );
            return;
        }
        SearchStatus::Failed(message) => {
            let panel = Rect::new(Theme::CONTENT_X, 330, Theme::CONTENT_W, 330);
            canvas.fill_round_rect(panel, 20.0, Theme::PANEL.with_alpha(140));
            canvas.stroke_round_rect(panel, 20.0, 1.0, Theme::DANGER.with_alpha(90));
            text_centered(
                canvas,
                "Archive.org search failed",
                panel.center().0,
                panel.y + 126,
                26.0,
                Theme::TEXT_SOFT,
                FontWeight::Medium,
            );
            let message = canvas.clipped(message, 1180.0, TextStyle::new(15.0, Theme::MUTED));
            text_centered(
                canvas,
                &message,
                panel.center().0,
                panel.y + 162,
                15.0,
                Theme::MUTED,
                FontWeight::Regular,
            );
            text_centered(
                canvas,
                "Focus the search box and press ENTER to try again.",
                panel.center().0,
                panel.y + 210,
                14.0,
                Theme::DIM,
                FontWeight::Regular,
            );
            return;
        }
        SearchStatus::Idle if app.archive_query.is_empty() => {
            let panel = Rect::new(Theme::CONTENT_X, 330, Theme::CONTENT_W, 330);
            canvas.fill_round_rect(panel, 20.0, Theme::PANEL.with_alpha(120));
            draw_icon(
                canvas,
                "search",
                Rect::new(panel.center().0 - 22, panel.y + 96, 44, 44),
                Theme::DIM,
            );
            text_centered(
                canvas,
                "Search the Internet Archive",
                panel.center().0,
                panel.y + 184,
                26.0,
                Theme::TEXT_SOFT,
                FontWeight::Medium,
            );
            text_centered(
                canvas,
                "Type a title, then press ENTER. Pick a result to add it to your library.",
                panel.center().0,
                panel.y + 220,
                15.0,
                Theme::MUTED,
                FontWeight::Regular,
            );
            return;
        }
        _ => {}
    }

    if app.archive_results.is_empty() {
        let panel = Rect::new(Theme::CONTENT_X, 330, Theme::CONTENT_W, 300);
        canvas.fill_round_rect(panel, 20.0, Theme::PANEL.with_alpha(120));
        text_centered(
            canvas,
            "No matches on Archive.org",
            panel.center().0,
            panel.y + 138,
            26.0,
            Theme::TEXT_SOFT,
            FontWeight::Medium,
        );
        text_centered(
            canvas,
            "Try a shorter title, an alternate spelling, or a different year.",
            panel.center().0,
            panel.y + 174,
            15.0,
            Theme::MUTED,
            FontWeight::Regular,
        );
        return;
    }

    for (index, rect) in search.results.iter().enumerate() {
        let Some(card) = app.archive_card(index) else {
            continue;
        };
        let in_library = app.is_in_library(&card.identifier);
        let adding = app.search_add.as_deref() == Some(card.identifier.as_str());
        draw_archive_result(
            canvas,
            *rect,
            card,
            app.is_focused(*rect),
            in_library,
            adding,
        );
    }
}

fn draw_archive_result(
    canvas: &mut Canvas,
    rect: Rect,
    card: &crate::catalog::SearchCard,
    focused: bool,
    in_library: bool,
    adding: bool,
) {
    canvas.fill_round_rect(
        rect,
        16.0,
        if focused {
            Theme::PANEL_RAISED
        } else {
            Theme::PANEL
        },
    );
    if focused {
        canvas.stroke_round_rect(rect, 16.0, 2.0, Theme::PRIMARY);
    } else {
        canvas.stroke_round_rect(rect, 16.0, 1.0, Theme::BORDER_SOFT);
    }

    let thumb = Rect::new(rect.x + 18, rect.y + 18, 200, 128);
    canvas.fill_round_vertical_gradient(thumb, 12.0, card.palette.0, card.palette.1);
    canvas.fill_circle(
        (thumb.right() - 34, thumb.y + 28),
        44.0,
        card.accent.with_alpha(26),
    );
    for offset in 0..5 {
        let x = thumb.x + 20 + offset * 38;
        canvas.line(
            x,
            thumb.y,
            x - 40,
            thumb.bottom(),
            1.0,
            WHITE.with_alpha(10),
        );
    }
    let initial = card.title.chars().next().unwrap_or('A').to_string();
    text(
        canvas,
        &initial,
        thumb.x + 18,
        thumb.y + 90,
        72.0,
        card.accent.with_alpha(54),
        FontWeight::Bold,
    );
    text_right(
        canvas,
        "ARCHIVE.ORG",
        thumb.right() - 12,
        thumb.bottom() - 12,
        9.0,
        WHITE.with_alpha(90),
        FontWeight::SemiBold,
    );

    let text_x = thumb.right() + 24;
    let text_width = (rect.right() - 190 - text_x).max(120) as f32;
    let title = canvas.clipped(
        &card.title,
        text_width,
        TextStyle::new(
            24.0,
            if focused {
                Theme::TEXT
            } else {
                Theme::TEXT_SOFT
            },
        )
        .semibold(),
    );
    text(
        canvas,
        &title,
        text_x,
        rect.y + 56,
        24.0,
        if focused {
            Theme::TEXT
        } else {
            Theme::TEXT_SOFT
        },
        FontWeight::SemiBold,
    );
    text(
        canvas,
        &card.kind,
        text_x,
        rect.y + 86,
        11.0,
        card.accent,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        if in_library {
            "Already in your library  ·  OK opens details"
        } else if adding {
            "Adding this title…"
        } else {
            "OK adds this title to your library"
        },
        text_x,
        rect.y + 116,
        13.0,
        if in_library {
            Theme::SUCCESS
        } else if adding {
            Theme::AMBER
        } else {
            Theme::MUTED
        },
        FontWeight::Regular,
    );

    let badge_text = if adding {
        "ADDING…"
    } else if in_library {
        "IN LIBRARY"
    } else {
        "ADD"
    };
    let badge = Rect::new(rect.right() - 156, rect.y + 62, 132, 42);
    canvas.fill_round_rect(
        badge,
        21.0,
        if in_library {
            Theme::SUCCESS.with_alpha(34)
        } else if adding {
            Theme::AMBER.with_alpha(30)
        } else {
            Theme::PRIMARY.with_alpha(24)
        },
    );
    canvas.stroke_round_rect(
        badge,
        21.0,
        1.0,
        if in_library {
            Theme::SUCCESS.with_alpha(130)
        } else if adding {
            Theme::AMBER.with_alpha(130)
        } else {
            Theme::PRIMARY.with_alpha(130)
        },
    );
    text_centered(
        canvas,
        badge_text,
        badge.center().0,
        badge.y + 27,
        12.0,
        if in_library {
            Theme::SUCCESS
        } else if adding {
            Theme::AMBER
        } else {
            Theme::PRIMARY_LIGHT
        },
        FontWeight::SemiBold,
    );
}

fn draw_detail(app: &App, canvas: &mut Canvas) {
    let item = app.selected_item();
    let detail = layout::detail(item.episodes.len());

    canvas.fill_round_vertical_gradient(detail.backdrop, 0.0, item.palette.0, item.palette.1);
    // Cinematic shapes.
    let circle_center = (detail.backdrop.right() - 240, detail.backdrop.y + 190);
    canvas.fill_circle(circle_center, 260.0, item.accent.with_alpha(26));
    canvas.fill_circle(
        (circle_center.0 + 60, circle_center.1 + 30),
        140.0,
        item.accent.with_alpha(18),
    );
    for offset in 0..7 {
        let x = detail.backdrop.right() - 520 + offset * 64;
        canvas.line(
            x,
            detail.backdrop.y,
            x - 180,
            detail.backdrop.bottom(),
            1.0,
            WHITE.with_alpha(10),
        );
    }
    canvas.fill_horizontal_gradient(
        Rect::new(
            detail.backdrop.x,
            detail.backdrop.y,
            1240,
            detail.backdrop.height,
        ),
        Theme::BG.with_alpha(246),
        Theme::BG.with_alpha(0),
    );
    canvas.fill_vertical_gradient(
        Rect::new(0, 560, Theme::WIDTH, 68),
        Theme::BG.with_alpha(0),
        Theme::BG,
    );

    // Back button.
    draw_text_button(
        canvas,
        detail.back,
        "Back",
        "back",
        app.is_focused(detail.back),
    );

    text(
        canvas,
        if app.live {
            "YOUR LIBRARY  ·  DETAILS"
        } else {
            "ARCHIVE PICTURES  ·  FEATURED"
        },
        324,
        166,
        12.0,
        item.accent,
        FontWeight::SemiBold,
    );
    let title_style = TextStyle::new(56.0, Theme::TEXT).semibold();
    let measured = canvas.measure_text(&item.title, title_style);
    let title_size = if measured > 940.0 { 42.0 } else { 56.0 };
    let title_lines = canvas.wrapped(
        &item.title,
        940.0,
        2,
        TextStyle::new(title_size, Theme::TEXT).semibold(),
    );
    let title_base = if title_lines.len() > 1 { 218 } else { 238 };
    for (index, line) in title_lines.iter().enumerate() {
        text(
            canvas,
            line,
            324,
            title_base + index as i32 * 48,
            title_size,
            Theme::TEXT,
            FontWeight::SemiBold,
        );
    }
    let meta_y = if title_lines.len() > 1 { 318 } else { 282 };
    text(
        canvas,
        &item.meta_label(),
        324,
        meta_y,
        16.0,
        Theme::TEXT_SOFT,
        FontWeight::Regular,
    );
    let mut next_y = meta_y + 40;
    if !item.tagline.is_empty() {
        text(
            canvas,
            &item.tagline,
            324,
            next_y,
            16.0,
            item.accent,
            FontWeight::Medium,
        );
        next_y += 48;
    }

    let lines = canvas.wrapped(
        &item.description,
        880.0,
        3,
        TextStyle::new(17.0, Theme::TEXT_SOFT),
    );
    for (index, line) in lines.iter().enumerate() {
        text(
            canvas,
            line,
            324,
            next_y + index as i32 * 27,
            17.0,
            Theme::TEXT_SOFT,
            FontWeight::Regular,
        );
    }

    draw_button(
        canvas,
        detail.play,
        if item.progress.is_some() {
            "Resume"
        } else {
            "Play"
        },
        "play",
        true,
        app.is_focused(detail.play),
    );
    if let Some(plan) = &item.playback_plan {
        if !plan.compatible {
            let reason = plan
                .reason
                .as_deref()
                .unwrap_or("This file needs a decoder this TV does not provide.");
            text(
                canvas,
                "NOT PLAYABLE ON THIS TV",
                324,
                586,
                12.0,
                Theme::DANGER,
                FontWeight::SemiBold,
            );
            let warning = canvas.clipped(reason, 900.0, TextStyle::new(14.0, Theme::TEXT_SOFT));
            text(
                canvas,
                &warning,
                324,
                610,
                14.0,
                Theme::TEXT_SOFT,
                FontWeight::Regular,
            );
        }
    }

    // Poster with a soft drop shadow.
    canvas.fill_round_rect(detail.poster.offset(0, 14), 18.0, PANEL_SHADOW);
    draw_poster(canvas, detail.poster, item, true);

    if item.is_series() {
        text(
            canvas,
            "Episodes",
            Theme::CONTENT_X,
            700,
            25.0,
            Theme::TEXT,
            FontWeight::SemiBold,
        );
        text_right(
            canvas,
            &format!("{} available", item.episodes.len()),
            Theme::CONTENT_X + Theme::CONTENT_W,
            700,
            12.0,
            Theme::MUTED,
            FontWeight::Regular,
        );
        for (rect, episode) in detail.episodes.iter().zip(&item.episodes) {
            draw_episode_card(canvas, *rect, item, episode, app.is_focused(*rect));
        }
    } else {
        text(
            canvas,
            "About this film",
            Theme::CONTENT_X,
            700,
            25.0,
            Theme::TEXT,
            FontWeight::SemiBold,
        );
        let info = if item.runtime_minutes > 0 && item.rating > 0.0 {
            vec![
                (
                    "RUNTIME",
                    format!("{} minutes", item.runtime_minutes),
                    "Uncut".to_string(),
                ),
                (
                    "RATING",
                    format!("★ {:.1} / 10", item.rating),
                    "Community score".to_string(),
                ),
                (
                    "SOURCE",
                    "Archive.org".to_string(),
                    "Public domain".to_string(),
                ),
            ]
        } else {
            let plan = item.playback_plan.as_ref();
            vec![
                (
                    "PLAYBACK",
                    plan.map(|plan| {
                        format!(
                            "{} · {}",
                            plan.video_codec
                                .as_deref()
                                .unwrap_or("Video")
                                .to_uppercase(),
                            plan.container.as_deref().unwrap_or("file").to_uppercase()
                        )
                    })
                    .unwrap_or_else(|| "Server selected".into()),
                    "Optimized for webOS".to_string(),
                ),
                (
                    "FILES",
                    format!("{} available", item.file_count.max(1)),
                    "Archive.org source".to_string(),
                ),
                (
                    "SOURCE",
                    "Archive.org".to_string(),
                    "Public domain".to_string(),
                ),
            ]
        };
        for (index, rect) in detail.info_cards.iter().enumerate() {
            let (label, value, note) = &info[index];
            canvas.fill_round_rect(*rect, 16.0, Theme::PANEL);
            canvas.stroke_round_rect(*rect, 16.0, 1.0, Theme::BORDER_SOFT);
            text(
                canvas,
                label,
                rect.x + 24,
                rect.y + 38,
                11.0,
                Theme::DIM,
                FontWeight::SemiBold,
            );
            text(
                canvas,
                value,
                rect.x + 24,
                rect.y + 86,
                24.0,
                Theme::TEXT,
                FontWeight::SemiBold,
            );
            text(
                canvas,
                note,
                rect.x + 24,
                rect.y + 118,
                13.0,
                Theme::MUTED,
                FontWeight::Regular,
            );
        }
    }
}

fn draw_episode_card(
    canvas: &mut Canvas,
    rect: Rect,
    item: &MediaItem,
    episode: &EpisodeView,
    focused: bool,
) {
    canvas.fill_round_rect(
        rect,
        15.0,
        if focused {
            Theme::PANEL_RAISED
        } else {
            Theme::PANEL
        },
    );
    canvas.stroke_round_rect(
        rect,
        15.0,
        1.0,
        if focused {
            Theme::BORDER
        } else {
            Theme::BORDER_SOFT
        },
    );
    let thumb = Rect::new(rect.x + 12, rect.y + 12, 126, 84);
    canvas.fill_round_vertical_gradient(thumb, 10.0, item.palette.0, item.palette.1);
    canvas.fill_circle(
        (thumb.right() - 28, thumb.y + 24),
        24.0,
        item.accent.with_alpha(55),
    );
    if let Some(progress) = episode.progress {
        let fraction = (progress as f32 / (episode.runtime_minutes as f32 * 60.0)).clamp(0.0, 1.0);
        canvas.fill_rect(
            Rect::new(thumb.x + 10, thumb.bottom() - 8, 106, 3),
            Color::rgba(255, 255, 255, 50),
        );
        canvas.fill_rect(
            Rect::new(
                thumb.x + 10,
                thumb.bottom() - 8,
                (106.0 * fraction) as i32,
                3,
            ),
            Theme::PRIMARY,
        );
    }
    draw_icon(
        canvas,
        "play",
        Rect::new(thumb.x + 50, thumb.y + 26, 24, 24),
        WHITE.with_alpha(220),
    );
    text(
        canvas,
        &if episode.runtime_minutes > 0 {
            format!("E{:02}  ·  {} min", episode.number, episode.runtime_minutes)
        } else {
            format!("E{:02}", episode.number)
        },
        rect.x + 154,
        rect.y + 34,
        11.0,
        Theme::DIM,
        FontWeight::SemiBold,
    );
    let title = canvas.clipped(
        &episode.title,
        (rect.width - 166) as f32,
        TextStyle::new(18.0, Theme::TEXT).semibold(),
    );
    text(
        canvas,
        &title,
        rect.x + 154,
        rect.y + 62,
        18.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    let description = canvas.clipped(
        &episode.description,
        (rect.width - 166) as f32,
        TextStyle::new(12.0, Theme::MUTED),
    );
    text(
        canvas,
        &description,
        rect.x + 154,
        rect.y + 86,
        12.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
    text(
        canvas,
        "Play episode",
        rect.x + 154,
        rect.y + 128,
        12.0,
        item.accent,
        FontWeight::Medium,
    );
}

fn draw_settings(app: &App, canvas: &mut Canvas) {
    let settings = layout::settings();
    let server_lines = vec![
        app.server_url.clone(),
        if app.server_version.is_empty() {
            "ArchivePlayer  ·  API version pending".into()
        } else {
            format!("ArchivePlayer {}  ·  API v1", app.server_version)
        },
    ];
    draw_settings_card(
        canvas,
        settings.cards[0],
        "SERVER",
        if app.paired { "Connected" } else { "Pairing" },
        &app.server_name,
        &server_lines,
        Some((
            if app.server_lan { "LAN" } else { "REMOTE" },
            if app.server_lan {
                Theme::SUCCESS
            } else {
                Theme::AMBER
            },
        )),
    );

    let selected = app.library.item(app.selected);
    let plan = selected.playback_plan.as_ref();
    let playback_title = plan
        .map(|plan| {
            format!(
                "{} · {}",
                plan.video_codec
                    .as_deref()
                    .unwrap_or("Video")
                    .to_uppercase(),
                plan.container.as_deref().unwrap_or("file").to_uppercase()
            )
        })
        .unwrap_or_else(|| {
            if app.live {
                "Server selected".into()
            } else {
                "H.264 · MP4".into()
            }
        });
    let playback_lines = vec![
        plan.and_then(|plan| match (plan.width, plan.height) {
            (Some(width), Some(height)) => Some(format!("{width}×{height}  ·  no transcoding")),
            _ => None,
        })
        .unwrap_or_else(|| {
            if app.live {
                "Profile selected by the server".into()
            } else {
                "Up to 1080p  ·  no transcoding".into()
            }
        }),
        "Plays with the native webOS media pipeline".into(),
    ];
    draw_settings_card(
        canvas,
        settings.cards[1],
        "PLAYBACK",
        if plan.is_some() || !app.live {
            "Direct play"
        } else {
            "Waiting for a title"
        },
        &playback_title,
        &playback_lines,
        Some(("webOS 5+", Theme::AMBER)),
    );

    let device_lines = vec![
        format!(
            "Device: webOS TV  ·  {}",
            if app.server_lan { "LAN" } else { "remote" }
        ),
        "Token stored in private app data".into(),
    ];
    draw_settings_card(
        canvas,
        settings.cards[2],
        "THIS DEVICE",
        if app.paired { "Paired" } else { "Not paired" },
        "ArchivePlayer TV",
        &device_lines,
        Some((
            if app.paired { "Active" } else { "Pairing" },
            if app.paired {
                Theme::SUCCESS
            } else {
                Theme::AMBER
            },
        )),
    );

    let about_lines = vec![
        "Based on ArchivePlayer by Jeppe Kjeldal".to_string(),
        "Not affiliated with LG or the Internet Archive".to_string(),
    ];
    draw_settings_card(
        canvas,
        settings.cards[3],
        "ABOUT",
        "MIT licensed",
        "ArchivePlayer 0.1.0",
        &about_lines,
        Some(("v0.1.0", Theme::DIM)),
    );

    draw_button(
        canvas,
        settings.pair,
        if app.paired {
            "Re-pair this TV"
        } else {
            "Pair this TV"
        },
        "",
        false,
        app.is_focused(settings.pair),
    );
    draw_button(
        canvas,
        settings.reconnect,
        "Check connection",
        "",
        false,
        app.is_focused(settings.reconnect),
    );
}

fn draw_settings_card(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    badge: &str,
    title: &str,
    lines: &[String],
    pill: Option<(&str, Color)>,
) {
    canvas.fill_round_rect(rect, 18.0, Theme::PANEL);
    canvas.stroke_round_rect(rect, 18.0, 1.0, Theme::BORDER_SOFT);
    text(
        canvas,
        label,
        rect.x + 28,
        rect.y + 38,
        11.0,
        Theme::DIM,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        badge,
        rect.x + 28,
        rect.y + 66,
        12.0,
        Theme::SUCCESS,
        FontWeight::Medium,
    );
    text(
        canvas,
        title,
        rect.x + 28,
        rect.y + 116,
        28.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    for (index, line) in lines.iter().enumerate() {
        text(
            canvas,
            line,
            rect.x + 28,
            rect.y + 152 + index as i32 * 24,
            14.0,
            Theme::MUTED,
            FontWeight::Regular,
        );
    }
    if let Some((pill_label, color)) = pill {
        let width = (pill_label.len() as i32) * 8 + 30;
        let pill_rect = Rect::new(rect.right() - width - 26, rect.y + 26, width, 30);
        canvas.fill_round_rect(pill_rect, 15.0, color.with_alpha(26));
        text_centered(
            canvas,
            pill_label,
            pill_rect.center().0,
            pill_rect.y + 20,
            11.0,
            color,
            FontWeight::Medium,
        );
    }
}

fn draw_pair(app: &App, canvas: &mut Canvas) {
    let pair = layout::pair();
    canvas.fill_round_rect(Rect::new(0, 0, Theme::WIDTH, Theme::HEIGHT), 0.0, Theme::BG);
    canvas.fill_vertical_gradient(
        Rect::new(0, 0, Theme::WIDTH, Theme::HEIGHT),
        Color::rgb(15, 19, 27),
        Theme::BG,
    );
    canvas.fill_circle((1520, 180), 320.0, Theme::PRIMARY.with_alpha(18));
    canvas.fill_circle((420, 900), 280.0, Theme::PRIMARY.with_alpha(10));

    canvas.fill_round_rect(pair.card, 26.0, Theme::PANEL.with_alpha(246));
    canvas.stroke_round_rect(pair.card, 26.0, 1.0, Theme::BORDER);
    canvas.fill_round_rect(
        Rect::new(pair.card.x + 44, pair.card.y + 44, 46, 46),
        13.0,
        Theme::PRIMARY,
    );
    canvas.fill_triangle(
        (pair.card.x + 60, pair.card.y + 57),
        (pair.card.x + 60, pair.card.y + 78),
        (pair.card.x + 77, pair.card.y + 67),
        WHITE,
    );

    text(
        canvas,
        "PAIR THIS TV",
        pair.card.x + 44,
        pair.card.y + 132,
        12.0,
        Theme::PRIMARY_LIGHT,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        "Connect to your server",
        pair.card.x + 44,
        pair.card.y + 190,
        44.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    let lines = [
        "Open the activation page on your phone or computer",
        "and enter the code below. Your library stays on your",
        "network; this TV only receives a revocable token.",
    ];
    for (index, line) in lines.iter().enumerate() {
        text(
            canvas,
            line,
            pair.card.x + 44,
            pair.card.y + 240 + index as i32 * 24,
            15.0,
            Theme::TEXT_SOFT,
            FontWeight::Regular,
        );
    }

    canvas.fill_round_rect(pair.code, 16.0, Theme::BG_SOFT);
    canvas.stroke_round_rect(pair.code, 16.0, 1.0, Theme::BORDER);
    let code = if app.pairing_code.is_empty() {
        "····–····"
    } else {
        &app.pairing_code
    };
    text_centered(
        canvas,
        code,
        pair.code.center().0,
        pair.code.y + 54,
        36.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    let expires = if app.pairing_seconds_left > 0 {
        format!(
            "Code expires in {}:{:02}",
            app.pairing_seconds_left / 60,
            app.pairing_seconds_left % 60
        )
    } else {
        "Request a new code when this one expires".to_string()
    };
    text(
        canvas,
        &expires,
        pair.code.x,
        pair.code.bottom() + 26,
        12.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
    let (status, status_color) = match app.pairing_status {
        PairingStatus::Connecting => ("Contacting the server…", Theme::AMBER),
        PairingStatus::Waiting => ("Waiting for approval…", Theme::AMBER),
        PairingStatus::Approved => ("Approved — loading your library", Theme::SUCCESS),
        PairingStatus::Denied => ("Pairing declined — request a new code", Theme::DANGER),
        PairingStatus::Expired => ("Code expired — request a new code", Theme::DANGER),
        PairingStatus::Error => ("Could not reach the server", Theme::DANGER),
    };
    canvas.fill_circle(
        (pair.code.x + 7, pair.code.bottom() + 60),
        4.0,
        status_color,
    );
    text(
        canvas,
        status,
        pair.code.x + 22,
        pair.code.bottom() + 64,
        13.0,
        Theme::TEXT_SOFT,
        FontWeight::Medium,
    );

    // Real QR code for the activation URL.
    let qr_card = Rect::new(
        pair.qr.x - 24,
        pair.qr.y - 24,
        pair.qr.width + 48,
        pair.qr.height + 48,
    );
    canvas.fill_round_rect(qr_card, 18.0, WHITE);
    if !app.pairing_complete_uri.is_empty() {
        draw_qr(canvas, pair.qr, &app.pairing_complete_uri);
    }
    text_centered(
        canvas,
        "Scan with your phone",
        qr_card.center().0,
        qr_card.bottom() + 38,
        14.0,
        Theme::TEXT_SOFT,
        FontWeight::Medium,
    );
    let short_uri = app
        .pairing_uri
        .split("://")
        .nth(1)
        .unwrap_or(&app.pairing_uri);
    text_centered(
        canvas,
        if short_uri.is_empty() {
            "waiting for the server address"
        } else {
            short_uri
        },
        qr_card.center().0,
        qr_card.bottom() + 60,
        12.0,
        Theme::MUTED,
        FontWeight::Regular,
    );

    draw_button(
        canvas,
        pair.simulate,
        if app.live {
            "Get a new code"
        } else {
            "Simulate approval"
        },
        "",
        true,
        app.is_focused(pair.simulate),
    );
    draw_button(
        canvas,
        pair.back,
        "Back",
        "",
        false,
        app.is_focused(pair.back),
    );
    text(
        canvas,
        if app.live {
            "The TV stores a revocable bearer token; no password is entered on the TV."
        } else {
            "Demo mode: OK completes pairing and stores a local token."
        },
        pair.simulate.x,
        pair.simulate.bottom() + 34,
        11.0,
        Theme::DIM,
        FontWeight::Regular,
    );
}

fn draw_qr(canvas: &mut Canvas, rect: Rect, text_value: &str) {
    let Ok(code) = qrcodegen::QrCode::encode_text(text_value, qrcodegen::QrCodeEcc::Medium) else {
        return;
    };
    let modules = code.size();
    let module = (rect.width.min(rect.height) as f32 / modules as f32).floor() as i32;
    let size = module * modules;
    let origin_x = rect.center().0 - size / 2;
    let origin_y = rect.center().1 - size / 2;
    for y in 0..modules {
        for x in 0..modules {
            if code.get_module(x, y) {
                canvas.fill_rect(
                    Rect::new(origin_x + x * module, origin_y + y * module, module, module),
                    Color::rgb(11, 13, 18),
                );
            }
        }
    }
}

fn draw_player(app: &App, canvas: &mut Canvas) {
    let item = app.selected_item();
    canvas.fill_round_vertical_gradient(
        Rect::new(0, 0, Theme::WIDTH, Theme::HEIGHT),
        0.0,
        item.palette.0,
        item.palette.1,
    );
    canvas.fill_circle((1500, 280), 420.0, item.accent.with_alpha(22));
    canvas.fill_circle((520, 820), 300.0, item.accent.with_alpha(12));
    for offset in 0..10 {
        let x = 500 + offset * 130;
        canvas.line(x, 0, x - 260, Theme::HEIGHT, 1.0, WHITE.with_alpha(8));
    }
    // Darken for the overlay controls.
    canvas.fill_vertical_gradient(
        Rect::new(0, 0, Theme::WIDTH, 240),
        Color::rgba(0, 0, 0, 150),
        Color::rgba(0, 0, 0, 0),
    );
    canvas.fill_vertical_gradient(
        Rect::new(0, 760, Theme::WIDTH, 320),
        Color::rgba(0, 0, 0, 0),
        Color::rgba(0, 0, 0, 220),
    );

    draw_icon(canvas, "back", Rect::new(82, 76, 24, 24), WHITE);
    text(canvas, "Back", 116, 96, 15.0, WHITE, FontWeight::Medium);
    text(
        canvas,
        "NOW PLAYING",
        280,
        96,
        12.0,
        item.accent,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        &item.title,
        280,
        146,
        42.0,
        WHITE,
        FontWeight::SemiBold,
    );
    text(
        canvas,
        &item.meta_label(),
        280,
        180,
        14.0,
        WHITE.with_alpha(170),
        FontWeight::Regular,
    );

    text_right(
        canvas,
        "1080p  ·  H.264",
        Theme::WIDTH - 80,
        96,
        13.0,
        WHITE.with_alpha(180),
        FontWeight::Medium,
    );
    text_right(
        canvas,
        "DIRECT PLAY",
        Theme::WIDTH - 80,
        122,
        11.0,
        Theme::SUCCESS,
        FontWeight::SemiBold,
    );

    // A quiet typographic watermark stands in for video on the host renderer.
    let initial = item.title.chars().next().unwrap_or('A').to_string();
    text_centered(
        canvas,
        &initial,
        Theme::WIDTH / 2,
        660,
        420.0,
        item.accent.with_alpha(14),
        FontWeight::Bold,
    );
    if app.player_playing {
        text_centered(
            canvas,
            "PLAYING",
            Theme::WIDTH / 2,
            700,
            12.0,
            WHITE.with_alpha(130),
            FontWeight::SemiBold,
        );
    } else {
        draw_icon(
            canvas,
            "play",
            Rect::new(Theme::WIDTH / 2 - 20, 476, 40, 40),
            WHITE,
        );
    }

    // Progress and controls.
    let progress = layout::player().progress;
    canvas.fill_round_rect(progress, 4.0, WHITE.with_alpha(40));
    canvas.fill_round_rect(
        Rect::new(
            progress.x,
            progress.y,
            (progress.width as f32 * app.player_fraction()) as i32,
            progress.height,
        ),
        4.0,
        Theme::PRIMARY,
    );
    canvas.fill_circle(
        (
            progress.x + (progress.width as f32 * app.player_fraction()) as i32,
            progress.y + 4,
        ),
        7.0,
        Theme::PRIMARY_LIGHT,
    );
    text(
        canvas,
        &app.player_time_label(),
        progress.x,
        progress.y + 38,
        13.0,
        WHITE.with_alpha(180),
        FontWeight::Medium,
    );
    text_right(
        canvas,
        &app.player_duration_label(),
        progress.right(),
        progress.y + 38,
        13.0,
        WHITE.with_alpha(180),
        FontWeight::Medium,
    );

    draw_round_control(
        canvas,
        layout::player().rewind,
        "rewind",
        app.is_focused(layout::player().rewind),
    );
    draw_play_control(
        canvas,
        layout::player().play,
        app.player_playing,
        app.is_focused(layout::player().play),
    );
    draw_round_control(
        canvas,
        layout::player().forward,
        "forward",
        app.is_focused(layout::player().forward),
    );
}

fn draw_play_control(canvas: &mut Canvas, rect: Rect, playing: bool, focused: bool) {
    canvas.fill_circle(
        rect.center(),
        rect.width as f32 / 2.0,
        if focused {
            Theme::PRIMARY_LIGHT
        } else {
            Theme::PRIMARY
        },
    );
    if playing {
        canvas.fill_round_rect(
            Rect::new(rect.center().0 - 13, rect.center().1 - 18, 8, 36),
            3.0,
            WHITE,
        );
        canvas.fill_round_rect(
            Rect::new(rect.center().0 + 5, rect.center().1 - 18, 8, 36),
            3.0,
            WHITE,
        );
    } else {
        canvas.fill_triangle(
            (rect.center().0 - 12, rect.center().1 - 18),
            (rect.center().0 - 12, rect.center().1 + 18),
            (rect.center().0 + 18, rect.center().1),
            WHITE,
        );
    }
}

fn draw_round_control(canvas: &mut Canvas, rect: Rect, icon: &str, focused: bool) {
    canvas.fill_circle(
        rect.center(),
        rect.width as f32 / 2.0,
        if focused {
            Theme::PANEL_HOVER
        } else {
            Color::rgba(8, 10, 14, 120)
        },
    );
    canvas.stroke_round_rect(rect, rect.width as f32 / 2.0, 1.0, WHITE.with_alpha(40));
    draw_icon(canvas, icon, rect.inset(25), WHITE.with_alpha(220));
}

fn draw_focus(app: &App, canvas: &mut Canvas) {
    let targets = app.targets();
    let Some(target) = targets.get(app.focus_index) else {
        return;
    };
    if app.search_editing && app.screen == Screen::Search {
        return;
    }
    let radius = if app.screen == Screen::Player {
        if target.action == Action::TogglePlay {
            54.0
        } else {
            44.0
        }
    } else {
        16.0
    };
    focus_ring(canvas, target.rect, radius);
}

fn focus_ring(canvas: &mut Canvas, rect: Rect, radius: f32) {
    canvas.stroke_round_rect(
        rect.inset(-8),
        radius + 6.0,
        1.0,
        Theme::PRIMARY.with_alpha(80),
    );
    canvas.stroke_round_rect(rect.inset(-4), radius + 4.0, 3.0, Theme::PRIMARY_LIGHT);
}

fn draw_toast(app: &App, canvas: &mut Canvas) {
    let Some(message) = &app.toast else {
        return;
    };
    let width = canvas.measure_text(message, TextStyle::new(15.0, WHITE)) as i32 + 56;
    let rect = Rect::new((Theme::WIDTH - width) / 2, 996, width, 46);
    canvas.fill_round_rect(rect, 23.0, Theme::PANEL_RAISED.with_alpha(246));
    canvas.stroke_round_rect(rect, 23.0, 1.0, Theme::BORDER);
    canvas.fill_circle((rect.x + 24, rect.y + 23), 4.0, Theme::SUCCESS);
    text(
        canvas,
        message,
        rect.x + 38,
        rect.y + 29,
        15.0,
        Theme::TEXT,
        FontWeight::Medium,
    );
}

fn draw_hero(canvas: &mut Canvas, rect: Rect, poster: Rect, item: &MediaItem, live: bool) {
    canvas.fill_round_vertical_gradient(rect, 24.0, item.palette.0, item.palette.1);
    canvas.fill_circle(
        (rect.right() - 260, rect.y + 120),
        250.0,
        item.accent.with_alpha(36),
    );
    canvas.fill_circle(
        (rect.right() - 140, rect.y + 300),
        160.0,
        item.accent.with_alpha(22),
    );
    for offset in 0..9 {
        let x = rect.right() - 620 + offset * 80;
        canvas.line(x, rect.y, x - 220, rect.bottom(), 1.0, WHITE.with_alpha(8));
    }
    canvas.fill_round_horizontal_gradient(
        Rect::new(rect.x, rect.y, 1080, rect.height),
        24.0,
        Theme::BG.with_alpha(238),
        Theme::BG.with_alpha(0),
    );

    text(
        canvas,
        if live {
            "FEATURED  ·  FROM YOUR LIBRARY"
        } else {
            "FEATURED  ·  PUBLIC DOMAIN"
        },
        324,
        178,
        12.0,
        Theme::PRIMARY_LIGHT,
        FontWeight::SemiBold,
    );
    let title_style = TextStyle::new(46.0, WHITE).semibold();
    let title_width = canvas.measure_text(&item.title, title_style);
    let title_size = if title_width > 900.0 { 38.0 } else { 54.0 };
    let title_lines = canvas.wrapped(
        &item.title,
        900.0,
        2,
        TextStyle::new(title_size, WHITE).semibold(),
    );
    let title_base = if title_lines.len() > 1 { 222 } else { 246 };
    for (index, line) in title_lines.iter().enumerate() {
        text(
            canvas,
            line,
            324,
            title_base + index as i32 * 48,
            title_size,
            WHITE,
            FontWeight::SemiBold,
        );
    }
    let meta_y = if title_lines.len() > 1 { 322 } else { 292 };
    text(
        canvas,
        &item.meta_label(),
        324,
        meta_y,
        16.0,
        WHITE.with_alpha(185),
        FontWeight::Regular,
    );
    let description = canvas.wrapped(
        &item.description,
        880.0,
        2,
        TextStyle::new(17.0, WHITE.with_alpha(205)),
    );
    for (index, line) in description.iter().enumerate() {
        text(
            canvas,
            line,
            324,
            meta_y + 38 + index as i32 * 26,
            17.0,
            WHITE.with_alpha(205),
            FontWeight::Regular,
        );
    }

    canvas.fill_round_rect(poster.offset(0, 18), 22.0, PANEL_SHADOW);
    draw_poster(canvas, poster, item, true);
    let badge = Rect::new(poster.right() - 126, poster.y + 20, 104, 24);
    canvas.fill_round_rect(badge, 12.0, Color::rgba(0, 0, 0, 150));
    text_centered(
        canvas,
        "ARCHIVE.ORG",
        badge.center().0,
        badge.y + 16,
        9.0,
        WHITE.with_alpha(200),
        FontWeight::SemiBold,
    );
}

fn draw_poster(canvas: &mut Canvas, rect: Rect, item: &MediaItem, show_title: bool) {
    if let Some(poster) = &item.poster {
        canvas.draw_image_cover(rect, 16.0, poster);
    } else {
        canvas.fill_round_vertical_gradient(rect, 16.0, item.palette.0, item.palette.1);
        draw_art_shapes(canvas, rect, item);
        let initial = item.title.chars().next().unwrap_or('A');
        let initial_size = rect.width as f32 * 0.72;
        text(
            canvas,
            &initial.to_string(),
            rect.x + 20,
            (rect.y as f32 + rect.height as f32 * 0.57) as i32,
            initial_size,
            item.accent.with_alpha(34),
            FontWeight::Bold,
        );
    }

    canvas.fill_round_vertical_gradient(
        rect,
        16.0,
        Color::rgba(0, 0, 0, 0),
        Color::rgba(0, 0, 0, if show_title { 238 } else { 150 }),
    );
    canvas.stroke_round_rect(rect.inset(1), 15.0, 1.0, WHITE.with_alpha(18));
    if show_title {
        let style = TextStyle::new(20.0, WHITE).semibold();
        let lines = canvas.wrapped(&item.title, (rect.width - 32) as f32, 3, style);
        let base = rect.bottom() - 20 - (lines.len() as i32 - 1) * 26;
        for (index, line) in lines.iter().enumerate() {
            text(
                canvas,
                line,
                rect.x + 16,
                base + index as i32 * 26,
                20.0,
                WHITE,
                FontWeight::SemiBold,
            );
        }
        text(
            canvas,
            &item.card_meta(),
            rect.x + 16,
            rect.y + rect.height - 118,
            10.0,
            WHITE.with_alpha(150),
            FontWeight::SemiBold,
        );
    }
}

fn draw_art_shapes(canvas: &mut Canvas, rect: Rect, item: &MediaItem) {
    let center = (rect.x + rect.width * 3 / 4, rect.y + rect.height / 4);
    match item.id.rem_euclid(3) {
        0 => {
            canvas.fill_circle(center, rect.width as f32 * 0.28, item.accent.with_alpha(54));
            canvas.fill_circle(
                (center.0 - 20, center.1 + 18),
                rect.width as f32 * 0.16,
                item.accent.with_alpha(34),
            );
        }
        1 => {
            for offset in 0..3 {
                canvas.fill_round_rect(
                    Rect::new(rect.x + 24, rect.y + 48 + offset * 44, rect.width - 48, 22),
                    11.0,
                    item.accent.with_alpha(38 - offset as u8 * 8),
                );
            }
            canvas.fill_circle(
                (rect.x + rect.width / 5, rect.y + rect.height / 5),
                rect.width as f32 * 0.14,
                WHITE.with_alpha(12),
            );
        }
        _ => {
            for offset in 0..4 {
                let inset = 18 + offset * 16;
                canvas.stroke_round_rect(
                    Rect::new(
                        rect.x + inset,
                        rect.y + 42 + offset * 18,
                        rect.width - inset * 2,
                        100,
                    ),
                    50.0,
                    1.5,
                    item.accent.with_alpha(46 - offset as u8 * 8),
                );
            }
        }
    }
    for offset in 0..5 {
        let y = rect.y + 46 + offset * 34;
        canvas.line(
            rect.x + 22,
            y,
            rect.right() - 32,
            y - 54,
            1.0,
            WHITE.with_alpha(12),
        );
    }
    canvas.fill_round_rect(
        Rect::new(rect.x + 18, rect.y + 18, 2, 52),
        1.0,
        item.accent.with_alpha(200),
    );
}

fn draw_landscape(
    canvas: &mut Canvas,
    rect: Rect,
    item: &MediaItem,
    progress: Option<f32>,
    show_resume: bool,
    focused: bool,
) {
    if let Some(poster) = &item.poster {
        canvas.draw_image_cover(rect, 14.0, poster);
    } else {
        canvas.fill_round_vertical_gradient(rect, 14.0, item.palette.0, item.palette.1);
        draw_art_shapes(canvas, rect, item);
    }
    canvas.fill_round_vertical_gradient(
        rect,
        14.0,
        Color::rgba(0, 0, 0, 0),
        Color::rgba(0, 0, 0, 225),
    );
    if focused {
        canvas.fill_round_rect(rect, 14.0, WHITE.with_alpha(8));
    }
    let title = canvas.clipped(
        &item.title,
        (rect.width - 36) as f32,
        TextStyle::new(20.0, WHITE).semibold(),
    );
    text(
        canvas,
        &title,
        rect.x + 18,
        rect.bottom() - 44,
        20.0,
        WHITE,
        FontWeight::SemiBold,
    );
    let subtitle = if show_resume {
        if item.is_series() {
            format!(
                "S1 E1  ·  {}",
                item.resume_label()
                    .unwrap_or_else(|| "Ready to play".into())
            )
        } else {
            item.resume_label().unwrap_or_else(|| item.runtime_label())
        }
    } else {
        item.card_meta()
    };
    text(
        canvas,
        &subtitle,
        rect.x + 18,
        rect.bottom() - 20,
        12.0,
        WHITE.with_alpha(170),
        FontWeight::Regular,
    );
    if let Some(fraction) = progress {
        let track = Rect::new(rect.x + 18, rect.bottom() - 10, rect.width - 36, 3);
        canvas.fill_round_rect(track, 2.0, WHITE.with_alpha(48));
        canvas.fill_round_rect(
            Rect::new(track.x, track.y, (track.width as f32 * fraction) as i32, 3),
            2.0,
            Theme::PRIMARY_LIGHT,
        );
    }
    if item.is_series() {
        let badge = Rect::new(rect.x + 18, rect.y + 16, 58, 22);
        canvas.fill_round_rect(badge, 11.0, Color::rgba(0, 0, 0, 120));
        text_centered(
            canvas,
            "SERIES",
            badge.center().0,
            badge.y + 15,
            9.0,
            WHITE.with_alpha(220),
            FontWeight::SemiBold,
        );
    }
}

fn section_header(canvas: &mut Canvas, title: &str, action: &str, rect: Rect) {
    text(
        canvas,
        title,
        rect.x,
        rect.y + 30,
        25.0,
        Theme::TEXT,
        FontWeight::SemiBold,
    );
    text_right(
        canvas,
        action,
        Theme::CONTENT_X + Theme::CONTENT_W,
        rect.y + 28,
        13.0,
        Theme::MUTED,
        FontWeight::Regular,
    );
}

fn draw_button(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    icon: &str,
    primary: bool,
    focused: bool,
) {
    let fill = if primary {
        if focused {
            Theme::PRIMARY_LIGHT
        } else {
            Theme::PRIMARY
        }
    } else if focused {
        Theme::PANEL_HOVER
    } else {
        Theme::PANEL_RAISED
    };
    canvas.fill_round_rect(rect, 14.0, fill);
    if !primary {
        canvas.stroke_round_rect(rect, 14.0, 1.0, Theme::BORDER);
    }
    let text_color = if primary {
        WHITE
    } else if focused {
        Theme::TEXT
    } else {
        Theme::TEXT_SOFT
    };
    let icon = if label == "Play" || label == "Resume" {
        "play"
    } else {
        icon
    };
    if icon.is_empty() {
        text_centered(
            canvas,
            label,
            rect.center().0,
            rect.center().1 + 6,
            15.0,
            text_color,
            FontWeight::Medium,
        );
    } else {
        draw_icon(
            canvas,
            icon,
            Rect::new(rect.x + 28, rect.center().1 - 11, 22, 22),
            text_color,
        );
        text(
            canvas,
            label,
            rect.x + 62,
            rect.center().1 + 6,
            15.0,
            text_color,
            FontWeight::Medium,
        );
    }
}

fn draw_text_button(canvas: &mut Canvas, rect: Rect, label: &str, icon: &str, focused: bool) {
    canvas.fill_round_rect(
        rect,
        12.0,
        if focused {
            Theme::PANEL_RAISED
        } else {
            Color::rgba(8, 10, 14, 80)
        },
    );
    canvas.stroke_round_rect(
        rect,
        12.0,
        1.0,
        if focused {
            Theme::BORDER
        } else {
            WHITE.with_alpha(20)
        },
    );
    draw_icon(
        canvas,
        icon,
        Rect::new(rect.x + 18, rect.center().1 - 10, 20, 20),
        WHITE.with_alpha(220),
    );
    text(
        canvas,
        label,
        rect.x + 48,
        rect.center().1 + 5,
        14.0,
        WHITE,
        FontWeight::Medium,
    );
}

fn draw_icon(canvas: &mut Canvas, name: &str, rect: Rect, color: Color) {
    let (cx, cy) = rect.center();
    match name {
        "Home" | "home" => {
            canvas.line(rect.x + 3, cy, cx, rect.y + 3, 2.0, color);
            canvas.line(cx, rect.y + 3, rect.right() - 3, cy, 2.0, color);
            canvas.line(rect.x + 5, cy, rect.x + 5, rect.bottom() - 3, 2.0, color);
            canvas.line(
                rect.right() - 5,
                cy,
                rect.right() - 5,
                rect.bottom() - 3,
                2.0,
                color,
            );
            canvas.line(
                rect.x + 5,
                rect.bottom() - 3,
                rect.right() - 5,
                rect.bottom() - 3,
                2.0,
                color,
            );
            canvas.fill_round_rect(Rect::new(cx - 4, rect.bottom() - 10, 8, 10), 2.0, color);
        }
        "Library" | "library" => {
            canvas.fill_round_rect(
                Rect::new(rect.x + 2, rect.y + 3, 4, rect.height - 6),
                2.0,
                color,
            );
            canvas.fill_round_rect(
                Rect::new(rect.x + 9, rect.y + 3, 4, rect.height - 6),
                2.0,
                color,
            );
            canvas.fill_round_rect(
                Rect::new(rect.x + 16, rect.y + 3, 4, rect.height - 6),
                2.0,
                color,
            );
        }
        "Search" | "search" => {
            canvas.stroke_round_rect(Rect::new(rect.x + 2, rect.y + 2, 14, 14), 7.0, 2.0, color);
            canvas.line(
                rect.x + 15,
                rect.y + 15,
                rect.right() - 3,
                rect.bottom() - 3,
                2.0,
                color,
            );
        }
        "Settings" | "settings" => {
            canvas.stroke_round_rect(Rect::new(cx - 6, cy - 6, 12, 12), 6.0, 2.0, color);
            canvas.line(cx, rect.y + 1, cx, rect.y + 5, 2.0, color);
            canvas.line(cx, rect.bottom() - 5, cx, rect.bottom() - 1, 2.0, color);
            canvas.line(rect.x + 1, cy, rect.x + 5, cy, 2.0, color);
            canvas.line(rect.right() - 5, cy, rect.right() - 1, cy, 2.0, color);
        }
        "play" => {
            canvas.fill_triangle(
                (rect.x + 6, rect.y + 3),
                (rect.x + 6, rect.bottom() - 3),
                (rect.right() - 3, cy),
                color,
            );
        }
        "back" => {
            canvas.line(rect.x + 13, rect.y + 4, rect.x + 5, cy, 2.2, color);
            canvas.line(rect.x + 5, cy, rect.x + 13, rect.bottom() - 4, 2.2, color);
        }
        "rewind" => {
            canvas.fill_triangle(
                (rect.x + 3, cy),
                (rect.x + 15, rect.y + 6),
                (rect.x + 15, rect.bottom() - 6),
                color,
            );
            canvas.fill_triangle(
                (rect.x + 12, cy),
                (rect.x + 24, rect.y + 6),
                (rect.x + 24, rect.bottom() - 6),
                color,
            );
        }
        "forward" => {
            canvas.fill_triangle(
                (rect.right() - 3, cy),
                (rect.right() - 15, rect.y + 6),
                (rect.right() - 15, rect.bottom() - 6),
                color,
            );
            canvas.fill_triangle(
                (rect.right() - 12, cy),
                (rect.right() - 24, rect.y + 6),
                (rect.right() - 24, rect.bottom() - 6),
                color,
            );
        }
        _ => {
            canvas.fill_circle((cx, cy), 4.0, color);
        }
    }
}

fn text(
    canvas: &mut Canvas,
    value: &str,
    x: i32,
    baseline: i32,
    size: f32,
    color: Color,
    weight: FontWeight,
) {
    canvas.text(
        value,
        x,
        baseline,
        TextStyle::new(size, color).weight(weight),
    );
}

fn text_centered(
    canvas: &mut Canvas,
    value: &str,
    center_x: i32,
    baseline: i32,
    size: f32,
    color: Color,
    weight: FontWeight,
) {
    canvas.text_centered(
        value,
        center_x,
        baseline,
        TextStyle::new(size, color).weight(weight),
    );
}

fn text_right(
    canvas: &mut Canvas,
    value: &str,
    right: i32,
    baseline: i32,
    size: f32,
    color: Color,
    weight: FontWeight,
) {
    canvas.text_right(
        value,
        right,
        baseline,
        TextStyle::new(size, color).weight(weight),
    );
}
