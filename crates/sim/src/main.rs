//! Host simulator for the native TV UI.
//!
//! It renders the same 1920x1080 canvas as the webOS target and displays it
//! through SDL2. `--all` writes deterministic screenshots without opening a
//! window, which is useful in CI and on headless machines.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use archiveplayer_tv_api::ServerTarget;
use archiveplayer_tv_gfx::Canvas;
use archiveplayer_tv_platform::{
    DiscoveredServer, FileSecretStore, MdnsDiscovery, SecretStore, ServerDiscovery,
};
use archiveplayer_tv_ui::Screen;
use archiveplayer_webos::catalog::Catalog;
use archiveplayer_webos::live::{LiveClient, LiveCommand, LiveEvent, PairingView};
use archiveplayer_webos::pairing::{PairingFlow, PairingPoll};
use archiveplayer_webos::progress::{target_for, ProgressAction, ProgressTracker};
use archiveplayer_webos::render;
use archiveplayer_webos::state::{App, PairingStatus};
use archiveplayer_webos::video::{VideoEvent, VideoPlayer};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{BlendMode, Texture};
use sdl2::video::FullscreenType;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

#[cfg(all(target_arch = "arm", target_os = "linux"))]
mod webos {
    use std::ffi::{c_char, CStr};

    const EXPORTED_TYPE_VIDEO: i32 = 0;

    #[link(name = "wayland-client")]
    extern "C" {
        fn SDL_GetWindowWMInfo(window: *mut std::ffi::c_void, info: *mut std::ffi::c_void) -> i32;
        fn SDL_GetVersion(version: *mut u8);
        fn wl_proxy_marshal(proxy: *mut std::ffi::c_void, opcode: u32, ...);
    }

    extern "C" {
        fn SDL_RaiseWindow(window: *mut std::ffi::c_void);
        fn SDL_SetWindowInputFocus(window: *mut std::ffi::c_void);
        fn SDL_SetWindowGrab(window: *mut std::ffi::c_void, grabbed: i32);
        fn SDL_webOSCreateExportedWindow(kind: i32) -> *const c_char;
        fn SDL_webOSSetExportedWindow(
            window_id: *const c_char,
            source: *const Rect,
            destination: *const Rect,
        ) -> i32;
        fn SDL_webOSDestroyExportedWindow(window_id: *const c_char);
    }

    #[repr(C)]
    pub struct Rect {
        pub x: i32,
        pub y: i32,
        pub w: i32,
        pub h: i32,
    }

    /// Ask the surface manager to route remote-control keys here.
    ///
    /// A freshly launched native app is the foreground app but its card is not
    /// automatically the focused surface: `surface-manager` reports
    /// `Focused none-group nor grabbed item` and drops every key press.
    pub fn focus_window(window: *mut std::ffi::c_void) {
        unsafe {
            SDL_RaiseWindow(window);
            SDL_SetWindowInputFocus(window);
            SDL_SetWindowGrab(window, 1);
        }
    }

