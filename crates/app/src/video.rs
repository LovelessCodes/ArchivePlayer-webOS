//! Playback through `com.webos.media` on webOS 5.
//!
//! The app owns an exported SDL video window (created by the simulator before
//! this module starts), so `load` only has to name that window as the sink.
//! Everything below runs on one worker thread: it registers an anonymous LS2
//! client, pumps its own GLib context, and keeps the media service's
//! subscription. The render loop never blocks on the bus; it posts commands and
//! drains events.

use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone, PartialEq)]
pub enum VideoCommand {
    Load { url: String, start_seconds: f64 },
    Play,
    Pause,
    Seek(f64),
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoEvent {
    Loaded,
    Position(f64),
    Playing(bool),
    Ended,
    VideoSize(i64, i64),
    Failed(String),
}

pub struct VideoPlayer {
    commands: Sender<VideoCommand>,
    events: Receiver<VideoEvent>,
}

impl VideoPlayer {
    /// Start the media worker. `window_id` is the id returned by
    /// `SDL_webOSCreateExportedWindow`; it is the only sink binding webOS 5
    /// needs, and it must already exist before the first load.
    pub fn start(window_id: String, app_id: String) -> Self {
        let (commands, inbox) = mpsc::channel();
        let (outbox, events) = mpsc::channel();
        #[cfg(target_arch = "arm")]
        {
            std::thread::Builder::new()
                .name("webos-media".into())
                .spawn(move || arm::run(window_id, app_id, inbox, outbox))
                .expect("media worker thread");
        }
        #[cfg(not(target_arch = "arm"))]
        {
            let _ = (window_id, app_id, inbox, outbox);
        }
        Self { commands, events }
    }

    pub fn load(&self, url: impl Into<String>, start_seconds: f64) {
        let _ = self.commands.send(VideoCommand::Load {
            url: url.into(),
            start_seconds,
        });
    }

    pub fn play(&self) {
        let _ = self.commands.send(VideoCommand::Play);
    }

    pub fn pause(&self) {
        let _ = self.commands.send(VideoCommand::Pause);
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.commands.send(VideoCommand::Seek(seconds));
    }

    pub fn stop(&self) {
        let _ = self.commands.send(VideoCommand::Stop);
    }

    pub fn poll(&self) -> Vec<VideoEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.commands.send(VideoCommand::Stop);
    }
}

#[cfg(target_arch = "arm")]
mod arm {
    use super::{VideoCommand, VideoEvent};
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::ptr;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[repr(C)]
    struct LSError {
        error_code: c_int,
        message: *mut c_char,
        file: *const c_char,
        line: c_int,
        func: *const c_char,
        padding: *mut c_void,
        magic: usize,
    }

    type MessageCallback =
        extern "C" fn(handle: *mut c_void, message: *mut c_void, context: *mut c_void) -> bool;

    #[link(name = "luna-service2")]
    #[link(name = "glib-2.0")]
    extern "C" {
        fn LSErrorInit(error: *mut LSError) -> bool;
        fn LSErrorFree(error: *mut LSError);
        fn LSRegister(name: *const c_char, handle: *mut *mut c_void, error: *mut LSError) -> bool;
        fn LSUnregister(handle: *mut c_void, error: *mut LSError) -> bool;
        fn LSGmainContextAttach(
            handle: *mut c_void,
            context: *mut c_void,
            error: *mut LSError,
        ) -> bool;
        fn LSCall(
            handle: *mut c_void,
            uri: *const c_char,
            payload: *const c_char,
            callback: Option<MessageCallback>,
            context: *mut c_void,
            token: *mut usize,
            error: *mut LSError,
        ) -> bool;
        fn LSCallOneReply(
            handle: *mut c_void,
            uri: *const c_char,
            payload: *const c_char,
            callback: Option<MessageCallback>,
            context: *mut c_void,
            token: *mut usize,
            error: *mut LSError,
        ) -> bool;
        fn LSCallCancel(handle: *mut c_void, token: usize, error: *mut LSError) -> bool;
        fn LSMessageGetPayload(message: *mut c_void) -> *const c_char;
        fn g_main_context_new() -> *mut c_void;
        fn g_main_context_iteration(context: *mut c_void, may_block: c_int) -> c_int;
        fn g_main_context_unref(context: *mut c_void);
    }

    const BASE: &str = "luna://com.webos.media/";
    const PUMP: Duration = Duration::from_millis(20);
    const REPLY_TIMEOUT: Duration = Duration::from_secs(8);

