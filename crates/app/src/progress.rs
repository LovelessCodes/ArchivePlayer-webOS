//! Playback progress scheduling.
//!
//! The render loop calls `ProgressTracker` continuously. The tracker decides
//! when a save is due, emits completion exactly once, and resets when the user
//! opens a different title or file. It has no network or clock dependencies
//! beyond the `Instant` passed by the caller, so the same policy runs on the
//! host simulator and on the television.

use std::time::{Duration, Instant};

use crate::catalog::MediaItem;

/// Save while playback is running at this interval.
pub const PROGRESS_INTERVAL: Duration = Duration::from_secs(15);

/// Consider a title finished this close to the end. The server also treats a
/// completed request as authoritative; this only avoids sending a save at
/// 99.9% right before the completed marker.
pub const COMPLETION_MARGIN_SECONDS: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTarget {
    pub identifier: String,
    pub file_name: String,
    /// `true` for a movie, `false` for one episode of a series.
    pub item_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressAction {
    Save {
        target: PlaybackTarget,
        position_seconds: i64,
        duration_seconds: i64,
    },
    Completed {
        target: PlaybackTarget,
    },
}

impl ProgressAction {
    pub fn target(&self) -> &PlaybackTarget {
        match self {
            Self::Save { target, .. } | Self::Completed { target } => target,
        }
    }
}

/// Pick the server-selected file for the currently open title.
///
/// A title without a loaded playback plan is not reportable: guessing a file
/// name would write progress against the wrong episode.
pub fn target_for(item: &MediaItem) -> Option<PlaybackTarget> {
    let plan = item.playback_plan.as_ref()?;
    if item.identifier.is_empty() || plan.file_name.is_empty() {
        return None;
    }
    Some(PlaybackTarget {
        identifier: item.identifier.clone(),
        file_name: plan.file_name.clone(),
        item_completed: !item.is_series(),
    })
}

#[derive(Debug)]
pub struct ProgressTracker {
    target: Option<PlaybackTarget>,
    last_sent: Instant,
    completed: bool,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            target: None,
            last_sent: Instant::now(),
            completed: false,
        }
    }

    /// Feed the current playback state. Returns an action when a periodic save
    /// or completion is due. The first observation of a target only arms the
    /// timer so opening a title does not immediately write progress. A paused
    /// player still emits completion at the end but does not repeat the same
    /// position every interval.
    pub fn observe(
        &mut self,
        target: Option<&PlaybackTarget>,
        position_seconds: i64,
        duration_seconds: i64,
        playing: bool,
        now: Instant,
    ) -> Option<ProgressAction> {
        let target = target?;
        if duration_seconds <= 0 {
            return None;
        }
        let position = position_seconds.clamp(0, duration_seconds);
        if self.target.as_ref() != Some(target) {
            self.target = Some(target.clone());
            self.last_sent = now;
            self.completed = false;
            return None;
        }
        if self.completed {
            return None;
        }
        if position >= duration_seconds.saturating_sub(COMPLETION_MARGIN_SECONDS) {
            self.completed = true;
            self.last_sent = now;
            return Some(ProgressAction::Completed {
                target: target.clone(),
            });
        }
        if !playing {
            return None;
        }
        if now.saturating_duration_since(self.last_sent) >= PROGRESS_INTERVAL {
            self.last_sent = now;
            return Some(ProgressAction::Save {
                target: target.clone(),
                position_seconds: position,
                duration_seconds,
            });
        }
        None
    }

    /// Save immediately, for pause, Back, and quit. A completed target is not
    /// overwritten with a late progress write.
    pub fn flush(
        &mut self,
        target: &PlaybackTarget,
        position_seconds: i64,
        duration_seconds: i64,
        now: Instant,
    ) -> Option<ProgressAction> {
        if duration_seconds <= 0 || self.completed {
            return None;
        }
        let position = position_seconds.clamp(0, duration_seconds);
        if self.target.as_ref() != Some(target) {
            self.target = Some(target.clone());
            self.completed = false;
        }
        if position >= duration_seconds.saturating_sub(COMPLETION_MARGIN_SECONDS) {
            self.completed = true;
            self.last_sent = now;
            return Some(ProgressAction::Completed {
                target: target.clone(),
            });
        }
        self.last_sent = now;
        Some(ProgressAction::Save {
            target: target.clone(),
            position_seconds: position,
            duration_seconds,
        })
    }

    /// Reset when playback stops without saving, for example a repaint-only
    /// preview or a switch back to the demo catalogue.
    pub fn reset(&mut self) {
        self.target = None;
        self.completed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn plan() -> archiveplayer_tv_protocol::PlaybackPlan {
        archiveplayer_tv_protocol::PlaybackPlan {
            platform: "webos".into(),
            file_name: "movie.mp4".into(),
            url: "https://example.test/movie.mp4".into(),
            mime_type: Some("video/mp4".into()),
            format: Some("MPEG4".into()),
            container: Some("mp4".into()),
            video_codec: Some("h264".into()),
            size_bytes: Some(1),
            length_seconds: Some(600.0),
            width: Some(1920),
            height: Some(1080),
            compatible: true,
            reason: None,
        }
    }

    fn target() -> PlaybackTarget {
        let mut catalog = Catalog::demo();
        catalog.items[0].identifier = "night_of_the_living_dead".into();
        catalog.items[0].playback_plan = Some(plan());
        target_for(&catalog.items[0]).expect("the demo title should carry a plan")
    }

    #[test]
    fn first_observation_arms_the_timer() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        assert_eq!(tracker.observe(Some(&target), 10, 600, true, now), None);
        assert_eq!(tracker.observe(Some(&target), 20, 600, true, now), None);
    }

    #[test]
    fn saves_every_interval() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        assert_eq!(tracker.observe(Some(&target), 10, 600, true, now), None);
        let action = tracker
            .observe(Some(&target), 30, 600, true, now + PROGRESS_INTERVAL)
            .expect("the interval should produce a save");
        assert!(matches!(
            action,
            ProgressAction::Save {
                position_seconds: 30,
                duration_seconds: 600,
                ..
            }
        ));
        assert_eq!(
            tracker.observe(Some(&target), 40, 600, true, now + PROGRESS_INTERVAL),
            None
        );
    }

    #[test]
    fn changing_the_file_resets_the_timer() {
        let target = target();
        let mut other = target.clone();
        other.file_name = "episode-2.mp4".into();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        assert_eq!(tracker.observe(Some(&target), 10, 600, true, now), None);
        assert_eq!(
            tracker.observe(Some(&other), 10, 600, true, now + PROGRESS_INTERVAL),
            None
        );
        assert!(matches!(
            tracker.observe(Some(&other), 30, 600, true, now + PROGRESS_INTERVAL * 2),
            Some(ProgressAction::Save { .. })
        ));
    }

    #[test]
    fn completes_once_near_the_end() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        tracker.observe(Some(&target), 10, 600, true, now);
        let action = tracker.observe(Some(&target), 598, 600, true, now);
        assert!(matches!(action, Some(ProgressAction::Completed { .. })));
        assert_eq!(tracker.observe(Some(&target), 600, 600, true, now), None);
        assert_eq!(tracker.flush(&target, 600, 600, now), None);
    }

    #[test]
    fn a_paused_player_does_not_repeat_saves() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        tracker.observe(Some(&target), 10, 600, true, now);
        assert_eq!(
            tracker.observe(Some(&target), 10, 600, false, now + PROGRESS_INTERVAL),
            None
        );
        assert!(matches!(
            tracker.observe(Some(&target), 598, 600, false, now + PROGRESS_INTERVAL),
            Some(ProgressAction::Completed { .. })
        ));
    }

    #[test]
    fn flush_at_the_end_completes_instead_of_saving() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        tracker.observe(Some(&target), 10, 600, true, now);
        assert!(matches!(
            tracker.flush(&target, 599, 600, now),
            Some(ProgressAction::Completed { .. })
        ));
        assert_eq!(tracker.flush(&target, 599, 600, now), None);
    }

    #[test]
    fn flush_saves_a_paused_title() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        tracker.observe(Some(&target), 10, 600, true, now);
        let action = tracker
            .flush(&target, 42, 600, now)
            .expect("pause should save immediately");
        assert!(matches!(
            action,
            ProgressAction::Save {
                position_seconds: 42,
                ..
            }
        ));
    }

    #[test]
    fn ignores_unknown_durations() {
        let target = target();
        let mut tracker = ProgressTracker::new();
        let now = Instant::now();
        assert_eq!(tracker.observe(Some(&target), 10, 0, true, now), None);
        assert_eq!(tracker.flush(&target, 10, 0, now), None);
    }

    #[test]
    fn only_movies_mark_the_whole_item_completed() {
        let movie = target();
        assert!(movie.item_completed);
        let mut catalog = Catalog::demo();
        catalog.items[5].identifier = "a_series".into();
        catalog.items[5].playback_plan = Some(plan());
        let episode = target_for(&catalog.items[5]).unwrap();
        assert!(!episode.item_completed);
    }

    #[test]
    fn a_title_without_a_plan_is_not_reportable() {
        let catalog = Catalog::demo();
        assert!(target_for(catalog.item(0)).is_none());
    }
}