    pub fn create_video_window() -> Option<String> {
        let id = unsafe { SDL_webOSCreateExportedWindow(EXPORTED_TYPE_VIDEO) };
        if id.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned())
    }

    pub fn place_video(window_id: &str, width: i32, height: i32) -> bool {
        let Ok(window_id) = std::ffi::CString::new(window_id) else {
            return false;
        };
        let source = Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        };
        let destination = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        unsafe { SDL_webOSSetExportedWindow(window_id.as_ptr(), &source, &destination) == 1 }
    }

    /// Allow the hardware video plane beneath the card window to show through.
    ///
    /// SDL marks its Wayland surface opaque, and the compositor then blends a
    /// black card over the film. Clearing the opaque region is what the UI plane
    /// needs; the commit is left to SDL's next swap. The SDL fork writes a
    /// larger `SDL_SysWMinfo` than its headers declare, so the buffer is
    /// deliberately oversized.
    pub fn clear_opaque_region(window: *mut std::ffi::c_void) -> bool {
        let mut info = [0u8; 512];
        unsafe {
            SDL_GetVersion(info.as_mut_ptr());
            let ok = SDL_GetWindowWMInfo(window, info.as_mut_ptr() as *mut std::ffi::c_void);
            let subsystem = i32::from_ne_bytes(info[4..8].try_into().unwrap());
            if ok == 0 || subsystem != 6 {
                return false;
            }
            let pointers = info.as_ptr().add(8) as *const *mut std::ffi::c_void;
            let surface = pointers.add(1).read_unaligned();
            if surface.is_null() {
                return false;
            }
            // wl_surface.set_opaque_region(NULL) is opcode 4.
            wl_proxy_marshal(surface, 4, std::ptr::null_mut::<std::ffi::c_void>());
            true
        }
    }

    pub fn destroy_video_window(window_id: &str) {
        if let Ok(window_id) = std::ffi::CString::new(window_id) {
            unsafe { SDL_webOSDestroyExportedWindow(window_id.as_ptr()) };
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    if let Some(index) = args.iter().position(|arg| arg == "--pair") {
        let url = args
            .get(index + 1)
            .ok_or_else(|| "--pair needs a server URL".to_string())?;
        let session_path = args
            .iter()
            .position(|arg| arg == "--session")
            .and_then(|position| args.get(position + 1))
            .map(PathBuf::from);
        return pair_in_terminal(url, session_path);
    }

    if args.iter().any(|arg| arg == "--list-servers") {
        let servers = discover_servers(discovery_timeout(&args)?)?;
        print_servers(&servers, &session_path(&args));
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--discover") {
        let servers = discover_servers(discovery_timeout(&args)?)?;
        let path = session_path(&args);
        let server = choose_server(&servers, &args, &path)?;
        print_servers(&servers, &path);
        println!(
            "Connecting to {} at {}",
            server.name,
            display_server(&server.lan_url)
        );
        let target = target_from_server(server)?;
        if let Some((path, screen)) = live_shot_request(&args)? {
            return write_live_screenshot_target(target, session_path(&args), &path, screen);
        }
        return run_live_window_target(target, session_path(&args));
    }

    if let Some(index) = args.iter().position(|arg| arg == "--server") {
        let url = args
            .get(index + 1)
            .ok_or_else(|| "--server needs a URL".to_string())?;
        let session_path = session_path(&args);
        if let Some((path, screen)) = live_shot_request(&args)? {
            return write_live_screenshot(url, session_path, &path, screen);
        }
        return run_live_window(url, session_path);
    }

    if let Some(index) = args.iter().position(|arg| arg == "--all") {
        let directory = args
            .get(index + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("docs/screenshots"));
        return write_all_screenshots(&directory);
    }

    if let Some(index) = args.iter().position(|arg| arg == "--screenshot") {
        let path = args
            .get(index + 1)
            .map(PathBuf::from)
            .ok_or_else(|| "--screenshot needs a path".to_string())?;
        let screen = args
            .iter()
            .position(|arg| arg == "--screen")
            .and_then(|position| args.get(position + 1))
            .map(|name| parse_screen(name))
            .transpose()?
            .unwrap_or(Screen::Home);
        return write_screenshot(&path, screen);
    }

    #[cfg(all(target_arch = "arm", target_os = "linux"))]
    if args.is_empty() {
        return run_tv_default();
    }

    run_window()
}

fn print_help() {
    println!("ArchivePlayer webOS host simulator");
    println!();
    println!("  cargo run -p archiveplayer-webos-sim");
    println!("  cargo run -p archiveplayer-webos-sim -- --screenshot out.png --screen home");
    println!("  cargo run -p archiveplayer-webos-sim -- --all docs/screenshots");
    println!("  cargo run -p archiveplayer-webos-sim -- --list-servers");
    println!("  cargo run -p archiveplayer-webos-sim -- --discover");
    println!("  cargo run -p archiveplayer-webos-sim -- --discover --name \"Living room\"");
    println!("  cargo run -p archiveplayer-webos-sim -- --server http://192.168.1.20:8080");
    println!("  cargo run -p archiveplayer-webos-sim -- --server URL --live-shot home.png");
    println!("  cargo run -p archiveplayer-webos-sim -- --pair http://192.168.1.20:8080");
    println!();
    println!("Keys: arrows move, Enter activates, Esc goes back, Q quits.");
    println!("      Space toggles playback on the player screen.");
}

fn parse_screen(name: &str) -> Result<Screen, String> {
    match name {
        "home" => Ok(Screen::Home),
        "library" => Ok(Screen::Library),
        "search" => Ok(Screen::Search),
        "detail" | "details" => Ok(Screen::Detail),
        "settings" => Ok(Screen::Settings),
        "pair" | "pairing" => Ok(Screen::Pair),
        "player" => Ok(Screen::Player),
        other => Err(format!("unknown screen: {other}")),
    }
}

fn app_for_screen(screen: Screen) -> App {
    let mut app = App::demo();
    app.screen = screen;
    match screen {
        Screen::Home => {
            app.focus_index = 4; // hero Play
        }
        Screen::Library => {
            app.focus_index = 10; // first poster after nav + chips
        }
        Screen::Search => {
            app.search_query = "noir".into();
            app.focus_index = 9; // first result after nav + input + chips
        }
        Screen::Detail => {
            app.selected = 0;
            app.focus_index = 1; // Play
        }
        Screen::Settings => {
            app.focus_index = 4; // Pair this TV
        }
        Screen::Pair => {
            app.focus_index = 0; // Simulate approval
        }
        Screen::Player => {
            app.selected = 0;
            app.player_position = 4820.0;
            app.player_duration = 5760.0;
            app.player_playing = true;
            app.focus_index = 2; // Play/pause
        }
    }
    app
}

fn pair_in_terminal(url: &str, session_path: Option<PathBuf>) -> Result<(), String> {
    let target =
        ServerTarget::new("lan", "ArchivePlayer", url).map_err(|error| error.to_string())?;
    let flow = PairingFlow::start(&target, "ArchivePlayer TV", "webOS TV")
        .map_err(|error| error.to_string())?;
    println!("Server:   {}", flow.target().lan_url);
    println!("Code:     {}", flow.user_code());
    println!("Approve:  {}", flow.verification_uri_complete());
    println!("Waiting for approval…");

    let deadline = Instant::now() + Duration::from_secs(flow.expires_in_seconds().max(1) as u64);
    loop {
        if Instant::now() >= deadline {
            return Err("pairing code expired".into());
        }
        match flow.poll().map_err(|error| error.to_string())? {
            PairingPoll::Pending => {
                std::thread::sleep(Duration::from_secs(flow.interval_seconds().max(1) as u64));
            }
            PairingPoll::SlowDown {
                retry_after_seconds,
            } => {
                std::thread::sleep(Duration::from_secs(retry_after_seconds.max(1)));
            }
            PairingPoll::Denied(message) => return Err(format!("approval denied: {message}")),
            PairingPoll::Expired(message) => return Err(format!("pairing expired: {message}")),
            PairingPoll::Approved(session) => {
                let path = session_path.unwrap_or_else(default_session_path);
                FileSecretStore::new(&path)
                    .save(&session)
                    .map_err(|error| error.to_string())?;
                println!("Paired as {}", session.user.email);
                println!("Session:  {}", path.display());
                return Ok(());
            }
        }
    }
}

fn default_session_path() -> PathBuf {
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            if directory.join("appinfo.json").is_file() {
                return directory.join("session.json");
            }
        }
    }
    if let Some(directory) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(directory)
            .join("archiveplayer-webos")
            .join("session.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/archiveplayer-webos/session.json");
    }
    PathBuf::from("archiveplayer-webos-session.json")
}