    struct Client {
        handle: *mut c_void,
        context: *mut c_void,
    }

    impl Drop for Client {
        fn drop(&mut self) {
            unsafe {
                let mut error = LSError {
                    error_code: 0,
                    message: ptr::null_mut(),
                    file: ptr::null(),
                    line: 0,
                    func: ptr::null(),
                    padding: ptr::null_mut(),
                    magic: 0,
                };
                LSErrorInit(&mut error);
                LSUnregister(self.handle, &mut error);
                LSErrorFree(&mut error);
                g_main_context_unref(self.context);
            }
        }
    }

    struct ReplySlot(Arc<Mutex<Option<String>>>);

    extern "C" fn on_reply(
        _handle: *mut c_void,
        message: *mut c_void,
        context: *mut c_void,
    ) -> bool {
        unsafe {
            let slot = &*(context as *const ReplySlot);
            if let Ok(mut reply) = slot.0.lock() {
                if reply.is_none() {
                    *reply = payload_text(message);
                }
            }
        }
        true
    }

    extern "C" fn on_event(
        _handle: *mut c_void,
        message: *mut c_void,
        context: *mut c_void,
    ) -> bool {
        unsafe {
            let queue = &*(context as *const Arc<Mutex<VecDeque<String>>>);
            if let Some(text) = payload_text(message) {
                if let Ok(mut queue) = queue.lock() {
                    if queue.len() < 256 {
                        queue.push_back(text);
                    }
                }
            }
        }
        true
    }

    unsafe fn payload_text(message: *mut c_void) -> Option<String> {
        let payload = LSMessageGetPayload(message);
        if payload.is_null() {
            return None;
        }
        Some(CStr::from_ptr(payload).to_string_lossy().into_owned())
    }

    fn new_error() -> LSError {
        let mut error = LSError {
            error_code: 0,
            message: ptr::null_mut(),
            file: ptr::null(),
            line: 0,
            func: ptr::null(),
            padding: ptr::null_mut(),
            magic: 0,
        };
        unsafe {
            LSErrorInit(&mut error);
        }
        error
    }

    fn error_text(error: &mut LSError) -> String {
        unsafe {
            let text = if error.message.is_null() {
                "LS2 error".to_owned()
            } else {
                CStr::from_ptr(error.message).to_string_lossy().into_owned()
            };
            LSErrorFree(error);
            format!("{text} ({})", error.error_code)
        }
    }

    fn register(name: &CStr) -> Result<Client, String> {
        unsafe {
            let mut handle = ptr::null_mut();
            let mut error = new_error();
            // The installer writes an LS2 role for this app whose allowed names are
            // `com.webos.media.client.*`; claiming that name is what grants the
            // outbound media calls. An anonymous handle is not privileged.
            if !LSRegister(name.as_ptr(), &mut handle, &mut error) {
                return Err(error_text(&mut error));
            }
            let context = g_main_context_new();
            if context.is_null() {
                return Err("no GLib context".into());
            }
            let mut error = new_error();
            if !LSGmainContextAttach(handle, context, &mut error) {
                let text = error_text(&mut error);
                LSUnregister(handle, &mut error);
                g_main_context_unref(context);
                return Err(text);
            }
            Ok(Client { handle, context })
        }
    }

    fn pump(context: *mut c_void, rounds: usize) {
        for _ in 0..rounds {
            unsafe {
                if g_main_context_iteration(context, 0) == 0 {
                    break;
                }
            }
        }
    }

