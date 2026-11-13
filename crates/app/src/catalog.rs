//! Deterministic demo catalogue for the host simulator and screenshots.
//!
//! Titles are public-domain films; the artwork is generated so the repository
//! ships no unlicensed poster images.

use archiveplayer_tv_gfx::{Color, Image};
use archiveplayer_tv_protocol::{
    ArchiveSearchResult, LibraryItem, LibraryResponse, MediaDetails, PlaybackPlan,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Movie,
    Series,
}

#[derive(Debug, Clone)]
pub struct EpisodeView {
    pub number: u16,
    pub title: String,
    pub description: String,
    pub runtime_minutes: u16,
    pub progress: Option<f64>,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: i64,
    pub identifier: String,
    pub title: String,
    pub year: u16,
    pub media_type: MediaType,
    pub description: String,
    pub tagline: String,
    pub genres: Vec<String>,
    pub category: String,
    pub runtime_minutes: u16,
    pub rating: f32,
    pub progress: Option<(i64, i64)>,
    pub episodes: Vec<EpisodeView>,
    pub file_count: usize,
    pub poster_url: Option<String>,
    pub poster: Option<Arc<Image>>,
    pub backdrop_url: Option<String>,
    pub backdrop: Option<Arc<Image>>,
    pub playback_plan: Option<PlaybackPlan>,
    pub palette: (Color, Color),
    pub accent: Color,
}

impl MediaItem {
    pub fn is_series(&self) -> bool {
        self.media_type == MediaType::Series
    }

    pub fn progress_fraction(&self) -> Option<f32> {
        self.progress
            .map(|(position, duration)| (position as f32 / duration.max(1) as f32).clamp(0.0, 1.0))
    }

    pub fn runtime_label(&self) -> String {
        match self.media_type {
            MediaType::Movie if self.runtime_minutes > 0 => {
                format!("{} min", self.runtime_minutes)
            }
            MediaType::Movie => "Feature".into(),
            MediaType::Series if !self.episodes.is_empty() => {
                format!("{} episodes", self.episodes.len())
            }
            MediaType::Series => "Series".into(),
        }
    }

    pub fn meta_label(&self) -> String {
        let mut parts = Vec::new();
        if self.year > 0 {
            parts.push(self.year.to_string());
        }
        if !self.category.is_empty() {
            parts.push(self.category.clone());
        } else if let Some(genre) = self.genres.first() {
            parts.push(genre.clone());
        }
        parts.push(self.runtime_label());
        if self.rating > 0.0 {
            parts.push(format!("★ {:.1}", self.rating));
        }
        parts.join("  ·  ")
    }

    pub fn card_meta(&self) -> String {
        let mut parts = Vec::new();
        if self.year > 0 {
            parts.push(self.year.to_string());
        }
        if !self.category.is_empty() {
            parts.push(self.category.clone());
        } else if let Some(genre) = self.genres.first() {
            parts.push(genre.clone());
        }
        parts.join("  ·  ")
    }

    pub fn resume_label(&self) -> Option<String> {
        self.progress.map(|(position, duration)| {
            let remaining = (duration - position).max(0);
            format!("{} min left", (remaining + 59) / 60)
        })
    }

    /// A deterministic, non-photographic palette. It keeps every screen
    /// recognizable while remote posters and wide backdrops load, and remains
    /// the fallback when a source has no usable artwork.
    fn live_palette(identifier: &str) -> ((Color, Color), Color) {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in identifier.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let hue = (hash & 0xff) as u8;
        let warm = Color::rgb(24 + (hue / 4), 22 + ((hue / 3) % 24), 28 + ((hue / 2) % 22));
        let accent = Color::rgb(
            120 + (hue % 90),
            92 + ((hue / 2) % 80),
            80 + ((hue / 3) % 90),
        );
        ((warm, Color::rgb(6, 8, 12)), accent)
    }