fn write_all_screenshots(directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let screens = [
        ("home", Screen::Home),
        ("library", Screen::Library),
        ("detail", Screen::Detail),
        ("search", Screen::Search),
        ("settings", Screen::Settings),
        ("pair", Screen::Pair),
        ("player", Screen::Player),
    ];
    for (name, screen) in screens {
        let path = directory.join(format!("{name}.png"));
        write_screenshot(&path, screen)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn write_screenshot(path: &Path, screen: Screen) -> Result<(), String> {
    let app = app_for_screen(screen);
    let mut canvas = Canvas::new(WIDTH, HEIGHT)?;
    render::render(&app, &mut canvas);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }
    canvas.save_png(path)?;
    Ok(())
}

fn write_live_screenshot(
    url: &str,
    session_path: PathBuf,
    path: &Path,
    screen: Screen,
) -> Result<(), String> {
    let target =
        ServerTarget::new("lan", "ArchivePlayer", url).map_err(|error| error.to_string())?;
    write_live_screenshot_target(target, session_path, path, screen)
}

fn write_live_screenshot_target(
    target: ServerTarget,
    session_path: PathBuf,
    path: &Path,
    screen: Screen,
) -> Result<(), String> {
    let mut app = App::live(target.name.clone(), target.lan_url.clone());
    let live = LiveClient::start(target, session_path);
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut loaded_details = false;
    let mut library_seen: Option<Instant> = None;
    let mut search_started = false;

    while Instant::now() < deadline {
        for event in live.events() {
            if let LiveEvent::Library(response) = &event {
                queue_artwork(&live, response, &app.library);
            }
            apply_live_event(&mut app, event, &None);
        }
        if screen != Screen::Pair && !app.library.items.is_empty() {
            library_seen.get_or_insert_with(Instant::now);
            if !loaded_details && matches!(screen, Screen::Detail | Screen::Player) {
                let identifier = app.library.item(0).identifier.clone();
                live.send(LiveCommand::LoadDetails(identifier));
                loaded_details = true;
            }
            if screen == Screen::Detail && !app.library.item(0).description.is_empty() {
                app.selected = 0;
                app.set_screen(Screen::Detail);
                app.focus_index = 1;
                let settled =
                    library_seen.is_some_and(|seen| seen.elapsed() > Duration::from_secs(4));
                if !app.library.item(0).episodes.is_empty()
                    || app.library.item(0).backdrop.is_some()
                    || settled
                {
                    break;
                }
            } else if screen == Screen::Home
                && app.screen == Screen::Home
                && ((app.library.items.iter().any(|item| item.poster.is_some())
                    && app.library.items.iter().any(|item| item.backdrop.is_some()))
                    || library_seen.is_some_and(|seen| seen.elapsed() > Duration::from_secs(4)))
            {
                break;
            } else if screen == Screen::Search && app.screen != Screen::Search {
                app.set_screen(Screen::Search);
                app.search_query = "night".into();
                if app.start_search() {
                    live.send(LiveCommand::Search {
                        query: app.archive_query.clone(),
                        page: 1,
                    });
                }
                search_started = true;
            }
            if screen == Screen::Search
                && search_started
                && !matches!(
                    app.search_status,
                    archiveplayer_webos::state::SearchStatus::Loading
                )
            {
                break;
            }
        }
        if screen == Screen::Pair && !app.pairing_code.is_empty() {
            break;
        }
        app.tick(0.016);
        std::thread::sleep(Duration::from_millis(50));
    }

    if screen == Screen::Detail {
        app.selected = 0;
        app.set_screen(Screen::Detail);
        app.focus_index = 1;
    } else if screen == Screen::Library {
        app.set_screen(Screen::Library);
        app.focus_index = 4;
    } else if screen == Screen::Search {
        app.set_screen(Screen::Search);
        app.focus_index = 9;
    } else if screen == Screen::Settings {
        app.set_screen(Screen::Settings);
        app.focus_index = 4;
    } else if screen == Screen::Player {
        app.selected = 0;
        app.set_screen(Screen::Player);
        app.focus_index = 2;
    }

    let mut canvas = Canvas::new(WIDTH, HEIGHT)?;
    render::render(&app, &mut canvas);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }
    canvas.save_png(path)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn session_path(args: &[String]) -> PathBuf {
    arg_value(args, "--session")
        .map(PathBuf::from)
        .unwrap_or_else(default_session_path)
}

fn discovery_timeout(args: &[String]) -> Result<Duration, String> {
    let Some(raw) = arg_value(args, "--discover-timeout") else {
        return Ok(Duration::from_secs(3));
    };
    let seconds = raw
        .parse::<u64>()
        .map_err(|_| "--discover-timeout needs a whole number of seconds".to_string())?;
    if !(1..=30).contains(&seconds) {
        return Err("--discover-timeout must be between 1 and 30 seconds".into());
    }
    Ok(Duration::from_secs(seconds))
}

fn live_shot_request(args: &[String]) -> Result<Option<(PathBuf, Screen)>, String> {
    let Some(path) = arg_value(args, "--live-shot") else {
        return Ok(None);
    };
    let screen = arg_value(args, "--live-screen")
        .map(parse_screen)
        .transpose()?
        .unwrap_or(Screen::Home);
    Ok(Some((PathBuf::from(path), screen)))
}

fn discover_servers(timeout: Duration) -> Result<Vec<DiscoveredServer>, String> {
    println!(
        "Browsing _archiveplayer._tcp.local for {} second(s)…",
        timeout.as_secs()
    );
    MdnsDiscovery
        .browse(timeout)
        .map_err(|error| error.to_string())
}

fn target_from_server(server: &DiscoveredServer) -> Result<ServerTarget, String> {
    let mut target = ServerTarget::new(&server.id, &server.name, &server.lan_url)
        .map_err(|error| error.to_string())?;
    if let Some(remote) = &server.remote_url {
        target = target
            .with_remote(remote)
            .map_err(|error| error.to_string())?;
    }
    target.api_version = server.api_version;
    target.server_version = server.version.clone();
    Ok(target)
}

fn choose_server<'a>(
    servers: &'a [DiscoveredServer],
    args: &[String],
    session_path: &Path,
) -> Result<&'a DiscoveredServer, String> {
    if servers.is_empty() {
        return Err(
            "No ArchivePlayer servers found on this network. Check that the server is bound to 0.0.0.0, ARCHIVEPLAYER_MDNS is enabled, and the TV and server are on the same LAN."
                .into(),
        );
    }

    let name = arg_value(args, "--name").map(str::to_ascii_lowercase);
    let server_id = arg_value(args, "--server-id");
    let filtered: Vec<&DiscoveredServer> = servers
        .iter()
        .filter(|server| {
            name.as_ref().is_none_or(|name| {
                server.name.to_ascii_lowercase().contains(name)
                    || server.id.to_ascii_lowercase().contains(name)
            })
        })
        .filter(|server| {
            server_id.is_none_or(|id| {
                server.id == id
                    || (id.len() >= 8 && server.id.starts_with(&id.to_ascii_lowercase()))
            })
        })
        .collect();

    if filtered.is_empty() {
        return Err(format!(
            "No discovered server matches {}. Found: {}",
            name.or_else(|| server_id.map(str::to_string))
                .unwrap_or_else(|| "the filter".into()),
            servers
                .iter()
                .map(|server| server.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if filtered.len() == 1 {
        return Ok(filtered[0]);
    }

    let saved_id = FileSecretStore::new(session_path)
        .load()
        .ok()
        .flatten()
        .map(|session| session.target.id);
    if let Some(id) = saved_id {
        if let Some(server) = filtered
            .iter()
            .find(|server| server.id == id && id.len() >= 12)
        {
            return Ok(server);
        }
    }

    Err(format!(
        "Found {} servers: {}. Choose one with --name <text> or --server-id <id>.",
        filtered.len(),
        filtered
            .iter()
            .map(|server| server.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn print_servers(servers: &[DiscoveredServer], session_path: &Path) {
    if servers.is_empty() {
        println!("No ArchivePlayer servers found.");
        return;
    }
    let paired_id = FileSecretStore::new(session_path)
        .load()
        .ok()
        .flatten()
        .map(|session| session.target.id);
    println!("{} server(s) found:", servers.len());
    for server in servers {
        let paired = paired_id
            .as_deref()
            .is_some_and(|id| id == server.id && id.len() >= 12);
        println!(
            "  {} ({}): {}{}",
            server.name,
            server.id,
            display_server(&server.lan_url),
            if paired { "  [paired]" } else { "" }
        );
        if let Some(remote) = &server.remote_url {
            println!("    remote: {remote}");
        }
        let version = server.version.as_deref().unwrap_or("unknown");
        println!("    API {} · server {version}", server.api_version);
    }
}

fn display_server(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

#[cfg(all(target_arch = "arm", target_os = "linux"))]
fn run_tv_default() -> Result<(), String> {
    let path = default_session_path();
    println!("Looking for ArchivePlayer servers on this network…");
    let servers = match discover_servers(Duration::from_secs(5)) {
        Ok(servers) => servers,
        Err(error) => {
            eprintln!("Discovery failed: {error}");
            return run_window();
        }
    };
    if servers.is_empty() {
        eprintln!("No ArchivePlayer server answered mDNS; showing the demo library.");
        return run_window();
    }
    let server = preferred_server(&servers, &path)
        .ok_or_else(|| "discovery returned no usable server".to_string())?;
    println!(
        "Connecting to {} at {}",
        server.name,
        display_server(&server.lan_url)
    );
    let target = target_from_server(server)?;
    run_live_window_target(target, path)
}

/// Pick the saved server when it is still advertised, otherwise the first by
/// name. A TV has no keyboard at startup, so a multi-server LAN must not turn
/// into an unanswerable prompt.
#[cfg(all(target_arch = "arm", target_os = "linux"))]
fn preferred_server<'a>(
    servers: &'a [DiscoveredServer],
    session_path: &Path,
) -> Option<&'a DiscoveredServer> {
    if let Ok(Some(session)) = FileSecretStore::new(session_path).load() {
        if session.target.id.len() >= 12 {
            if let Some(server) = servers.iter().find(|server| server.id == session.target.id) {
                return Some(server);
            }
        }
    }
    servers
        .iter()
        .min_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)))
}

fn run_window() -> Result<(), String> {
    run_window_app(App::demo(), None)
}

fn run_live_window(url: &str, session_path: PathBuf) -> Result<(), String> {
    let target =
        ServerTarget::new("lan", "ArchivePlayer", url).map_err(|error| error.to_string())?;
    run_live_window_target(target, session_path)
}

fn run_live_window_target(target: ServerTarget, session_path: PathBuf) -> Result<(), String> {
    let app = App::live(target.name.clone(), target.lan_url.clone());
    let live = LiveClient::start(target, session_path);
    run_window_app(app, Some(live))
}

/// Translate the raw scancode webOS's SDL fork writes into `keycode`.
///
/// `SDL_GetKeyFromScancode` is not reliable on this fork because the event
/// field already holds the scancode; the common remote keys are mapped
/// explicitly and everything else falls back to the library's own translation.
#[cfg(all(target_arch = "arm", target_os = "linux"))]
fn normalize_remote_key(key: Keycode) -> Keycode {
    match key.into_i32() {
        20 => Keycode::Q,
        40 => Keycode::Return,
        41 => Keycode::Escape,
        42 => Keycode::Backspace,
        44 => Keycode::Space,
        79 => Keycode::Right,
        80 => Keycode::Left,
        81 => Keycode::Down,
        82 => Keycode::Up,
        95 => Keycode::F11,
        96 => Keycode::F12,
        code => sdl2::keyboard::Scancode::from_i32(code)
            .and_then(Keycode::from_scancode)
            .unwrap_or(key),
    }
}

fn run_window_app(mut app: App, live: Option<LiveClient>) -> Result<(), String> {
    // LG's SDL fork reads this environment variable (not the SDL hint) before
    // SDL_Init and then delivers the remote's Back key instead of handing it to
    // the system launcher. Other SDL builds ignore it.
    std::env::set_var("SDL_WEBOS_ACCESS_POLICY_KEYS_BACK", "true");
    sdl2::hint::set("SDL_WEBOS_ACCESS_POLICY_KEYS_BACK", "1");
    // The card window is the UI plane above the hardware video plane. Its EGL
    // surface must carry alpha or the compositor sees an opaque black card and
    // hides the film behind it.
    sdl2::hint::set("SDL_VIDEO_EGL_ALLOW_TRANSPARENCY", "1");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    #[allow(unused_mut)]
    let mut window = video
        .window("ArchivePlayer — webOS native preview", 1920, 1080)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|error| error.to_string())?;
    #[cfg(all(target_arch = "arm", target_os = "linux"))]
    {
        if let Err(error) = window.set_fullscreen(FullscreenType::Desktop) {
            eprintln!("fullscreen request failed: {error}");
        }
        webos::focus_window(window.raw() as *mut std::ffi::c_void);
    }
    let mut sdl_canvas = window
        .into_canvas()
        .build()
        .map_err(|error| error.to_string())?;
    // Window, renderer output, and texture are all 1920x1080 when the window
    // type is fullscreen. A logical size equal to the output makes webOS 5's SDL
    // 2.0.10 letterbox a 1280x720 crop, so leave it unset.
    #[cfg(all(target_arch = "arm", target_os = "linux"))]
    let _ = webos::clear_opaque_region(sdl_canvas.window().raw() as *mut std::ffi::c_void);
    let texture_creator = sdl_canvas.texture_creator();
    // `Canvas` stores straight RGBA bytes. `ABGR8888` is the SDL name for
    // that byte order (`A` in the high bit, `R` in the low byte); the more
    // obvious `RGBA8888` is the opposite order and turned the whole UI red on
    // webOS 5 by reading alpha as red.
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::ABGR8888, WIDTH, HEIGHT)
        .map_err(|error| error.to_string())?;
    texture.set_blend_mode(BlendMode::None);
    let mut event_pump = sdl.event_pump()?;
    let text_input = video.text_input();

    #[allow(unused_mut)]
    let mut video_player: Option<VideoPlayer> = None;
    #[allow(unused_mut, unused_variables)]
    let mut video_window: Option<String> = None;
    #[cfg(all(target_arch = "arm", target_os = "linux"))]
    {
        let window_id = webos::create_video_window().unwrap_or_default();
        if !window_id.is_empty() {
            let _ = webos::place_video(&window_id, WIDTH as i32, HEIGHT as i32);
            video_player = Some(VideoPlayer::start(
                window_id.clone(),
                "codes.loveless.archiveplayer".into(),
            ));
            video_window = Some(window_id);
        }
    }

    let mut frame_canvas = Canvas::new(WIDTH, HEIGHT)?;
    let mut last = Instant::now();
    let mut was_editing = false;
    let mut progress = ProgressTracker::new();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    if let Some(live) = &live {
                        flush_progress(&mut progress, &app, live);
                    }
                    break 'running;
                }
                Event::KeyDown {
                    keycode: Some(key), ..
                } => {
                    // webOS's SDL fork puts the raw scancode in `keycode`
                    // (Right is 79, Return 40) and leaves `scancode` empty.
                    // Translate those back to ordinary SDLK values; the
                    // private Back keycode (482) and normal host keycodes do
                    // not resolve as scancodes and pass through unchanged.
                    #[cfg(all(target_arch = "arm", target_os = "linux"))]
                    let key = normalize_remote_key(key);
                    match key {
                        Keycode::Q if !app.search_editing => {
                            if let Some(live) = &live {
                                flush_progress(&mut progress, &app, live);
                            }
                            break 'running;
                        }
                        // LG's SDL fork reports the remote's Back key as a private
                        // keycode rather than SDLK_AC_BACK; 482 is what webOS 5
                        // sends, 461 is the value seen on older builds.
                        key if key == Keycode::Escape
                            || key.into_i32() == 482
                            || key.into_i32() == 461 =>
                        {
                            if let Some(live) = &live {
                                flush_progress(&mut progress, &app, live);
                            }
                            app.back();
                        }
                        Keycode::Backspace => {
                            if app.search_editing {
                                app.on_backspace();
                            } else {
                                if let Some(live) = &live {
                                    flush_progress(&mut progress, &app, live);
                                }
                                app.back();
                            }
                        }
                        Keycode::Up => app.move_focus(archiveplayer_tv_ui::FocusMove::Up),
                        Keycode::Down => app.move_focus(archiveplayer_tv_ui::FocusMove::Down),
                        Keycode::Left => app.move_focus(archiveplayer_tv_ui::FocusMove::Left),
                        Keycode::Right => app.move_focus(archiveplayer_tv_ui::FocusMove::Right),
                        Keycode::Return | Keycode::KpEnter => {
                            if let Some(live) = &live {
                                flush_progress(&mut progress, &app, live);
                            }
                            let action = app.activate();
                            if matches!(action, archiveplayer_webos::state::Action::Play(_)) {
                                progress.reset();
                            }
                            if let Some(live) = &live {
                                dispatch_action(live, &app, action);
                            }
                            act_on_video(&mut app, &video_player, action);
                        }
                        Keycode::Space if !app.search_editing => {
                            if let Some(live) = &live {
                                flush_progress(&mut progress, &app, live);
                            }
                            let action = app.activate();
                            if matches!(action, archiveplayer_webos::state::Action::Play(_)) {
                                progress.reset();
                            }
                            if let Some(live) = &live {
                                dispatch_action(live, &app, action);
                            }
                            act_on_video(&mut app, &video_player, action);
                        }
                        Keycode::F11 => {
                            let fullscreen = sdl_canvas.window().fullscreen_state();
                            if fullscreen == FullscreenType::Off {
                                sdl_canvas
                                    .window_mut()
                                    .set_fullscreen(FullscreenType::Desktop)
                                    .map_err(|error| error.to_string())?;
                            } else {
                                sdl_canvas
                                    .window_mut()
                                    .set_fullscreen(FullscreenType::Off)
                                    .map_err(|error| error.to_string())?;
                            }
                        }
                        Keycode::F12 => {
                            let path = PathBuf::from("screenshot.png");
                            let mut shot = Canvas::new(WIDTH, HEIGHT)?;
                            render::render(&app, &mut shot);
                            shot.save_png(&path)?;
                            println!("wrote {}", path.display());
                        }
                        _ => {}
                    }
                }
                Event::TextInput { text, .. } => app.on_text(&text),
                _ => {}
            }
        }

        if app.search_editing != was_editing {
            if app.search_editing {
                text_input.start();
            } else {
                text_input.stop();
            }
            was_editing = app.search_editing;
        }

        if let Some(live) = &live {
            for event in live.events() {
                if let LiveEvent::Library(response) = &event {
                    queue_artwork(live, response, &app.library);
                }
                apply_live_event(&mut app, event, &video_player);
            }
        }

        if let Some(player) = &video_player {
            for event in player.poll() {
                match event {
                    VideoEvent::Loaded => {}
                    VideoEvent::Position(position) => {
                        app.player_position = position.clamp(0.0, app.player_duration.max(1.0));
                    }
                    VideoEvent::Playing(playing) => app.player_playing = playing,
                    VideoEvent::Ended => {
                        app.player_playing = false;
                        app.player_position = app.player_duration;
                    }
                    VideoEvent::VideoSize(width, height) => {
                        #[cfg(all(target_arch = "arm", target_os = "linux"))]
                        if let Some(window_id) = &video_window {
                            let _ = webos::place_video(window_id, width as i32, height as i32);
                        }
                        #[cfg(not(all(target_arch = "arm", target_os = "linux")))]
                        let _ = (width, height);
                    }
                    VideoEvent::Failed(message) => {
                        app.video_active = false;
                        app.player_playing = false;
                        app.show_toast(format!("Playback failed: {message}"));
                    }
                }
            }
        }
        if app.screen != Screen::Player && app.video_active {
            if let Some(player) = &video_player {
                player.stop();
            }
            app.video_active = false;
        }

        let now = Instant::now();
        let delta = now.duration_since(last).as_secs_f64().min(0.5);
        last = now;
        app.tick(delta);
        if let Some(live) = &live {
            observe_progress(&mut progress, &app, live);
        }
        render::render(&app, &mut frame_canvas);
        upload_texture(&mut texture, &frame_canvas)?;
        sdl_canvas.clear();
        sdl_canvas
            .copy(&texture, None, None)
            .map_err(|error| error.to_string())?;
        #[cfg(all(target_arch = "arm", target_os = "linux"))]
        if app.video_active {
            // SDL's swap re-asserts the surface's opaque region on this firmware;
            // clear it on every frame that must show the video plane beneath.
            let _ = webos::clear_opaque_region(sdl_canvas.window().raw() as *mut std::ffi::c_void);
        }
        sdl_canvas.present();
        #[cfg(all(target_arch = "arm", target_os = "linux"))]
        if app.frame == 1 || app.frame == 60 {
            // The surface is mapped by the first present; asking before that
            // is accepted by SDL but the compositor has no item to focus yet.
            webos::focus_window(sdl_canvas.window().raw() as *mut std::ffi::c_void);
        }
        std::thread::sleep(Duration::from_millis(16));
    }

    if let Some(player) = &video_player {
        player.stop();
    }
    drop(video_player);
    #[cfg(all(target_arch = "arm", target_os = "linux"))]
    if let Some(window_id) = &video_window {
        webos::destroy_video_window(window_id);
    }

    Ok(())
}

