mod server;

use server::{ServerState, SharedFolder};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};

struct AppState {
    server: Arc<Mutex<ServerState>>,
    data_dir: PathBuf,
    logs: Arc<Mutex<Vec<String>>>,
}

fn state_path(data_dir: &PathBuf) -> PathBuf { data_dir.join("state.json") }

fn add_log(logs: &Arc<Mutex<Vec<String>>>, msg: &str) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    if let Ok(mut log) = logs.lock() {
        log.push(format!("[{}] {}", now, msg));
        if log.len() > 200 { log.remove(0); }
    }
}

fn save_state(app: &AppState) {
    let s = app.server.lock().unwrap();
    let data = serde_json::json!({"folders": s.folders, "password": s.password});
    let _ = std::fs::create_dir_all(&app.data_dir);
    let _ = std::fs::write(state_path(&app.data_dir), data.to_string());
}

fn load_state(app: &AppState) {
    let path = state_path(&app.data_dir);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&raw) {
            let mut s = app.server.lock().unwrap();
            if let Some(folders) = data.get("folders") {
                if let Ok(fs) = serde_json::from_value::<Vec<SharedFolder>>(folders.clone()) { s.folders = fs; }
            }
            if let Some(pwd) = data.get("password") { s.password = pwd.as_str().map(|s| s.to_string()); }
        }
    }
}

#[tauri::command] fn start_server(state: State<AppState>) -> Result<String, String> { server::start_server(Arc::clone(&state.server))?; add_log(&state.logs, "服务器已启动 (端口 9726)"); save_state(&state); Ok("Server started".into()) }
#[tauri::command] fn stop_server(state: State<AppState>) -> Result<String, String> { server::stop_server(Arc::clone(&state.server))?; add_log(&state.logs, "服务器已停止"); Ok("Server stopped".into()) }
#[tauri::command] fn add_folder(state: State<AppState>, name: String, path: String) -> Result<Vec<SharedFolder>, String> { let mut s = state.server.lock().map_err(|e| e.to_string())?; if s.folders.iter().any(|f| f.name == name) { return Err(format!("Folder '{}' already exists", name)); } s.folders.push(SharedFolder { name: name.clone(), path: path.clone(), active: true, allow_rename: false, allow_upload: false, allow_delete: false }); let result = s.folders.clone(); drop(s); add_log(&state.logs, &format!("添加文件夹: {}", name)); save_state(&state); Ok(result) }
#[tauri::command] fn remove_folder(state: State<AppState>, name: String) -> Result<Vec<SharedFolder>, String> { let mut s = state.server.lock().map_err(|e| e.to_string())?; s.folders.retain(|f| f.name != name); let result = s.folders.clone(); drop(s); add_log(&state.logs, &format!("移除文件夹: {}", name)); save_state(&state); Ok(result) }
#[tauri::command] fn toggle_folder(state: State<AppState>, name: String) -> Result<Vec<SharedFolder>, String> { let mut s = state.server.lock().map_err(|e| e.to_string())?; let mut status = false; if let Some(f) = s.folders.iter_mut().find(|f| f.name == name) { f.active = !f.active; status = f.active; } let result = s.folders.clone(); drop(s); add_log(&state.logs, &format!("文件夹 '{}' {}共享", name, if status { "开始" } else { "停止" })); save_state(&state); Ok(result) }
#[tauri::command] fn set_password(state: State<AppState>, password: String) -> Result<String, String> { let mut s = state.server.lock().map_err(|e| e.to_string())?; if password.is_empty() { s.password = None; } else { s.password = Some(password); } drop(s); save_state(&state); Ok("Password updated".into()) }
#[tauri::command] fn toggle_folder_perm(state: State<AppState>, name: String, perm: String) -> Result<Vec<SharedFolder>, String> { let mut s = state.server.lock().map_err(|e| e.to_string())?; if let Some(f) = s.folders.iter_mut().find(|f| f.name == name) { match perm.as_str() { "rename" => f.allow_rename = !f.allow_rename, "upload" => f.allow_upload = !f.allow_upload, "delete" => f.allow_delete = !f.allow_delete, _ => return Err("Invalid permission".into()), } } let result = s.folders.clone(); drop(s); add_log(&state.logs, &format!("文件夹 '{}' 权限变更", name)); save_state(&state); Ok(result) }
#[tauri::command] fn get_folders(state: State<AppState>) -> Result<Vec<SharedFolder>, String> { Ok(state.server.lock().map_err(|e| e.to_string())?.folders.clone()) }
#[tauri::command] fn get_password(state: State<AppState>) -> Result<Option<String>, String> { Ok(state.server.lock().map_err(|e| e.to_string())?.password.clone()) }
#[tauri::command] fn is_server_running(state: State<AppState>) -> Result<bool, String> { Ok(state.server.lock().map_err(|e| e.to_string())?.active) }
#[tauri::command] fn get_local_ip() -> Result<String, String> { local_ip_address::local_ip().map(|ip| ip.to_string()).map_err(|e| e.to_string()) }
#[tauri::command] fn get_logs(state: State<AppState>) -> Result<Vec<String>, String> { Ok(state.logs.lock().map_err(|e| e.to_string())?.clone()) }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("com.pil.publisher");
    let app_state = AppState { server: Arc::new(Mutex::new(ServerState::new(9726))), data_dir, logs: Arc::new(Mutex::new(Vec::new())) };
    load_state(&app_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            start_server, stop_server, add_folder, remove_folder, toggle_folder, toggle_folder_perm,
            set_password, get_folders, get_password, is_server_running, get_local_ip, get_logs,
        ])
        .setup(|app| {
            // System tray
            let show_item = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show_item, &quit_item]).build()?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("PiLPublisher")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => { if let Some(w) = app.get_webview_window("main") { let _ = w.show(); let _ = w.set_focus(); } }
                        "quit" => { app.exit(0); }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") { let _ = window.open_devtools(); }

            // Prevent the tray from being dropped
            std::mem::forget(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PiLPublisher");
}