    pub fn placeholder() -> &'static MediaItem {
        static PLACEHOLDER: std::sync::OnceLock<MediaItem> = std::sync::OnceLock::new();
        PLACEHOLDER.get_or_init(|| MediaItem {
            id: 0,
            identifier: String::new(),
            title: "Nothing here yet".into(),
            year: 0,
            media_type: MediaType::Movie,
            description: "Add a title from the ArchivePlayer web app.".into(),
            tagline: String::new(),
            genres: Vec::new(),
            category: String::new(),
            runtime_minutes: 0,
            rating: 0.0,
            progress: None,
            episodes: Vec::new(),
            file_count: 0,
            poster_url: None,
            poster: None,
            backdrop_url: None,
            backdrop: None,
            playback_plan: None,
            palette: (Color::rgb(20, 23, 30), Color::rgb(7, 9, 13)),
            accent: Color::rgb(110, 150, 190),
        })
    }

    pub fn from_library_item(item: &LibraryItem) -> Self {
        let media_type = if item.media_type.to_lowercase().contains("tv")
            || item.media_type.to_lowercase().contains("series")
        {
            MediaType::Series
        } else {
            MediaType::Movie
        };
        let (palette, accent) = Self::live_palette(&item.identifier);
        let duration = item.duration_seconds.max(0);
        let progress =
            (duration > 0).then(|| (item.watch_progress_seconds.max(0).min(duration), duration));
        MediaItem {
            id: item.id,
            identifier: item.identifier.clone(),
            title: item.title.clone(),
            year: 0,
            media_type,
            description: String::new(),
            tagline: String::new(),
            genres: vec![item.category_name.clone()],
            category: item.category_name.clone(),
            runtime_minutes: (duration / 60) as u16,
            rating: 0.0,
            progress,
            episodes: Vec::new(),
            file_count: 0,
            poster_url: item
                .tmdb_poster_url
                .clone()
                .or_else(|| item.thumbnail_url.clone()),
            poster: None,
            backdrop_url: item.tmdb_backdrop_url.clone(),
            backdrop: None,
            playback_plan: None,
            palette,
            accent,
        }
    }
}

/// A search hit from Archive.org that is not in the library yet. It keeps the
/// palette and title so the result row can be drawn without a network image.
#[derive(Debug, Clone)]
pub struct SearchCard {
    pub identifier: String,
    pub title: String,
    pub kind: String,
    pub palette: (Color, Color),
    pub accent: Color,
}