/// Start the media pipeline for the selected title when the server supplied a
/// compatible plan. The player's fake clock stays off while it runs.
fn start_video(app: &mut App, video_player: &Option<VideoPlayer>) {
    if app.video_active || app.screen != Screen::Player {
        return;
    }
    let Some(player) = video_player else {
        return;
    };
    let Some(plan) = app
        .library
        .item(app.selected)
        .playback_plan
        .as_ref()
        .filter(|plan| plan.compatible)
        .filter(|plan| !plan.url.is_empty())
    else {
        return;
    };
    if let Some(length) = plan.length_seconds.filter(|length| *length > 0.0) {
        app.player_duration = length;
        app.player_position = app.player_position.clamp(0.0, length);
    }
    player.load(plan.url.clone(), app.player_position);
    app.video_active = true;
    app.player_playing = true;
}

/// Mirror an action that already changed the fake state onto the real pipeline.
fn act_on_video(
    app: &mut App,
    video_player: &Option<VideoPlayer>,
    action: archiveplayer_webos::state::Action,
) {
    use archiveplayer_webos::state::Action;
    let Some(player) = video_player else {
        return;
    };
    match action {
        Action::Play(_) => start_video(app, video_player),
        Action::TogglePlay if app.video_active => {
            if app.player_playing {
                player.play();
            } else {
                player.pause();
            }
        }
        Action::Seek(_) if app.video_active => player.seek(app.player_position),
        Action::Back if app.video_active => {
            player.stop();
            app.video_active = false;
        }
        _ => {}
    }
}

