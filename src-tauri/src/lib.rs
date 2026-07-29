mod capture;
mod server;

use server::ServerState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

struct AppState {
    server: Arc<Mutex<ServerState>>,
    data_dir: PathBuf,
    logs: Arc<Mutex<Vec<String>>>,
}

fn add_log(logs: &Arc<Mutex<Vec<String>>>, msg: &str) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    if let Ok(mut log) = logs.lock() {
        log.push(format!("[{}] {}", now, msg));
        if log.len() > 200 { log.remove(0); }
    }
}

fn save_state(app: &AppState) {
    let s = app.server.lock().unwrap();
    let q = s.capture.quality.lock().unwrap();
    let fps = s.capture.fps.lock().unwrap();
    let data = serde_json::json!({
        "password": s.password,
        "quality": *q,
        "fps": *fps,
    });
    let _ = std::fs::create_dir_all(&app.data_dir);
    let _ = std::fs::write(app.data_dir.join("state.json"), data.to_string());
}

fn load_state(app: &AppState) {
    let path = app.data_dir.join("state.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&raw) {
            let mut s = app.server.lock().unwrap();
            if let Some(pwd) = data.get("password") {
                s.password = pwd.as_str().map(|s| s.to_string());
            }
            if let Some(q) = data.get("quality").and_then(|v| v.as_u64()) {
                *s.capture.quality.lock().unwrap() = q as u8;
            }
            if let Some(f) = data.get("fps").and_then(|v| v.as_u64()) {
                *s.capture.fps.lock().unwrap() = f as u32;
            }
        }
    }
}

// ── Commands ──

#[tauri::command]
fn start_server(state: State<AppState>) -> Result<String, String> {
    server::start_server(Arc::clone(&state.server))?;
    let s = state.server.lock().map_err(|e| e.to_string())?;
    capture::start_capture(&s.capture)?;
    drop(s);
    add_log(&state.logs, "服务器已启动 (端口 8726) + 屏幕捕获");
    save_state(&state);
    Ok("Server started".into())
}

#[tauri::command]
fn stop_server(state: State<AppState>) -> Result<String, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    capture::stop_capture(&s.capture);
    drop(s);
    server::stop_server(Arc::clone(&state.server))?;
    add_log(&state.logs, "服务器已停止");
    Ok("Server stopped".into())
}

#[tauri::command]
fn start_capture(state: State<AppState>) -> Result<String, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    capture::start_capture(&s.capture)?;
    drop(s);
    add_log(&state.logs, "屏幕捕获已开始");
    Ok("Capture started".into())
}

#[tauri::command]
fn stop_capture(state: State<AppState>) -> Result<String, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    capture::stop_capture(&s.capture);
    drop(s);
    add_log(&state.logs, "屏幕捕获已停止");
    Ok("Capture stopped".into())
}

#[tauri::command]
fn set_quality(state: State<AppState>, quality: u8) -> Result<String, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    let q = quality.clamp(10, 100);
    *s.capture.quality.lock().unwrap() = q;
    drop(s);
    add_log(&state.logs, &format!("画质设为 {}%", q));
    save_state(&state);
    Ok("Quality updated".into())
}

#[tauri::command]
fn set_fps(state: State<AppState>, fps: u32) -> Result<String, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    let f = fps.clamp(1, 30);
    *s.capture.fps.lock().unwrap() = f;
    drop(s);
    add_log(&state.logs, &format!("帧率设为 {} FPS", f));
    save_state(&state);
    Ok("FPS updated".into())
}

#[tauri::command]
fn set_password(state: State<AppState>, password: String) -> Result<String, String> {
    let mut s = state.server.lock().map_err(|e| e.to_string())?;
    if password.is_empty() {
        s.password = None;
    } else {
        s.password = Some(password);
    }
    drop(s);
    save_state(&state);
    Ok("Password updated".into())
}

#[tauri::command]
fn is_server_running(state: State<AppState>) -> Result<bool, String> {
    Ok(state.server.lock().map_err(|e| e.to_string())?.active)
}