impl SearchCard {
    pub fn from_result(result: &ArchiveSearchResult) -> Self {
        let (palette, accent) = MediaItem::live_palette(&result.identifier);
        let kind = match result.media_type.to_lowercase().as_str() {
            "movies" => "MOVIE".to_string(),
            "tv" | "series" => "SERIES".to_string(),
            "animation" => "ANIMATION".to_string(),
            "" => "ARCHIVE.ORG".to_string(),
            other => other.to_uppercase(),
        };
        Self {
            identifier: result.identifier.clone(),
            title: result.title.clone(),
            kind,
            palette,
            accent,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub items: Vec<MediaItem>,
    pub categories: Vec<String>,
}

impl Catalog {
    pub fn demo() -> Self {
        Self {
            items: items(),
            categories: ["All", "Movies", "Series", "Horror", "Noir", "Sci-Fi"]
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            categories: ["All", "Movies", "Series"]
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        }
    }

    /// Replace the catalogue with the server's library. Loaded details are
    /// preserved by identifier so a refresh does not discard an open synopsis.
    pub fn from_library(response: &LibraryResponse, previous: &Catalog) -> Self {
        let mut categories = ["All", "Movies", "Series"]
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        for name in &response.category_names {
            let name = name.trim();
            if !name.is_empty() && !categories.iter().any(|existing| existing == name) {
                categories.push(name.to_string());
            }
        }
        let mut items = Vec::new();
        for group in &response.categories {
            for item in &group.items {
                if !categories.iter().any(|name| name == &item.category_name) {
                    categories.push(item.category_name.clone());
                }
                if items
                    .iter()
                    .any(|existing: &MediaItem| existing.identifier == item.identifier)
                {
                    continue;
                }
                let mut mapped = MediaItem::from_library_item(item);
                if let Some(old) = previous
                    .items
                    .iter()
                    .find(|old| old.identifier == mapped.identifier)
                {
                    if !old.description.is_empty() {
                        mapped.description = old.description.clone();
                    }
                    if !old.episodes.is_empty() {
                        mapped.episodes = old.episodes.clone();
                    }
                    mapped.playback_plan = old.playback_plan.clone();
                    mapped.poster = old.poster.clone();
                    if old.backdrop_url == mapped.backdrop_url {
                        mapped.backdrop = old.backdrop.clone();
                    }
                }
                items.push(mapped);
            }
        }
        Self { items, categories }
    }

    pub fn featured(&self) -> Option<&MediaItem> {
        self.items.first()
    }

    pub fn item(&self, index: usize) -> &MediaItem {
        self.items
            .get(index)
            .unwrap_or_else(|| MediaItem::placeholder())
    }

    pub fn set_poster(&mut self, identifier: &str, image: Image) -> bool {
        let Some(index) = self.index_of(identifier) else {
            return false;
        };
        self.items[index].poster = Some(Arc::new(image));
        true
    }

    pub fn set_backdrop(&mut self, identifier: &str, image: Image) -> bool {
        let Some(index) = self.index_of(identifier) else {
            return false;
        };
        self.items[index].backdrop = Some(Arc::new(image));
        true
    }

    pub fn index_of(&self, identifier: &str) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.identifier == identifier)
    }

    pub fn apply_details(&mut self, details: &MediaDetails) -> bool {
        let Some(index) = self.index_of(&details.identifier) else {
            return false;
        };
        let item = &mut self.items[index];
        item.title = details.title.clone();
        if let Some(description) = &details.description {
            item.description = description.clone();
        }
        item.media_type = if details.media_type.to_lowercase().contains("tv")
            || details.media_type.to_lowercase().contains("series")
        {
            MediaType::Series
        } else {
            MediaType::Movie
        };
        item.runtime_minutes = (details.duration_seconds.max(0) / 60) as u16;
        item.progress = (details.duration_seconds > 0).then(|| {
            (
                details
                    .watch_progress_seconds
                    .max(0)
                    .min(details.duration_seconds),
                details.duration_seconds,
            )
        });
        item.poster_url = details
            .tmdb_poster_url
            .clone()
            .or_else(|| details.thumbnail_url.clone());
        let previous_backdrop = item.backdrop_url.clone();
        item.backdrop_url = details.tmdb_backdrop_url.clone();
        if previous_backdrop != item.backdrop_url {
            item.backdrop = None;
        }
        item.playback_plan = details.playback_plan.clone();
        item.file_count = details
            .video_file_urls
            .len()
            .max(details.video_files.len())
            .max(1);
        if item.media_type == MediaType::Movie {
            item.episodes.clear();
        }
        if item.media_type == MediaType::Series && !details.episodes.is_empty() {
            let progress = details
                .episode_progress
                .iter()
                .map(|progress| (progress.file_name.clone(), progress))
                .collect::<std::collections::HashMap<_, _>>();
            item.episodes = details
                .episodes
                .iter()
                .enumerate()
                .map(|(index, episode)| {
                    let saved = progress.get(&episode.file_name);
                    EpisodeView {
                        number: episode
                            .episode_number
                            .and_then(|number| u16::try_from(number).ok())
                            .unwrap_or((index + 1) as u16),
                        title: episode
                            .name
                            .clone()
                            .unwrap_or_else(|| episode.file_name.clone()),
                        description: episode.overview.clone().unwrap_or_default(),
                        runtime_minutes: saved
                            .map(|progress| (progress.duration_seconds.max(0) / 60) as u16)
                            .unwrap_or(0),
                        progress: saved.map(|progress| progress.position_seconds as f64),
                        file_name: Some(episode.file_name.clone()),
                    }
                })
                .collect();
        }
        true
    }