fn queue_artwork(
    live: &LiveClient,
    response: &archiveplayer_tv_protocol::LibraryResponse,
    catalog: &Catalog,
) {
    let mut seen = HashSet::new();
    for group in &response.categories {
        for item in &group.items {
            if !seen.insert(item.identifier.clone()) {
                continue;
            }
            let existing = catalog.index_of(&item.identifier).map(|index| {
                let item = catalog.item(index);
                (item.poster.is_some(), item.backdrop.is_some())
            });
            let (has_poster, has_backdrop) = existing.unwrap_or((false, false));
            if !has_poster {
                if let Some(url) = item
                    .tmdb_poster_url
                    .clone()
                    .or_else(|| item.thumbnail_url.clone())
                {
                    live.load_poster(item.identifier.clone(), url);
                }
            }
            if !has_backdrop {
                if let Some(url) = item.tmdb_backdrop_url.clone() {
                    live.load_backdrop(item.identifier.clone(), url);
                }
            }
        }
    }
}

fn observe_progress(tracker: &mut ProgressTracker, app: &App, live: &LiveClient) {
    if !app.paired || app.screen != Screen::Player {
        return;
    }
    let Some(target) = target_for(app.library.item(app.selected)) else {
        return;
    };
    let action = tracker.observe(
        Some(&target),
        app.player_position.round() as i64,
        app.player_duration.round() as i64,
        app.player_playing,
        Instant::now(),
    );
    if let Some(action) = action {
        send_progress(live, action);
    }
}

