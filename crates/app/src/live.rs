//! Worker thread that owns the HTTP connection and the pairing grant.
//!
//! The UI is a synchronous render loop. Network calls, pairing polls, and
//! token storage therefore live on one worker thread and communicate through
//! bounded channels. The same seam is used by the host simulator and the
//! webOS shell; only the renderer and the input source differ.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use archiveplayer_tv_api::{
    connect_target, fetch_image_bytes, ApiClient, ApiError, ConnectedServer, ServerTarget,
    StoredSession,
};
use archiveplayer_tv_gfx::Image;
use archiveplayer_tv_platform::{FileSecretStore, SecretStore};
use archiveplayer_tv_protocol::{CompletedRequest, LibraryResponse, MediaDetails, ProgressRequest};

use crate::pairing::{PairingFlow, PairingPoll};
use crate::poster::decode_poster;

const CLIENT_NAME: &str = "ArchivePlayer TV";
const DEVICE_NAME: &str = "webOS TV";
const POLL_SLICE: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingView {
    Connecting,
    Waiting,
    Approved,
    Denied,
    Expired,
    Error,
}

#[derive(Debug)]
pub enum LiveEvent {
    Status(String),
    Connected {
        name: String,
        url: String,
        version: String,
        lan: bool,
    },
    PairingRequired(String),
    PairingCode {
        code: String,
        uri: String,
        complete_uri: String,
        expires_in_seconds: i64,
    },
    PairingStatus(PairingView),
    Library(LibraryResponse),
    Details(Box<MediaDetails>),
    Poster {
        identifier: String,
        image: Image,
    },
    Error(String),
}

#[derive(Debug)]
pub enum LiveCommand {
    StartPairing,
    Refresh,
    LoadDetails(String),
    SaveProgress {
        identifier: String,
        file_name: String,
        position_seconds: i64,
        duration_seconds: i64,
    },
    MarkCompleted {
        identifier: String,
        file_name: String,
        item_completed: bool,
    },
    Shutdown,
}

pub struct LiveClient {
    commands: Sender<LiveCommand>,
    events: Receiver<LiveEvent>,
    worker: Option<JoinHandle<()>>,
    posters: PosterQueue,
}

struct PosterRequest {
    identifier: String,
    url: String,
}

/// Two download/decode threads keep slow Archive.org thumbnails from starving
/// each other while leaving the API worker free for pairing and playback.
struct PosterQueue {
    senders: Vec<Sender<PosterRequest>>,
    next: AtomicUsize,
    workers: Vec<JoinHandle<()>>,
}

impl PosterQueue {
    fn start(events: Sender<LiveEvent>) -> Self {
        let mut senders = Vec::new();
        let mut workers = Vec::new();
        for index in 0..2 {
            let (sender, receiver) = mpsc::channel::<PosterRequest>();
            senders.push(sender);
            let events = events.clone();
            workers.push(
                thread::Builder::new()
                    .name(format!("archiveplayer-poster-{index}"))
                    .spawn(move || {
                        while let Ok(request) = receiver.recv() {
                            let Ok(bytes) = fetch_image_bytes(&request.url) else {
                                continue;
                            };
                            let Ok(image) = decode_poster(&bytes) else {
                                continue;
                            };
                            let _ = events.send(LiveEvent::Poster {
                                identifier: request.identifier,
                                image,
                            });
                        }
                    })
                    .expect("the poster worker thread should start"),
            );
        }
        Self {
            senders,
            next: AtomicUsize::new(0),
            workers,
        }
    }

    fn enqueue(&self, identifier: String, url: String) {
        if self.senders.is_empty() {
            return;
        }
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.senders.len();
        let _ = self.senders[index].send(PosterRequest { identifier, url });
    }
}