    fn call_one(client: &Client, method: &str, payload: &str) -> Result<String, String> {
        let uri = CString::new(format!("{BASE}{method}")).map_err(|error| error.to_string())?;
        let payload = CString::new(payload).map_err(|error| error.to_string())?;
        let slot = Arc::new(Mutex::new(None));
        let context = Box::into_raw(Box::new(ReplySlot(Arc::clone(&slot))));
        let mut token = 0usize;
        let mut error = new_error();
        let started = unsafe {
            LSCallOneReply(
                client.handle,
                uri.as_ptr(),
                payload.as_ptr(),
                Some(on_reply),
                context as *mut c_void,
                &mut token,
                &mut error,
            )
        };
        if !started {
            unsafe {
                drop(Box::from_raw(context));
            }
            let text = error_text(&mut error);
            return Err(format!("{method}: {text}"));
        }
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            pump(client.context, 16);
            if let Ok(reply) = slot.lock() {
                if let Some(text) = reply.clone() {
                    unsafe {
                        drop(Box::from_raw(context));
                    }
                    return Ok(text);
                }
            }
            if Instant::now() >= deadline {
                unsafe {
                    let mut cancel_error = new_error();
                    LSCallCancel(client.handle, token, &mut cancel_error);
                    LSErrorFree(&mut cancel_error);
                    drop(Box::from_raw(context));
                }
                return Err(format!("{method} timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn call_ok(client: &Client, method: &str, payload: &str) -> Result<String, String> {
        let reply = call_one(client, method, payload)?;
        let value: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
        if value.get("returnValue").and_then(Value::as_bool) == Some(false) {
            let text = value
                .get("errorText")
                .and_then(Value::as_str)
                .unwrap_or("media call refused");
            return Err(format!("{method}: {text}"));
        }
        Ok(reply)
    }

    fn seconds_of(value: &Value) -> Option<f64> {
        let number = |value: &Value| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && *number >= 0.0)
        };
        if let Some(ms) = number(value) {
            return Some(ms / 1000.0);
        }
        let inner = value.get("currentTime")?;
        if let Some(ms) = number(inner) {
            return Some(ms / 1000.0);
        }
        let nested = inner.get("currentTime")?;
        number(nested).map(|ms| ms / 1000.0)
    }

    fn decode_event(text: &str, out: &Sender<VideoEvent>) {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            return;
        };
        if let Some(position) = value.get("currentTime").and_then(seconds_of) {
            let _ = out.send(VideoEvent::Position(position));
        }
        if value.get("loadCompleted").is_some() {
            let _ = out.send(VideoEvent::Loaded);
        }
        if let Some(playing) = value.get("playing").and_then(Value::as_bool) {
            let _ = out.send(VideoEvent::Playing(playing));
        }
        if value.get("paused").and_then(Value::as_bool) == Some(true) {
            let _ = out.send(VideoEvent::Playing(false));
        }
        if value.get("endOfStream").and_then(Value::as_bool) == Some(true) {
            let _ = out.send(VideoEvent::Ended);
        }
        if let Some(info) = value.get("videoInfo").and_then(Value::as_object) {
            let width = info.get("width").and_then(Value::as_i64).unwrap_or(0);
            let height = info.get("height").and_then(Value::as_i64).unwrap_or(0);
            if width > 0 && height > 0 {
                let _ = out.send(VideoEvent::VideoSize(width, height));
            }
        }
        if let Some(error) = value.get("error").and_then(Value::as_object) {
            let text = error
                .get("errorText")
                .and_then(Value::as_str)
                .unwrap_or("media error");
            let _ = out.send(VideoEvent::Failed(text.to_owned()));
        }
    }