#[tauri::command]
fn is_capture_running(state: State<AppState>) -> Result<bool, String> {
    Ok(state.server.lock().map_err(|e| e.to_string())?.capture.active.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
fn get_local_ip() -> Result<String, String> {
    local_ip_address::local_ip().map(|ip| ip.to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_logs(state: State<AppState>) -> Result<Vec<String>, String> {
    Ok(state.logs.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn get_password(state: State<AppState>) -> Result<Option<String>, String> {
    Ok(state.server.lock().map_err(|e| e.to_string())?.password.clone())
}

#[tauri::command]
fn get_quality(state: State<AppState>) -> Result<u8, String> {
    Ok(*state.server.lock().map_err(|e| e.to_string())?.capture.quality.lock().unwrap())
}

#[tauri::command]
fn get_fps(state: State<AppState>) -> Result<u32, String> {
    Ok(*state.server.lock().map_err(|e| e.to_string())?.capture.fps.lock().unwrap())
}

#[derive(serde::Serialize)]
struct PollFrameResult {
    seq: u64,
    data: String,
}

#[tauri::command]
fn poll_frame(state: State<AppState>, seq: u64) -> Result<Option<PollFrameResult>, String> {
    let s = state.server.lock().map_err(|e| e.to_string())?;
    let frames = Arc::clone(&s.capture.frames);
    drop(s);
    Ok(capture::poll_frame(&frames, seq).map(|(seq, data)| PollFrameResult { seq, data }))
}

// ── Entry point ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("com.pil.broadcast");
    let _ = std::fs::create_dir_all(&data_dir);

    let lock_path = data_dir.join(".instance.lock");
    let singleton = std::fs::OpenOptions::new().create_new(true).write(true).open(&lock_path);
    match singleton {
        Ok(_) => {}
        Err(_) => {
            match std::net::TcpStream::connect("127.0.0.1:9725") {
                Ok(mut stream) => {
                    let _ = std::io::Write::write_all(&mut stream, b"show\n");
                    std::process::exit(0);
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&lock_path);
                    if std::fs::OpenOptions::new().create_new(true).write(true).open(&lock_path).is_err() {
                        std::process::exit(0);
                    }
                }
            }
        }
    };

    let app_state = AppState {
        server: Arc::new(Mutex::new(ServerState::new(8726))),
        data_dir,
        logs: Arc::new(Mutex::new(Vec::new())),
    };
    load_state(&app_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            start_server, stop_server,
            start_capture, stop_capture,
            set_quality, set_fps, set_password,
            is_server_running, is_capture_running,
            get_local_ip, get_logs, get_password, get_quality, get_fps,
            poll_frame,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            if let Some(ipc_win) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    if let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:9725") {
                        for conn in listener.incoming() {
                            if let Ok(mut s) = conn {
                                let mut buf = [0u8; 8];
                                if std::io::Read::read(&mut s, &mut buf).is_ok() {
                                    if buf.starts_with(b"show") {
                                        let _ = ipc_win.show();
                                        let _ = ipc_win.set_focus();
                                    }
                                }
                            }
                        }
                    }
                });
            }

            let show_item = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let feedback_item = MenuItemBuilder::with_id("feedback", "问题反馈").build(app)?;
            let support_item = MenuItemBuilder::with_id("support", "联系获得支持").build(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show_item, &sep1, &feedback_item, &support_item, &sep2, &quit_item])
                .build()?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("PILBroadcast")
                .on_menu_event({
                    let quit_lock = lock_path.clone();
                    move |app, event| {
                        match event.id().as_ref() {
                            "show" => {
                                if let Some(w) = app.get_webview_window("main") {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            }
                            "feedback" => {
                                let _ = std::process::Command::new("cmd")
                                    .args(["/c", "start", "https://feedback.yxpil.com/"])
                                    .spawn();
                            }
                            "support" => {
                                let _ = std::process::Command::new("cmd")
                                    .args(["/c", "start", "https://yxpil.com/"])
                                    .spawn();
                            }
                            "quit" => {
                                let _ = std::fs::remove_file(&quit_lock);
                                app.exit(0);
                            }
                            _ => {}
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.open_devtools();
            }

            std::mem::forget(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PILBroadcast");
}