    pub fn continue_watching(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.progress
                    .is_some_and(|(position, duration)| position < duration)
            })
            .map(|(index, _)| index)
            .take(4)
            .collect()
    }

    pub fn recently_added(&self) -> Vec<usize> {
        (0..self.items.len()).take(4).collect()
    }

    pub fn suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();
        for item in &self.items {
            for value in std::iter::once(&item.category).chain(item.genres.iter()) {
                let value = value.trim();
                if !value.is_empty()
                    && !value.eq_ignore_ascii_case("movies")
                    && !value.eq_ignore_ascii_case("series")
                    && !suggestions
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(value))
                {
                    suggestions.push(value.to_string());
                }
                if suggestions.len() == 4 {
                    return suggestions;
                }
            }
        }
        suggestions
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return (0..self.items.len()).take(6).collect();
        }
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.title.to_lowercase().contains(&query)
                    || item.description.to_lowercase().contains(&query)
                    || item.category.to_lowercase().contains(&query)
                    || item
                        .genres
                        .iter()
                        .any(|genre| genre.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn movie(
    id: i64,
    identifier: &str,
    title: &str,
    year: u16,
    description: &str,
    tagline: &str,
    genres: &[&str],
    runtime_minutes: u16,
    rating: f32,
    progress: Option<(i64, i64)>,
    palette: (u32, u32),
    accent: u32,
) -> MediaItem {
    MediaItem {
        id,
        identifier: identifier.to_string(),
        title: title.to_string(),
        year,
        media_type: MediaType::Movie,
        description: description.to_string(),
        tagline: tagline.to_string(),
        genres: genres.iter().map(|genre| (*genre).to_string()).collect(),
        category: genres.first().copied().unwrap_or("Feature").to_string(),
        runtime_minutes,
        rating,
        progress,
        episodes: Vec::new(),
        file_count: 1,
        poster_url: None,
        poster: None,
        backdrop_url: None,
        backdrop: None,
        playback_plan: None,
        palette: (hex(palette.0), hex(palette.1)),
        accent: hex(accent),
    }
}

#[allow(clippy::too_many_arguments)]
fn series(
    id: i64,
    identifier: &str,
    title: &str,
    year: u16,
    description: &str,
    tagline: &str,
    genres: &[&str],
    rating: f32,
    episodes: Vec<EpisodeView>,
    palette: (u32, u32),
    accent: u32,
) -> MediaItem {
    MediaItem {
        id,
        identifier: identifier.to_string(),
        title: title.to_string(),
        year,
        media_type: MediaType::Series,
        description: description.to_string(),
        tagline: tagline.to_string(),
        genres: genres.iter().map(|genre| (*genre).to_string()).collect(),
        category: genres.first().copied().unwrap_or("Series").to_string(),
        runtime_minutes: 0,
        rating,
        progress: Some((1330, 1560)),
        episodes,
        file_count: 1,
        poster_url: None,
        poster: None,
        backdrop_url: None,
        backdrop: None,
        playback_plan: None,
        palette: (hex(palette.0), hex(palette.1)),
        accent: hex(accent),
    }
}

const fn hex(value: u32) -> Color {
    Color::rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

fn items() -> Vec<MediaItem> {
    vec![
        movie(
            1,
            "night_of_the_living_dead",
            "Night of the Living Dead",
            1968,
            "A group of strangers barricade themselves in a farmhouse while the recently dead rise and close in. A landmark of independent horror, shot on a shoestring and still impossible to shake.",
            "They won't stay dead.",
            &["Horror", "Thriller"],
            96,
            8.6,
            Some((4820, 5760)),
            (0x2b211d, 0x08090d),
            0xb88b5a,
        ),
        movie(
            2,
            "metropolis",
            "Metropolis",
            1927,
            "In a towering future city, the wealthy live above while workers toil beneath the machines. A young man and a visionary scientist collide with a machine-made double.",
            "The mediator between head and hands must be the heart.",
            &["Sci-Fi", "Drama"],
            153,
            8.8,
            Some((3120, 9180)),
            (0x23384b, 0x070b12),
            0x7fc7d9,
        ),
        movie(
            3,
            "nosferatu",
            "Nosferatu",
            1922,
            "An estate agent's journey to a remote castle brings a plague-bearing count to a quiet town. Expressionist shadows, a rat-like silhouette, and one of cinema's first vampires.",
            "A symphony of horror.",
            &["Horror", "Silent"],
            94,
            8.4,
            Some((1560, 5640)),
            (0x35202c, 0x08070c),
            0xd06a86,
        ),
        movie(
            4,
            "the_cabinet_of_dr_caligari",
            "The Cabinet of Dr. Caligari",
            1920,
            "A somnambulist is commanded to kill by a sinister showman. Painted sets lean at impossible angles, making the whole world feel like a fever dream.",
            "You must become Caligari.",
            &["Horror", "Silent"],
            77,
            8.2,
            None,
            (0x3c301f, 0x0a090b),
            0xd1ab58,
        ),
        movie(
            5,
            "charade",
            "Charade",
            1963,
            "A widow discovers her late husband stole a fortune, and several charming strangers want it back. A playful thriller with Paris, banter, and a very good chase.",
            "Who is the man in the shadows?",
            &["Thriller", "Romance"],
            113,
            8.0,
            Some((2400, 6780)),
            (0x3b252d, 0x0b090d),
            0xe59aae,
        ),
        series(
            6,
            "adventures_of_superman",
            "Adventures of Superman",
            1952,
            "The Man of Steel protects Metropolis from gangsters, mad scientists, and a surprising number of exploding warehouses. A fondly remembered slice of early television.",
            "Truth, justice, and a faster-than-a-speeding-bullet schedule.",
            &["Adventure", "Classic TV"],
            7.6,
            vec![
                EpisodeView {
                    number: 1,
                    title: "Superman on Earth".into(),
                    description: "Jor-El sends his infant son to Earth moments before Krypton dies."
                        .into(),
                    runtime_minutes: 26,
                    progress: Some(1330.0),
                    file_name: None,
                },
                EpisodeView {
                    number: 2,
                    title: "The Haunted Lighthouse".into(),
                    description: "A suspicious lighthouse keeper and a missing inheritance draw Clark and Lois north."
                        .into(),
                    runtime_minutes: 26,
                    progress: Some(510.0),
                    file_name: None,
                },
                EpisodeView {
                    number: 3,
                    title: "The Case of the Talkative Dummy".into(),
                    description: "A ventriloquist's act hides a message that someone is willing to kill for."
                        .into(),
                    runtime_minutes: 26,
                    progress: None,
                    file_name: None,
                },
                EpisodeView {
                    number: 4,
                    title: "The Mystery of the Broken Statues".into(),
                    description: "A trail of shattered plaster leads the Daily Planet to a smuggling ring."
                        .into(),
                    runtime_minutes: 26,
                    progress: None,
                    file_name: None,
                },
            ],
            (0x252d3d, 0x08090d),
            0xe05d54,
        ),
        movie(
            7,
            "plan_9_from_outer_space",
            "Plan 9 from Outer Space",
            1959,
            "Aliens resurrect the dead to stop humanity from destroying the universe. Ed Wood's gloriously earnest misfire is a monument to making the impossible anyway.",
            "Future events such as these will affect you.",
            &["Sci-Fi", "Cult"],
            79,
            4.0,
            None,
            (0x1f2b29, 0x070b0d),
            0x77b79c,
        ),
        movie(
            8,
            "carnival_of_souls",
            "Carnival of Souls",
            1962,
            "After a car crash, a woman takes a job as a church organist and is drawn to an abandoned lakeside pavilion. A low-budget ghost story with the logic of a bad dream.",
            "She survived the accident. Or did she?",
            &["Horror", "Cult"],
            78,
            7.3,
            None,
            (0x28233b, 0x07070e),
            0x9b8bd8,
        ),
        movie(
            9,
            "the_general",
            "The General",
            1926,
            "A railroad engineer chases his stolen locomotive, and the woman he loves, through the Civil War. Buster Keaton's stunts are still astonishingly physical.",
            "A comedy of the Civil War.",
            &["Comedy", "Silent"],
            78,
            8.2,
            None,
            (0x342a20, 0x0a0909),
            0xc98d58,
        ),
        movie(
            10,
            "sherlock_jr",
            "Sherlock Jr.",
            1924,
            "A projectionist dreams himself into the film he is showing. Keaton walks through a movie screen and into a cascade of sight gags and impossible physics.",
            "Every inch a detective.",
            &["Comedy", "Silent"],
            45,
            8.2,
            None,
            (0x252b3c, 0x08090d),
            0x86a7d8,
        ),
        movie(
            11,
            "detour",
            "Detour",
            1945,
            "A pianist hitchhiking to Los Angeles is pulled into a spiral of bad luck, borrowed identities, and a blackmailing stranger. Poverty-row noir at its most fatalistic.",
            "He went looking for trouble.",
            &["Noir", "Thriller"],
            68,
            7.4,
            None,
            (0x32241f, 0x09080a),
            0xc27a5f,
        ),
        movie(
            12,
            "the_kid",
            "The Kid",
            1921,
            "A tramp raises an abandoned child and the two build a life on the margins. Chaplin balances slapstick with a tenderness that still lands a century later.",
            "A picture with a smile and perhaps a tear.",
            &["Comedy", "Drama"],
            68,
            8.3,
            None,
            (0x332b23, 0x0b0a0a),
            0xd5b17a,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use archiveplayer_tv_protocol::{
        Episode, EpisodeProgress, LibraryItem, LibraryResponse, MediaDetails,
    };

    fn library_item() -> LibraryItem {
        LibraryItem {
            id: 7,
            identifier: "night_of_the_living_dead".into(),
            title: "Night of the Living Dead".into(),
            media_type: "movies".into(),
            thumbnail_url: Some("https://archive.org/services/img/night".into()),
            tmdb_poster_url: None,
            tmdb_backdrop_url: Some("https://image.tmdb.org/t/p/w1280/backdrop.jpg".into()),
            category_name: "Horror".into(),
            watch_progress_seconds: 120,
            duration_seconds: 5_760,
            current_episode_file_name: None,
            last_watched: None,
            is_completed: false,
            added_at: None,
        }
    }

    #[test]
    fn maps_a_server_library_and_keeps_progress() {
        let response = LibraryResponse {
            categories: vec![archiveplayer_tv_protocol::CategoryGroup {
                name: "Horror".into(),
                items: vec![library_item()],
            }],
            category_names: vec!["Horror".into()],
            total_items: 1,
        };
        let catalog = Catalog::from_library(&response, &Catalog::empty());
        assert_eq!(catalog.items.len(), 1);
        assert!(catalog.categories.iter().any(|name| name == "Horror"));
        let item = catalog.item(0);
        assert_eq!(item.title, "Night of the Living Dead");
        assert_eq!(item.progress, Some((120, 5_760)));
        assert!(item.description.is_empty());
        assert_eq!(
            item.backdrop_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w1280/backdrop.jpg")
        );
        assert_eq!(catalog.continue_watching(), vec![0]);
    }

    #[test]
    fn details_fill_the_item_and_movies_do_not_become_series() {
        let response = LibraryResponse {
            categories: vec![archiveplayer_tv_protocol::CategoryGroup {
                name: "Horror".into(),
                items: vec![library_item()],
            }],
            category_names: vec!["Horror".into()],
            total_items: 1,
        };
        let mut catalog = Catalog::from_library(&response, &Catalog::empty());
        let details = MediaDetails {
            identifier: "night_of_the_living_dead".into(),
            title: "Night of the Living Dead".into(),
            description: Some("A farmhouse siege.".into()),
            media_type: "movies".into(),
            thumbnail_url: None,
            tmdb_poster_url: None,
            tmdb_backdrop_url: None,
            tmdb_id: None,
            tmdb_media_type: None,
            video_files: vec!["a.mp4".into(), "b.mp4".into()],
            video_file_urls: Vec::new(),
            playback_plan: Some(PlaybackPlan {
                platform: "webos".into(),
                file_name: "a.mp4".into(),
                url: "https://archive.org/a.mp4".into(),
                mime_type: Some("video/mp4".into()),
                format: Some("h.264".into()),
                container: Some("mp4".into()),
                video_codec: Some("h264".into()),
                size_bytes: None,
                length_seconds: Some(5_760.0),
                width: Some(1920),
                height: Some(1080),
                compatible: true,
                reason: None,
            }),
            episodes: vec![Episode {
                season_number: Some(1),
                episode_number: Some(1),
                name: Some("Part one".into()),
                overview: None,
                still_url: None,
                file_name: "a.mp4".into(),
            }],
            watch_progress_seconds: 120,
            duration_seconds: 5_760,
            current_episode_file_name: None,
            last_watched: None,
            is_completed: false,
            autoplay_enabled: true,
            library_item_id: Some(7),
            episode_progress: vec![EpisodeProgress {
                file_name: "a.mp4".into(),
                position_seconds: 120,
                duration_seconds: 5_760,
                completed: false,
            }],
        };
        assert!(catalog.apply_details(&details));
        let item = catalog.item(0);
        assert_eq!(item.description, "A farmhouse siege.");
        assert_eq!(item.runtime_minutes, 96);
        assert_eq!(item.file_count, 2);
        assert_eq!(item.media_type, MediaType::Movie);
        assert!(item.episodes.is_empty());
        assert!(item.playback_plan.is_some());
    }

    #[test]
    fn posters_survive_a_library_refresh() {
        let response = LibraryResponse {
            categories: vec![archiveplayer_tv_protocol::CategoryGroup {
                name: "Horror".into(),
                items: vec![library_item()],
            }],
            category_names: vec!["Horror".into()],
            total_items: 1,
        };
        let mut previous = Catalog::from_library(&response, &Catalog::empty());
        let image = Image::from_rgba(2, 2, [120, 20, 10, 255].repeat(4)).unwrap();
        assert!(previous.set_poster("night_of_the_living_dead", image));
        assert!(!previous.set_poster("missing", Image::from_rgba(2, 2, vec![0; 16]).unwrap()));
        let next = Catalog::from_library(&response, &previous);
        assert!(next.item(0).poster.is_some());
    }

    #[test]
    fn backdrops_survive_a_library_refresh() {
        let response = LibraryResponse {
            categories: vec![archiveplayer_tv_protocol::CategoryGroup {
                name: "Horror".into(),
                items: vec![library_item()],
            }],
            category_names: vec!["Horror".into()],
            total_items: 1,
        };
        let mut previous = Catalog::from_library(&response, &Catalog::empty());
        let image = Image::from_rgba(2, 2, [30, 60, 120, 255].repeat(4)).unwrap();
        assert!(previous.set_backdrop("night_of_the_living_dead", image));
        let next = Catalog::from_library(&response, &previous);
        assert!(next.item(0).backdrop.is_some());
    }

    #[test]
    fn an_empty_catalogue_has_a_safe_placeholder() {
        let catalog = Catalog::empty();
        assert!(catalog.featured().is_none());
        let placeholder = catalog.item(0);
        assert!(placeholder.title.contains("Nothing"));
        assert!(catalog.search("anything").is_empty());
    }
}