fn flush_progress(tracker: &mut ProgressTracker, app: &App, live: &LiveClient) {
    if !app.paired || app.screen != Screen::Player {
        return;
    }
    let Some(target) = target_for(app.library.item(app.selected)) else {
        return;
    };
    let action = tracker.flush(
        &target,
        app.player_position.round() as i64,
        app.player_duration.round() as i64,
        Instant::now(),
    );
    if let Some(action) = action {
        send_progress(live, action);
    }
}

fn send_progress(live: &LiveClient, action: ProgressAction) {
    match action {
        ProgressAction::Save {
            target,
            position_seconds,
            duration_seconds,
        } => live.send(LiveCommand::SaveProgress {
            identifier: target.identifier,
            file_name: target.file_name,
            position_seconds,
            duration_seconds,
        }),
        ProgressAction::Completed { target } => live.send(LiveCommand::MarkCompleted {
            identifier: target.identifier,
            file_name: target.file_name,
            item_completed: target.item_completed,
        }),
    }
}

fn dispatch_action(live: &LiveClient, app: &App, action: archiveplayer_webos::state::Action) {
    use archiveplayer_webos::state::Action;
    match action {
        Action::Open(index) | Action::Play(index) => {
            let identifier = app.library.item(index).identifier.clone();
            if !identifier.is_empty() {
                live.send(LiveCommand::LoadDetails(identifier));
            }
        }
        Action::SubmitSearch => live.send(LiveCommand::Search {
            query: app.archive_query.clone(),
            page: 1,
        }),
        Action::AddResult(index) => {
            if let Some(card) = app.archive_card(index) {
                live.send(LiveCommand::AddToLibrary {
                    identifier: card.identifier.clone(),
                    category_name: None,
                });
            }
        }
        Action::Pair | Action::PairRefresh => live.send(LiveCommand::StartPairing),
        Action::Reconnect => live.send(LiveCommand::Refresh),
        _ => {}
    }
}

