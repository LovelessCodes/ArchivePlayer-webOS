//! Host simulator for the native TV UI.
//!
//! It renders the same 1920x1080 canvas as the webOS target and displays it
//! through SDL2. `--all` writes deterministic screenshots without opening a
//! window, which is useful in CI and on headless machines.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use archiveplayer_tv_api::ServerTarget;
use archiveplayer_tv_gfx::Canvas;
use archiveplayer_tv_platform::{FileSecretStore, SecretStore};
use archiveplayer_tv_ui::Screen;
use archiveplayer_webos::pairing::{PairingFlow, PairingPoll};
use archiveplayer_webos::render;
use archiveplayer_webos::state::App;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{BlendMode, Texture};
use sdl2::video::FullscreenType;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn main() -> Result<(), String> {
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

    run_window()
}

fn print_help() {
    println!("ArchivePlayer webOS host simulator");
    println!();
    println!("  cargo run -p archiveplayer-webos-sim");
    println!("  cargo run -p archiveplayer-webos-sim -- --screenshot out.png --screen home");
    println!("  cargo run -p archiveplayer-webos-sim -- --all docs/screenshots");
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

fn run_window() -> Result<(), String> {
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let window = video
        .window("ArchivePlayer — webOS native preview", 1280, 720)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|error| error.to_string())?;
    let mut sdl_canvas = window
        .into_canvas()
        .build()
        .map_err(|error| error.to_string())?;
    sdl_canvas
        .set_logical_size(WIDTH, HEIGHT)
        .map_err(|error| error.to_string())?;
    let texture_creator = sdl_canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA8888, WIDTH, HEIGHT)
        .map_err(|error| error.to_string())?;
    texture.set_blend_mode(BlendMode::None);
    let mut event_pump = sdl.event_pump()?;
    let text_input = video.text_input();
    let mut app = App::demo();
    app.focus_index = 4;

    let mut frame_canvas = Canvas::new(WIDTH, HEIGHT)?;
    let mut last = Instant::now();
    let mut was_editing = false;

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(key), ..
                } => match key {
                    Keycode::Q if !app.search_editing => break 'running,
                    Keycode::Escape => app.back(),
                    Keycode::Backspace => {
                        if app.search_editing {
                            app.on_backspace();
                        } else {
                            app.back();
                        }
                    }
                    Keycode::Up => app.move_focus(archiveplayer_tv_ui::FocusMove::Up),
                    Keycode::Down => app.move_focus(archiveplayer_tv_ui::FocusMove::Down),
                    Keycode::Left => app.move_focus(archiveplayer_tv_ui::FocusMove::Left),
                    Keycode::Right => app.move_focus(archiveplayer_tv_ui::FocusMove::Right),
                    Keycode::Return | Keycode::KpEnter => app.activate(),
                    Keycode::Space => app.activate(),
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
                },
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

        let now = Instant::now();
        let delta = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;
        app.tick(delta);
        render::render(&app, &mut frame_canvas);
        upload_texture(&mut texture, &frame_canvas)?;
        sdl_canvas.clear();
        sdl_canvas
            .copy(&texture, None, None)
            .map_err(|error| error.to_string())?;
        sdl_canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

fn upload_texture(texture: &mut Texture, canvas: &Canvas) -> Result<(), String> {
    let pitch = (canvas.width() * 4) as usize;
    texture
        .update(None, canvas.pixels(), pitch)
        .map_err(|error| error.to_string())
}