    pub fn run(
        window_id: String,
        app_id: String,
        inbox: Receiver<VideoCommand>,
        out: Sender<VideoEvent>,
    ) {
        let service_name = CString::new(format!("com.webos.media.client.{app_id}"))
            .expect("app id cannot contain a NUL");
        let client = match register(&service_name) {
            Ok(client) => client,
            Err(error) => {
                let _ = out.send(VideoEvent::Failed(format!("LS2: {error}")));
                return;
            }
        };
        let mut media_id: Option<String> = None;
        let mut subscription: Option<(usize, *mut c_void)> = None;
        let queue: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        loop {
            match inbox.recv_timeout(PUMP) {
                Ok(VideoCommand::Load { url, start_seconds }) => {
                    if let Some((token, context)) = subscription.take() {
                        unsafe {
                            let mut error = new_error();
                            LSCallCancel(client.handle, token, &mut error);
                            LSErrorFree(&mut error);
                            drop(Box::from_raw(context));
                        }
                    }
                    if let Some(id) = media_id.take() {
                        let _ = call_one(&client, "unload", &json!({ "mediaId": id }).to_string());
                    }
                    let payload = json!({
                        "uri": url,
                        "type": "media",
                        "payload": {
                            "option": {
                                "appId": app_id,
                                "windowId": window_id,
                            }
                        }
                    })
                    .to_string();
                    match call_ok(&client, "load", &payload) {
                        Ok(reply) => {
                            let value: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
                            let id = value
                                .get("mediaId")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned();
                            if id.is_empty() {
                                let _ =
                                    out.send(VideoEvent::Failed("load returned no mediaId".into()));
                                continue;
                            }
                            media_id = Some(id.clone());
                            let context = Box::into_raw(Box::new(Arc::clone(&queue)));
                            let mut token = 0usize;
                            let mut error = new_error();
                            let uri = CString::new(format!("{BASE}subscribe")).unwrap();
                            let payload = CString::new(
                                json!({ "mediaId": id, "subscribe": true }).to_string(),
                            )
                            .unwrap();
                            let subscribed = unsafe {
                                LSCall(
                                    client.handle,
                                    uri.as_ptr(),
                                    payload.as_ptr(),
                                    Some(on_event),
                                    context as *mut c_void,
                                    &mut token,
                                    &mut error,
                                )
                            };
                            if subscribed {
                                subscription = Some((token, context as *mut c_void));
                                if start_seconds > 0.5 {
                                    let _ = call_one(
                                        &client,
                                        "seek",
                                        &json!({
                                            "mediaId": id,
                                            "position": (start_seconds * 1000.0).round() as i64,
                                        })
                                        .to_string(),
                                    );
                                }
                                let _ =
                                    call_ok(&client, "play", &json!({ "mediaId": id }).to_string());
                            } else {
                                let _ = out.send(VideoEvent::Failed(error_text(&mut error)));
                                unsafe {
                                    drop(Box::from_raw(context));
                                }
                            }
                        }
                        Err(error) => {
                            let _ = out.send(VideoEvent::Failed(error));
                        }
                    }
                }
                Ok(VideoCommand::Play) => {
                    if let Some(id) = media_id.clone() {
                        if let Err(error) =
                            call_one(&client, "play", &json!({ "mediaId": id }).to_string())
                        {
                            let _ = out.send(VideoEvent::Failed(error));
                        }
                    }
                }
                Ok(VideoCommand::Pause) => {
                    if let Some(id) = media_id.clone() {
                        if let Err(error) =
                            call_one(&client, "pause", &json!({ "mediaId": id }).to_string())
                        {
                            let _ = out.send(VideoEvent::Failed(error));
                        }
                    }
                }
                Ok(VideoCommand::Seek(seconds)) => {
                    if let Some(id) = media_id.clone() {
                        let position = (seconds.max(0.0) * 1000.0).round() as i64;
                        if let Err(error) = call_one(
                            &client,
                            "seek",
                            &json!({ "mediaId": id, "position": position }).to_string(),
                        ) {
                            let _ = out.send(VideoEvent::Failed(error));
                        }
                    }
                }
                Ok(VideoCommand::Stop) => {
                    if let Some((token, context)) = subscription.take() {
                        unsafe {
                            let mut error = new_error();
                            LSCallCancel(client.handle, token, &mut error);
                            LSErrorFree(&mut error);
                            drop(Box::from_raw(context));
                        }
                    }
                    if let Some(id) = media_id.take() {
                        let _ = call_one(&client, "unload", &json!({ "mediaId": id }).to_string());
                    }
                    return;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            pump(client.context, 32);
            if let Ok(mut queue) = queue.lock() {
                while let Some(text) = queue.pop_front() {
                    decode_event(&text, &out);
                }
            }
            while let Ok(command) = inbox.try_recv() {
                // Queue a command for the next iteration without blocking: the
                // only ones that matter between frames are play/pause/seek.
                match command {
                    VideoCommand::Play => {
                        if let Some(id) = media_id.clone() {
                            let _ =
                                call_one(&client, "play", &json!({ "mediaId": id }).to_string());
                        }
                    }
                    VideoCommand::Pause => {
                        if let Some(id) = media_id.clone() {
                            let _ =
                                call_one(&client, "pause", &json!({ "mediaId": id }).to_string());
                        }
                    }
                    VideoCommand::Seek(seconds) => {
                        if let Some(id) = media_id.clone() {
                            let position = (seconds.max(0.0) * 1000.0).round() as i64;
                            let _ = call_one(
                                &client,
                                "seek",
                                &json!({ "mediaId": id, "position": position }).to_string(),
                            );
                        }
                    }
                    VideoCommand::Stop => {
                        if let Some((token, context)) = subscription.take() {
                            unsafe {
                                let mut error = new_error();
                                LSCallCancel(client.handle, token, &mut error);
                                LSErrorFree(&mut error);
                                drop(Box::from_raw(context));
                            }
                        }
                        if let Some(id) = media_id.take() {
                            let _ =
                                call_one(&client, "unload", &json!({ "mediaId": id }).to_string());
                        }
                        return;
                    }
                    VideoCommand::Load { .. } => break,
                }
            }
        }
    }
}