fn apply_live_event(app: &mut App, event: LiveEvent, video_player: &Option<VideoPlayer>) {
    match event {
        LiveEvent::Status(message) => app.show_toast(message),
        LiveEvent::Connected {
            name,
            url,
            version,
            lan,
        } => {
            app.set_connection(name, url, version, lan, true);
            app.set_pairing_status(PairingStatus::Approved);
        }
        LiveEvent::PairingRequired(reason) => {
            let name = app.server_name.clone();
            let url = app.server_url.clone();
            app.set_connection(name, url, String::new(), true, false);
            app.set_pairing_status(PairingStatus::Waiting);
            app.reset_search_state();
            app.show_toast(reason);
        }
        LiveEvent::PairingCode {
            code,
            uri,
            complete_uri,
            expires_in_seconds,
        } => app.set_pairing_code(code, uri, complete_uri, expires_in_seconds),
        LiveEvent::PairingStatus(status) => {
            let status = match status {
                PairingView::Connecting => PairingStatus::Connecting,
                PairingView::Waiting => PairingStatus::Waiting,
                PairingView::Approved => PairingStatus::Approved,
                PairingView::Denied => PairingStatus::Denied,
                PairingView::Expired => PairingStatus::Expired,
                PairingView::Error => PairingStatus::Error,
            };
            app.set_pairing_status(status);
        }
        LiveEvent::Library(response) => {
            let catalog = Catalog::from_library(&response, &app.library);
            let count = catalog.items.len();
            app.set_catalog(catalog);
            app.show_toast(format!("Loaded {count} titles"));
        }
        LiveEvent::SearchResults { query, response } => {
            app.apply_archive_results(&query, *response);
        }
        LiveEvent::SearchFailed { query, message } => {
            app.fail_archive_search(&query, message);
        }
        LiveEvent::Added { identifier, title } => {
            app.finish_add(&identifier);
            app.show_toast(format!("Added {title}"));
        }
        LiveEvent::AddFailed {
            identifier,
            message,
        } => {
            app.finish_add(&identifier);
            app.show_toast(message);
        }
        LiveEvent::Details(details) => {
            let incompatible = details
                .playback_plan
                .as_ref()
                .filter(|plan| !plan.compatible)
                .map(|plan| {
                    plan.reason
                        .clone()
                        .unwrap_or_else(|| "This file is not playable on this TV.".into())
                });
            if app.library.apply_details(&details) {
                app.show_toast(format!("Loaded {}", details.title));
            }
            if app.screen == Screen::Player {
                if let Some(seconds) = details
                    .playback_plan
                    .as_ref()
                    .and_then(|plan| plan.length_seconds)
                    .filter(|seconds| *seconds > 0.0)
                {
                    app.player_duration = seconds.max(1.0);
                    app.player_position = app.player_position.min(app.player_duration);
                }
            }
            if let Some(reason) = incompatible {
                if app.screen == Screen::Player {
                    app.screen = Screen::Detail;
                    app.focus_index = 1;
                }
                app.show_toast(reason);
            } else if app.screen == Screen::Player {
                start_video(app, video_player);
            }
        }
        LiveEvent::Poster { identifier, image } => {
            app.library.set_poster(&identifier, image);
        }
        LiveEvent::Backdrop { identifier, image } => {
            app.library.set_backdrop(&identifier, image);
        }
        LiveEvent::Error(message) => app.show_toast(message),
    }
}