impl Drop for PosterQueue {
    fn drop(&mut self) {
        self.senders.clear();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl LiveClient {
    pub fn start(target: ServerTarget, session_path: PathBuf) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let posters = PosterQueue::start(event_tx.clone());
        let worker = thread::Builder::new()
            .name("archiveplayer-live".into())
            .spawn(move || Worker::new(target, session_path, event_tx).run(command_rx))
            .expect("the live worker thread should start");
        Self {
            commands: command_tx,
            events: event_rx,
            worker: Some(worker),
            posters,
        }
    }

    pub fn send(&self, command: LiveCommand) {
        let _ = self.commands.send(command);
    }

    /// Queue artwork for a title. Failures fall back to procedural artwork, so
    /// the UI is never blocked or emptied by a poster.
    pub fn load_poster(&self, identifier: String, url: String) {
        self.posters.enqueue(identifier, url);
    }

    /// Drain events without blocking the render loop.
    pub fn events(&self) -> Vec<LiveEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for LiveClient {
    fn drop(&mut self) {
        let _ = self.commands.send(LiveCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn same_server(left: &ServerTarget, right: &ServerTarget) -> bool {
    let normalize = |url: &str| url.trim_end_matches('/').to_string();
    normalize(&left.lan_url) == normalize(&right.lan_url)
        || match (&left.remote_url, &right.remote_url) {
            (Some(left), Some(right)) => normalize(left) == normalize(right),
            _ => false,
        }
}

struct Worker {
    target: ServerTarget,
    store: FileSecretStore,
    events: Sender<LiveEvent>,
    session: Option<StoredSession>,
    client: Option<ApiClient>,
    pairing: Option<PairingFlow>,
    next_poll: Instant,
}

impl Worker {
    fn new(target: ServerTarget, session_path: PathBuf, events: Sender<LiveEvent>) -> Self {
        Self {
            target,
            store: FileSecretStore::new(session_path),
            events,
            session: None,
            client: None,
            pairing: None,
            next_poll: Instant::now(),
        }
    }

    fn run(mut self, commands: Receiver<LiveCommand>) {
        self.session = self
            .store
            .load()
            .ok()
            .flatten()
            .filter(|session| same_server(&session.target, &self.target));
        if self.session.is_some() {
            self.connect_saved();
        } else {
            let _ = self.store.clear();
            self.start_pairing("Pair this TV to your server");
        }

        loop {
            match commands.recv_timeout(POLL_SLICE) {
                Ok(LiveCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(command) => self.handle(command),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            self.poll_pairing();
        }
    }

    fn send(&self, event: LiveEvent) {
        let _ = self.events.send(event);
    }

    fn connect_saved(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match connect_target(&session.target, Some(session.token.clone())) {
            Ok(connected) => {
                self.target = session.target.clone();
                self.connected(connected);
                self.refresh_library();
            }
            Err(ApiError::Unauthorized) => {
                self.forget_session();
                self.start_pairing("This TV was unpaired. Scan the new code to reconnect.");
            }
            Err(error) => {
                self.send(LiveEvent::Error(error.to_string()));
                self.send(LiveEvent::PairingRequired(error.to_string()));
                self.send(LiveEvent::PairingStatus(PairingView::Error));
            }
        }
    }

    fn connected(&mut self, connected: ConnectedServer) {
        self.target.server_version = Some(connected.health.version.clone());
        self.target.api_version = connected.health.api_version;
        if let Some(session) = &mut self.session {
            session.target = self.target.clone();
            let _ = self.store.save(session);
        }
        let url = connected.client.base_url().to_string();
        let version = connected.health.version.clone();
        self.client = Some(connected.client);
        self.send(LiveEvent::Connected {
            name: self.target.name.clone(),
            url,
            version,
            lan: !connected.used_remote,
        });
        self.send(LiveEvent::PairingStatus(PairingView::Approved));
    }

    fn refresh_library(&mut self) {
        let Some(client) = &self.client else {
            return;
        };
        match client.library() {
            Ok(library) => self.send(LiveEvent::Library(library)),
            Err(error) => self.handle_api_error(error, "could not load the library"),
        }
    }

    fn load_details(&mut self, identifier: String) {
        let Some(client) = &self.client else {
            return;
        };
        match client.playback_details(&identifier, Some("webos"), None) {
            Ok(details) => self.send(LiveEvent::Details(Box::new(details))),
            Err(error) => self.handle_api_error(error, "could not load title details"),
        }
    }

    fn handle_api_error(&mut self, error: ApiError, context: &str) {
        if error.is_unauthorized() {
            self.forget_session();
            self.start_pairing("The server session expired. Pair this TV again.");
        } else {
            self.send(LiveEvent::Error(format!("{context}: {error}")));
        }
    }

    fn forget_session(&mut self) {
        self.session = None;
        self.client = None;
        let _ = self.store.clear();
    }

    fn start_pairing(&mut self, reason: &str) {
        self.client = None;
        self.pairing = None;
        self.send(LiveEvent::PairingRequired(reason.to_string()));
        self.send(LiveEvent::PairingStatus(PairingView::Connecting));
        match PairingFlow::start(&self.target, CLIENT_NAME, DEVICE_NAME) {
            Ok(flow) => {
                self.send(LiveEvent::PairingCode {
                    code: flow.user_code().to_string(),
                    uri: flow.verification_uri().to_string(),
                    complete_uri: flow.verification_uri_complete().to_string(),
                    expires_in_seconds: flow.expires_in_seconds(),
                });
                self.next_poll =
                    Instant::now() + Duration::from_secs(flow.interval_seconds() as u64);
                self.pairing = Some(flow);
            }
            Err(error) => {
                self.send(LiveEvent::Error(format!(
                    "could not start pairing: {error}"
                )));
                self.send(LiveEvent::PairingStatus(PairingView::Error));
            }
        }
    }

    fn poll_pairing(&mut self) {
        if Instant::now() < self.next_poll {
            return;
        }
        let Some(flow) = &self.pairing else {
            return;
        };
        let interval = Duration::from_secs(flow.interval_seconds() as u64);
        match flow.poll() {
            Ok(PairingPoll::Pending) => {
                self.next_poll = Instant::now() + interval;
            }
            Ok(PairingPoll::SlowDown {
                retry_after_seconds,
            }) => {
                self.next_poll = Instant::now() + Duration::from_secs(retry_after_seconds.max(1));
            }
            Ok(PairingPoll::Approved(session)) => {
                self.pairing = None;
                self.session = Some(*session);
                if let Some(session) = &self.session {
                    if let Err(error) = self.store.save(session) {
                        self.send(LiveEvent::Error(format!(
                            "could not save the session: {error}"
                        )));
                    }
                }
                self.send(LiveEvent::PairingStatus(PairingView::Approved));
                self.connect_saved();
            }
            Ok(PairingPoll::Denied(message)) => {
                self.pairing = None;
                self.send(LiveEvent::PairingStatus(PairingView::Denied));
                self.send(LiveEvent::Error(message));
            }
            Ok(PairingPoll::Expired(message)) => {
                self.pairing = None;
                self.send(LiveEvent::PairingStatus(PairingView::Expired));
                self.send(LiveEvent::Error(message));
            }
            Err(error) => {
                self.pairing = None;
                self.send(LiveEvent::PairingStatus(PairingView::Error));
                self.send(LiveEvent::Error(error.to_string()));
            }
        }
    }

    fn handle(&mut self, command: LiveCommand) {
        match command {
            LiveCommand::Shutdown => {}
            LiveCommand::StartPairing => {
                self.forget_session();
                self.start_pairing("Enter this code in the web app");
            }
            LiveCommand::Refresh => {
                if self.client.is_some() {
                    self.refresh_library();
                } else if self.pairing.is_none() {
                    self.start_pairing("Pair this TV to your server");
                }
            }
            LiveCommand::LoadDetails(identifier) => self.load_details(identifier),
            LiveCommand::SaveProgress {
                identifier,
                file_name,
                position_seconds,
                duration_seconds,
            } => {
                if let Some(client) = &self.client {
                    let request = ProgressRequest {
                        file_name,
                        position_seconds,
                        duration_seconds,
                    };
                    if let Err(error) = client.save_progress(&identifier, &request) {
                        self.handle_api_error(error, "could not save progress");
                    }
                }
            }
            LiveCommand::MarkCompleted {
                identifier,
                file_name,
                item_completed,
            } => {
                if let Some(client) = &self.client {
                    let request = CompletedRequest {
                        file_name,
                        item_completed,
                    };
                    if let Err(error) = client.mark_completed(&identifier, &request) {
                        self.handle_api_error(error, "could not mark the title completed");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_bound_to_the_requested_server() {
        let left = ServerTarget::new("a", "One", "http://192.168.1.20:8080").unwrap();
        let same = ServerTarget::new("b", "Two", "http://192.168.1.20:8080/").unwrap();
        let other = ServerTarget::new("c", "Three", "http://192.168.1.21:8080").unwrap();
        assert!(same_server(&left, &same));
        assert!(!same_server(&left, &other));
    }

    #[test]
    fn events_are_drained_non_blocking() {
        let (command_tx, _command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let client = LiveClient {
            commands: command_tx,
            events: event_rx,
            worker: None,
            posters: PosterQueue {
                senders: Vec::new(),
                next: AtomicUsize::new(0),
                workers: Vec::new(),
            },
        };
        event_tx.send(LiveEvent::Status("ready".into())).unwrap();
        let events = client.events();
        assert_eq!(events.len(), 1);
        assert!(format!("{:?}", events[0]).contains("ready"));
        assert!(client.events().is_empty());
    }
}
