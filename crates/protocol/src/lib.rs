//! Wire types for the ArchivePlayer server API.
//!
//! The server is the source of truth for these shapes. Keep this crate small
//! and dependency-light so it can also be used by a host-side simulator or a
//! future TV client. Fields are serialized in the server's camelCase form.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    #[serde(default)]
    pub has_users: bool,
    #[serde(default)]
    pub current_user: Option<User>,
    #[serde(default)]
    pub last_email: Option<String>,
    #[serde(default)]
    pub registration_open: bool,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: String,
    pub version: String,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
}

fn default_api_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: i64,
    pub identifier: String,
    pub title: String,
    #[serde(default = "default_media_type")]
    pub media_type: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub tmdb_poster_url: Option<String>,
    #[serde(default = "default_category")]
    pub category_name: String,
    #[serde(default)]
    pub watch_progress_seconds: i64,
    #[serde(default)]
    pub duration_seconds: i64,
    #[serde(default)]
    pub current_episode_file_name: Option<String>,
    #[serde(default)]
    pub last_watched: Option<String>,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub added_at: Option<String>,
}

fn default_media_type() -> String {
    "movies".into()
}

fn default_category() -> String {
    "Uncategorized".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSearchResult {
    pub identifier: String,
    pub title: String,
    pub media_type: String,
    pub thumbnail_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSearchResponse {
    #[serde(default)]
    pub results: Vec<ArchiveSearchResult>,
    #[serde(default)]
    pub has_more_results: bool,
    #[serde(default)]
    pub current_page: i64,
    #[serde(default)]
    pub total_results: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryGroup {
    pub name: String,
    #[serde(default)]
    pub items: Vec<LibraryItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResponse {
    #[serde(default)]
    pub categories: Vec<CategoryGroup>,
    #[serde(default)]
    pub category_names: Vec<String>,
    #[serde(default)]
    pub total_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    #[serde(default)]
    pub season_number: Option<i32>,
    #[serde(default)]
    pub episode_number: Option<i32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub still_url: Option<String>,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeProgress {
    pub file_name: String,
    #[serde(default)]
    pub position_seconds: i64,
    #[serde(default)]
    pub duration_seconds: i64,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFileUrl {
    pub file_name: String,
    pub url: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub length_seconds: Option<f64>,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub height: Option<i64>,
    #[serde(default)]
    pub video_codec: Option<String>,
}

/// The server-selected file. `compatible` is false when the file needs a
/// decoder the requested platform cannot rely on; show `reason` instead of
/// starting playback in that case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackPlan {
    pub platform: String,
    pub file_name: String,
    pub url: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub video_codec: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub length_seconds: Option<f64>,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub height: Option<i64>,
    #[serde(default = "default_true")]
    pub compatible: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetails {
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_media_type")]
    pub media_type: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub tmdb_poster_url: Option<String>,
    #[serde(default)]
    pub tmdb_id: Option<i64>,
    #[serde(default)]
    pub tmdb_media_type: Option<String>,
    #[serde(default)]
    pub video_files: Vec<String>,
    #[serde(default)]
    pub video_file_urls: Vec<VideoFileUrl>,
    #[serde(default)]
    pub playback_plan: Option<PlaybackPlan>,
    #[serde(default)]
    pub episodes: Vec<Episode>,
    #[serde(default)]
    pub watch_progress_seconds: i64,
    #[serde(default)]
    pub duration_seconds: i64,
    #[serde(default)]
    pub current_episode_file_name: Option<String>,
    #[serde(default)]
    pub last_watched: Option<String>,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default = "default_true")]
    pub autoplay_enabled: bool,
    #[serde(default)]
    pub library_item_id: Option<i64>,
    #[serde(default)]
    pub episode_progress: Vec<EpisodeProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStartRequest {
    pub client_name: String,
    pub platform: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    #[serde(default = "default_expires_in")]
    pub expires_in: i64,
    #[serde(default = "default_interval")]
    pub interval: i64,
}

fn default_expires_in() -> i64 {
    600
}

fn default_interval() -> i64 {
    5
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTokenRequest {
    pub device_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTokenResponse {
    pub token: String,
    pub user: User,
    #[serde(default)]
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressRequest {
    pub file_name: String,
    pub position_seconds: i64,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedRequest {
    pub file_name: String,
    pub item_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_playback_plan() {
        let details: MediaDetails = serde_json::from_str(
            r#"{
                "identifier": "movie",
                "title": "Movie",
                "videoFiles": ["movie.mp4"],
                "videoFileUrls": [{
                    "fileName": "movie.mp4",
                    "url": "https://archive.org/download/movie/movie.mp4",
                    "format": "h.264 IA",
                    "mimeType": "video/mp4",
                    "sizeBytes": 1234,
                    "lengthSeconds": 99.5,
                    "width": 1920,
                    "height": 1080,
                    "videoCodec": "h264"
                }],
                "playbackPlan": {
                    "platform": "webos",
                    "fileName": "movie.mp4",
                    "url": "https://archive.org/download/movie/movie.mp4",
                    "mimeType": "video/mp4",
                    "format": "h.264 IA",
                    "container": "mp4",
                    "videoCodec": "h264",
                    "sizeBytes": 1234,
                    "lengthSeconds": 99.5,
                    "width": 1920,
                    "height": 1080,
                    "compatible": true,
                    "reason": null
                }
            }"#,
        )
        .unwrap();
        let plan = details.playback_plan.unwrap();
        assert_eq!(plan.platform, "webos");
        assert_eq!(plan.video_codec.as_deref(), Some("h264"));
        assert_eq!(details.video_file_urls[0].length_seconds, Some(99.5));
    }

    #[test]
    fn tolerates_older_servers() {
        let details: MediaDetails = serde_json::from_str(
            r#"{"identifier":"movie","title":"Movie","videoFiles":["movie.mp4"]}"#,
        )
        .unwrap();
        assert!(details.playback_plan.is_none());
        assert!(details.video_file_urls.is_empty());
        assert_eq!(details.media_type, "movies");
    }

    #[test]
    fn serializes_requests_in_camel_case() {
        let value = serde_json::to_value(ProgressRequest {
            file_name: "movie.mp4".into(),
            position_seconds: 10,
            duration_seconds: 100,
        })
        .unwrap();
        assert_eq!(value["fileName"], "movie.mp4");
        assert_eq!(value["positionSeconds"], 10);
    }
}