fn upload_texture(texture: &mut Texture, canvas: &Canvas) -> Result<(), String> {
    let pitch = (canvas.width() * 4) as usize;
    texture
        .update(None, canvas.pixels(), pitch)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use archiveplayer_tv_api::StoredSession;
    use archiveplayer_tv_protocol::User;

    fn server(name: &str, id: &str, url: &str) -> DiscoveredServer {
        DiscoveredServer {
            id: id.into(),
            name: name.into(),
            lan_url: url.into(),
            remote_url: None,
            api_version: 1,
            version: Some("0.5.0".into()),
        }
    }

    #[test]
    fn one_discovered_server_is_selected_without_a_prompt() {
        let path = Path::new("/does/not/exist/session.json");
        let servers = vec![server(
            "Living room",
            &"a".repeat(64),
            "http://192.168.1.44:8080/",
        )];
        let chosen = choose_server(&servers, &["--discover".into()], path).unwrap();
        assert_eq!(chosen.name, "Living room");
    }

    #[test]
    fn multiple_servers_need_a_name_or_id() {
        let path = Path::new("/does/not/exist/session.json");
        let servers = vec![
            server("Living room", &"a".repeat(64), "http://192.168.1.44:8080/"),
            server("Attic", &"b".repeat(64), "http://192.168.1.45:8080/"),
        ];
        assert!(choose_server(&servers, &["--discover".into()], path).is_err());

        let args = ["--discover".into(), "--name".into(), "attic".into()];
        let chosen = choose_server(&servers, &args, path).unwrap();
        assert_eq!(chosen.name, "Attic");
    }

    #[test]
    fn a_saved_stable_id_breaks_a_tie() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let id = "a".repeat(64);
        let store = FileSecretStore::new(&path);
        store
            .save(&StoredSession {
                target: ServerTarget::new(&id, "Living room", "http://192.168.1.44:8080").unwrap(),
                token: "0123456789abcdef".into(),
                user: User {
                    id: 1,
                    email: "owner@example.com".into(),
                    display_name: None,
                    created_at: None,
                },
                device_name: "webOS TV".into(),
            })
            .unwrap();
        let servers = vec![
            server("Attic", &"b".repeat(64), "http://192.168.1.45:8080/"),
            server("Living room", &id, "http://192.168.1.44:8080/"),
        ];
        let chosen = choose_server(&servers, &["--discover".into()], &path).unwrap();
        assert_eq!(chosen.id, id);
    }

    #[test]
    fn remote_url_is_carried_into_the_target() {
        let mut server = server("Living room", &"a".repeat(64), "http://192.168.1.44:8080/");
        server.remote_url = Some("https://media.example.com/".into());
        let target = target_from_server(&server).unwrap();
        assert_eq!(target.lan_url, "http://192.168.1.44:8080/");
        assert_eq!(
            target.remote_url.as_deref(),
            Some("https://media.example.com/")
        );
        assert_eq!(target.api_version, 1);
    }

    #[test]
    fn discovery_timeout_is_bounded() {
        let args = ["--discover-timeout".into(), "0".into()];
        assert!(discovery_timeout(&args).is_err());
        let args = ["--discover-timeout".into(), "5".into()];
        assert_eq!(discovery_timeout(&args).unwrap(), Duration::from_secs(5));
        assert_eq!(discovery_timeout(&[]).unwrap(), Duration::from_secs(3));
    }
}
