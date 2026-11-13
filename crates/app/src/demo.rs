//! Deterministic demo catalogue for the host simulator and screenshots.
//!
//! Titles are public-domain films; the artwork is generated so the repository
//! ships no unlicensed poster images.

use archiveplayer_tv_gfx::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Movie,
    Series,
}

#[derive(Debug, Clone)]
pub struct DemoEpisode {
    pub number: u16,
    pub title: &'static str,
    pub description: &'static str,
    pub runtime_minutes: u16,
    pub progress: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct DemoItem {
    pub id: i64,
    pub identifier: &'static str,
    pub title: &'static str,
    pub year: u16,
    pub media_type: MediaType,
    pub description: &'static str,
    pub tagline: &'static str,
    pub genres: &'static [&'static str],
    pub runtime_minutes: u16,
    pub rating: f32,
    pub progress: Option<(i64, i64)>,
    pub episodes: Vec<DemoEpisode>,
    pub palette: (Color, Color),
    pub accent: Color,
}

impl DemoItem {
    pub fn is_series(&self) -> bool {
        self.media_type == MediaType::Series
    }

    pub fn progress_fraction(&self) -> Option<f32> {
        self.progress
            .map(|(position, duration)| (position as f32 / duration.max(1) as f32).clamp(0.0, 1.0))
    }

    pub fn runtime_label(&self) -> String {
        match self.media_type {
            MediaType::Movie => format!("{} min", self.runtime_minutes),
            MediaType::Series => format!("{} episodes", self.episodes.len()),
        }
    }

    pub fn meta_label(&self) -> String {
        format!(
            "{}  ·  {}  ·  {}  ·  ★ {:.1}",
            self.year,
            self.genres.first().copied().unwrap_or("Feature"),
            self.runtime_label(),
            self.rating
        )
    }

    pub fn resume_label(&self) -> Option<String> {
        self.progress.map(|(position, duration)| {
            let remaining = (duration - position).max(0);
            format!("{} min left", (remaining + 59) / 60)
        })
    }
}

#[derive(Debug, Clone)]
pub struct DemoLibrary {
    pub items: Vec<DemoItem>,
    pub categories: Vec<&'static str>,
}

impl DemoLibrary {
    pub fn demo() -> Self {
        Self {
            items: items(),
            categories: vec!["All", "Movies", "Series", "Documentaries", "Noir", "Sci-Fi"],
        }
    }

    pub fn featured(&self) -> &DemoItem {
        &self.items[0]
    }

    pub fn item(&self, index: usize) -> &DemoItem {
        &self.items[index.min(self.items.len().saturating_sub(1))]
    }

    pub fn continue_watching(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.progress.is_some())
            .map(|(index, _)| index)
            .take(4)
            .collect()
    }

    pub fn recently_added(&self) -> Vec<usize> {
        (0..self.items.len()).take(4).collect()
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
    identifier: &'static str,
    title: &'static str,
    year: u16,
    description: &'static str,
    tagline: &'static str,
    genres: &'static [&'static str],
    runtime_minutes: u16,
    rating: f32,
    progress: Option<(i64, i64)>,
    palette: (u32, u32),
    accent: u32,
) -> DemoItem {
    DemoItem {
        id,
        identifier,
        title,
        year,
        media_type: MediaType::Movie,
        description,
        tagline,
        genres,
        runtime_minutes,
        rating,
        progress,
        episodes: Vec::new(),
        palette: (hex(palette.0), hex(palette.1)),
        accent: hex(accent),
    }
}

#[allow(clippy::too_many_arguments)]
fn series(
    id: i64,
    identifier: &'static str,
    title: &'static str,
    year: u16,
    description: &'static str,
    tagline: &'static str,
    genres: &'static [&'static str],
    rating: f32,
    episodes: Vec<DemoEpisode>,
    palette: (u32, u32),
    accent: u32,
) -> DemoItem {
    DemoItem {
        id,
        identifier,
        title,
        year,
        media_type: MediaType::Series,
        description,
        tagline,
        genres,
        runtime_minutes: 0,
        rating,
        progress: Some((1330, 1560)),
        episodes,
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

fn items() -> Vec<DemoItem> {
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
                DemoEpisode {
                    number: 1,
                    title: "Superman on Earth",
                    description: "Jor-El sends his infant son to Earth moments before Krypton dies.",
                    runtime_minutes: 26,
                    progress: Some(1330.0),
                },
                DemoEpisode {
                    number: 2,
                    title: "The Haunted Lighthouse",
                    description: "A suspicious lighthouse keeper and a missing inheritance draw Clark and Lois north.",
                    runtime_minutes: 26,
                    progress: Some(510.0),
                },
                DemoEpisode {
                    number: 3,
                    title: "The Case of the Talkative Dummy",
                    description: "A ventriloquist's act hides a message that someone is willing to kill for.",
                    runtime_minutes: 26,
                    progress: None,
                },
                DemoEpisode {
                    number: 4,
                    title: "The Mystery of the Broken Statues",
                    description: "A trail of shattered plaster leads the Daily Planet to a smuggling ring.",
                    runtime_minutes: 26,
                    progress: None,
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
