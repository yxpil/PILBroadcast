use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Response, Server, StatusCode};

// ── Inline SVG icons (stripped from public/Icons) ──

const ICON_FOLDER: &str = r#"<svg width="20" height="20" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M405 107a43 43 0 0133 15l94 113h268a85 85 0 0185 85v85h213a43 43 0 0142 51l-85 427a43 43 0 01-42 34H85l-1-0a41 41 0 01-3-0h-0a43 43 0 01-7-2l-1-0a44 44 0 01-5-2l-1-1a43 43 0 01-7-4l-1-1-2-1-1-1-1-1a43 43 0 01-8-16l-0-1A43 43 0 0143 875V192a85 85 0 0185-85h277zM140 832h678l68-341H225l-85 341zM128 515l19-77 2-7a43 43 0 0139-26h342V320H240V192h277z"/></svg>"#;

const ICON_FILE: &str = r#"<svg width="18" height="18" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M640 128a128 128 0 01128 128v512a128 128 0 01-128 128H256a128 128 0 01-128-128V256a128 128 0 01128-128h384zm-42 256H298a43 43 0 000 85h300a43 43 0 000-85zm42 170H298a43 43 0 000 86h342a43 43 0 000-86z"/></svg>"#;

const ICON_CLOSE: &str = r#"<svg width="16" height="16" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M738 226a43 43 0 0160 60L572 512l226 226a43 43 0 01-60 60L512 572 286 798a43 43 0 01-60-60L452 512 226 286a43 43 0 0160-60L512 452l226-226z"/></svg>"#;

const ICON_DELETE: &str = r#"<svg width="16" height="16" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M811 341a43 43 0 0143 43v427a128 128 0 01-128 128H299a128 128 0 01-128-128V384a43 43 0 0185 0v427a43 43 0 0043 43h427a43 43 0 0043-43V384a43 43 0 0143-43zM384 384a43 43 0 0143 43v299a43 43 0 11-85 0V427a43 43 0 0142-43zm256 0a43 43 0 0143 43v299a43 43 0 11-85 0V427a43 43 0 0142-43zm213-171a43 43 0 110 85H171a43 43 0 010-85h682zm-171-128a43 43 0 110 85H341a43 43 0 010-85h341z"/></svg>"#;

const ICON_BACK: &str = r#"<svg width="18" height="18" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M725 226a43 43 0 010 60L499 512l226 226a43 43 0 01-60 60L378 542a43 43 0 010-60l287-256a43 43 0 0160 0z"/></svg>"#;

const ICON_DOWNLOAD: &str = r#"<svg width="18" height="18" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M512 640a43 43 0 01-30-13L341 486a43 43 0 0160-60l68 68V171a43 43 0 1186 0v323l68-68a43 43 0 0160 60l-141 141a43 43 0 01-30 13z"/><path fill="currentColor" d="M213 725a43 43 0 00-85 0 171 171 0 00171 171h426a171 171 0 00171-171 43 43 0 00-85 0 85 85 0 01-85 85H299a85 85 0 01-85-85z"/></svg>"#;

const ICON_UPLOAD: &str = r#"<svg width="18" height="18" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M512 213a43 43 0 0130 13l141 141a43 43 0 01-60 60l-68-68v323a43 43 0 01-86 0V359l-68 68a43 43 0 01-60-60l141-141a43 43 0 0130-13z"/><path fill="currentColor" d="M213 725a43 43 0 00-85 0 171 171 0 00171 171h426a171 171 0 00171-171 43 43 0 00-85 0 85 85 0 01-85 85H299a85 85 0 01-85-85z"/></svg>"#;

const ICON_RENAME: &str = r#"<svg width="16" height="16" viewBox="0 0 1024 1024" fill="none"><path fill="currentColor" d="M725 128a128 128 0 0190 38l43 43a128 128 0 010 181L482 767a85 85 0 01-48 24l-170 26a43 43 0 01-47-47l26-170a85 85 0 0124-48l377-376a128 128 0 0181-48zm0 85a43 43 0 00-30 13L318 603l-17 113 113-17 377-377a43 43 0 000-60l-43-43a43 43 0 00-23-6z"/></svg>"#;

// ── HTML Generator ──

fn html_response(content: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let h = Header::from_bytes(&b"Content-Type"[..], "text/html; charset=utf-8").unwrap();
    Response::from_string(content).with_header(h)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolder {
    pub name: String,
    pub path: String,
    pub active: bool,
    pub allow_rename: bool,
    pub allow_upload: bool,
    pub allow_delete: bool,
}

pub struct ServerState {
    pub folders: Vec<SharedFolder>,
    pub password: Option<String>,
    pub active: bool,
    port: u16,
    shutdown_flag: Arc<AtomicBool>,
}

impl ServerState {
    pub fn new(port: u16) -> Self {
        Self { folders: Vec::new(), password: None, active: false, port,
            shutdown_flag: Arc::new(AtomicBool::new(true)),
        }
    }
}

// ── CSS stylesheet shared across all pages ──
const STYLESHEET: &str = r#"
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:#A8E6CF;min-height:100vh;color:#1b4332}
.header{display:flex;align-items:center;justify-content:space-between;padding:14px 20px;background:rgba(255,255,255,0.45);backdrop-filter:blur(10px);border-bottom:1px solid rgba(255,255,255,0.3)}
.header h1{font-size:20px;font-weight:700}
.header .actions{display:flex;gap:8px;align-items:center}
.btn{display:inline-flex;align-items:center;gap:6px;padding:8px 16px;border:none;border-radius:10px;background:rgba(255,255,255,0.85);color:#2d6a4f;font-size:14px;font-weight:600;cursor:pointer;text-decoration:none;transition:background .15s,transform .1s}
.btn:hover{background:#fff;transform:scale(1.02)}
.btn.danger{color:#d62828}
.btn.primary{background:#40916c;color:#fff}
.btn.primary:hover{background:#2d6a4f}
.btn svg{flex-shrink:0}
.container{max-width:860px;margin:0 auto;padding:20px}
.breadcrumb{display:flex;align-items:center;gap:6px;margin-bottom:20px;font-size:14px;flex-wrap:wrap}
.breadcrumb a{color:#40916c;text-decoration:none;font-weight:500}
.breadcrumb a:hover{text-decoration:underline}
.breadcrumb span{color:#6b9080}
.file-table{width:100%;border-collapse:collapse}
.file-table th{text-align:left;padding:10px 14px;font-size:12px;font-weight:600;color:#6b9080;text-transform:uppercase;letter-spacing:.5px;border-bottom:1px solid rgba(0,0,0,0.06)}
.file-table td{padding:10px 14px;font-size:14px;border-bottom:1px solid rgba(0,0,0,0.04)}
.file-table tr:hover td{background:rgba(255,255,255,0.4)}
.file-row{display:flex;align-items:center;gap:8px}
.file-row svg{flex-shrink:0;color:#6b9080}
.file-row a{color:#1b4332;text-decoration:none;font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:300px}
.file-row a:hover{color:#40916c}
.row-actions{display:flex;gap:4px}
.row-actions button,.row-actions a{padding:5px 8px;border:none;border-radius:8px;background:transparent;color:#6b9080;cursor:pointer;transition:background .15s,color .15s;display:flex}
.row-actions button:hover,.row-actions a:hover{background:rgba(255,255,255,0.7);color:#2d6a4f}
.row-actions .danger:hover{color:#d62828}
.size{color:#6b9080;font-size:13px;font-family:monospace}
.pwd-gate{max-width:400px;margin:80px auto;text-align:center}
.pwd-gate h1{font-size:28px;margin-bottom:12px}
.pwd-gate p{margin-bottom:20px;color:#6b9080}
.pwd-gate form{display:flex;gap:10px;justify-content:center}
.pwd-gate input{padding:12px 16px;border:none;border-radius:10px;font-size:16px;background:rgba(255,255,255,0.85);color:#1b4332;outline:none;width:220px}
.pwd-gate input::placeholder{color:#95d5b2}
.pwd-gate button{padding:12px 24px;border:none;border-radius:10px;background:rgba(255,255,255,0.9);color:#2d6a4f;font-size:16px;font-weight:600;cursor:pointer}
.empty-state{text-align:center;padding:60px 20px;color:#6b9080}
.empty-state svg{opacity:.4;margin-bottom:12px}
.empty-state p{font-size:16px}
footer{text-align:center;padding:20px;font-size:12px;color:#6b9080;opacity:.7}
.upload-area{border:2px dashed rgba(0,0,0,0.1);border-radius:12px;padding:30px;text-align:center;margin-bottom:20px;cursor:pointer;transition:border-color .15s,background .15s}
.upload-area:hover{border-color:#40916c;background:rgba(255,255,255,0.3)}
.upload-area p{color:#6b9080;font-size:14px}
"#;

fn page_wrapper(title: &str, body: String, extra: &str) -> String {
    format!(r#"<!DOCTYPE html><html lang="zh-CN">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} - PiLPublisher</title>
<style>{STYLESHEET}</style></head>
<body>{body}<footer>Powered by PiLPublisher</footer>{extra}</body></html>"#)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024*1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024*1024*1024 { format!("{:.1} MB", bytes as f64 / (1024.0*1024.0)) }
    else { format!("{:.2} GB", bytes as f64 / (1024.0*1024.0*1024.0)) }
}

// ── Root page (shared folders list) ──
fn build_root_page(folders: &[SharedFolder]) -> String {
    let items: String = folders.iter().filter(|f| f.active).map(|f| {
        let name = html_escape(&f.name);
        format!(r#"<tr>
            <td><div class="file-row">{ICON_FOLDER} <a href="/files/{name}">{name}</a></div></td>
            <td class="size">文件夹</td>
            <td></td></tr>"#)
    }).collect::<Vec<_>>().join("\n");

    let body = if items.is_empty() {
        format!(r#"<div class="empty-state">{ICON_FOLDER}<p>暂无共享内容</p></div>"#)
    } else {
        format!(r#"<div class="container">
            <table class="file-table"><thead><tr><th>名称</th><th>大小</th><th>操作</th></tr></thead><tbody>{items}</tbody></table></div>"#)
    };

    let header = r#"<div class="header"><h1>PiLPublisher</h1><span style="font-size:13px;color:#6b9080">文件共享</span></div>"#;

    page_wrapper("PiLPublisher", format!("{header}{body}"), "")
}

// ── Password gate ──
fn build_pwd_gate() -> String {
    let body = format!(r#"<div class="pwd-gate"><h1>PiLPublisher</h1><p>需要密码才能访问</p>
        <form method="post" action="/"><input type="password" name="pwd" placeholder="输入密码" autofocus/><button type="submit" class="btn primary">解锁</button></form></div>"#);
    page_wrapper("PiLPublisher", body, "")
}

// ── Folder browsing page ──
fn build_folder_page(folder: &SharedFolder, sub_path: &str) -> String {
    let folder_name = &folder.name;
    let listing_path = Path::new(&folder.path).join(sub_path);
    let url_prefix = if sub_path.is_empty() { folder_name.to_string() } else { format!("{}/{}", folder_name, sub_path) };
    let file_prefix = if sub_path.is_empty() { String::new() } else { format!("{}/", sub_path) };
    let mut rows = String::new();
    if let Ok(mut entries) = fs::read_dir(&listing_path) {
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        while let Some(Ok(e)) = entries.next() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push((name, true));
            } else {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((name, false, size));
            }
        }
        dirs.sort_by(|a,b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        files.sort_by(|a,b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        for (name, _) in &dirs {
            let escaped = html_escape(name);
            rows.push_str(&format!(r#"<tr>
                <td><div class="file-row">{ICON_FOLDER}<a href="/files/{url_prefix}/{escaped}">{escaped}</a></div></td>
                <td class="size">文件夹</td>
                <td></td></tr>"#));
        }
        for (name, _, size) in &files {
            let escaped = html_escape(name);
            let sz = format_size(*size);
            let full_name = format!("{}{}", file_prefix, name);
            let full_escaped = html_escape(&full_name);
            let mut actions = format!(r#"<a href="/dl/{url_prefix}/{escaped}" download title="下载">{ICON_DOWNLOAD}</a>"#);
            if folder.allow_rename {
                actions.push_str(&format!(r#"<button onclick="renameFile('{folder_name}','{full_escaped}')" title="重命名">{ICON_RENAME}</button>"#));
            }
            if folder.allow_delete {
                actions.push_str(&format!(r#"<button onclick="deleteFile('{folder_name}','{full_escaped}')" class="danger" title="删除">{ICON_DELETE}</button>"#));
            }
            rows.push_str(&format!(r#"<tr>
                <td><div class="file-row">{ICON_FILE}<a href="/dl/{url_prefix}/{escaped}" download>{escaped}</a></div></td>
                <td class="size">{sz}</td>
                <td><div class="row-actions">{actions}</div></td></tr>"#));
        }
    }

    let mut header_extra = String::from(r#"<a href="/" class="btn">"#);
    header_extra.push_str(ICON_BACK);
    header_extra.push_str(" 返回</a>");
    if folder.allow_upload {
        header_extra.push_str(r#"<button class="btn" onclick="document.getElementById('file-upload').click()">"#);
        header_extra.push_str(ICON_UPLOAD);
        header_extra.push_str(" 上传文件</button>");
    }

    let mut upload_html = String::new();
    if folder.allow_upload {
        upload_html = format!(r#"<div class="upload-area" onclick="document.getElementById('file-upload').click()">
            <p>点击此处上传文件到此文件夹</p></div>
        <form id="upload-form" style="display:none" method="post" action="/upload/{folder_name}" enctype="multipart/form-data">
            <input type="file" id="file-upload" name="file" onchange="this.form.submit()"/></form>"#);
    }

    let header_html = format!(r#"<div class="header">
        <h1>{folder_name}</h1>
        <div class="actions">{header_extra}</div></div>
        <div class="container">
        {upload_html}
        <table class="file-table"><thead><tr><th>名称</th><th>大小</th><th>操作</th></tr></thead><tbody>{rows}</tbody></table></div>"#);

    let mut scripts = String::new();
    if folder.allow_rename || folder.allow_delete {
        scripts.push_str(r#"<script>"#);
        if folder.allow_rename {
            scripts.push_str(r#"function renameFile(folder,oldName){var n=prompt('新文件名:',oldName);if(n&&n!==oldName){fetch('/rename/'+folder+'/'+encodeURIComponent(oldName),{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:'name='+encodeURIComponent(n)}).then(r=>{if(r.ok)location.reload();else alert('重命名失败')})}}"#);
        }
        if folder.allow_delete {
            scripts.push_str(r#"function deleteFile(folder,name){if(confirm('确定删除 '+name+'?')){fetch('/delete/'+folder+'/'+encodeURIComponent(name),{method:'POST'}).then(r=>{if(r.ok)location.reload();else alert('删除失败')})}}"#);
        }
        scripts.push_str("</script>");
    }

    page_wrapper(folder_name, header_html, &scripts)
}

fn html_escape(s: &str) -> String {
    s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;")
}

// ── Server start/stop ──

pub fn start_server(state: Arc<Mutex<ServerState>>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.active { return Err("Server is already active".into()); }
    s.active = true;
    // Start listener thread if not already running
    if s.shutdown_flag.load(Ordering::SeqCst) {
        // First time — start the listener
        s.shutdown_flag.store(false, Ordering::SeqCst);
        let port = s.port;
        let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .map_err(|e| format!("Failed to bind: {}", e))?;
        let _ = listener.set_nonblocking(false);
        let server = Server::from_listener(listener, None)
            .map_err(|e| format!("Failed to create server: {}", e))?;
        let shutdown_flag = Arc::clone(&s.shutdown_flag);
        drop(s);
        let state_clone = Arc::clone(&state);
        thread::spawn(move || {
            loop {
                if shutdown_flag.load(Ordering::SeqCst) { break; }
                match server.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(Some(request)) => handle_request(request, &state_clone),
                    Ok(None) => continue,
                    Err(_) => break,
                }
            }
        });
    } else {
        drop(s);
    }
    Ok(())
}

pub fn stop_server(state: Arc<Mutex<ServerState>>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.active = false;
    Ok(())
}

/// Build the "paused" page
fn build_paused_page() -> String {
    let body = r#"<div class="pwd-gate"><h1>PiLPublisher</h1><p>服务已暂停</p><p style="font-size:13px;color:#6b9080">请等待管理员重新开启共享</p></div>"#;
    page_wrapper("PiLPublisher", body.to_string(), "")
}

// ── Request handler ──

fn handle_request(mut request: tiny_http::Request, state: &Arc<Mutex<ServerState>>) {
    let url = request.url().to_string();
    let method = request.method().clone();

    let s = state.lock().unwrap();

    // If server is paused, show paused page
    if !s.active {
        let _ = request.respond(html_response(build_paused_page()));
        return;
    }

    // Password check
    let mut authed = false;
    if let Some(ref pwd) = s.password {
        for h in request.headers() {
            if h.field.equiv("Cookie") && h.value.as_str().contains(&format!("auth={}", pwd)) { authed = true; }
        }
    } else { authed = true; }

    // POST password
    if method == tiny_http::Method::Post && url == "/" {
        let mut body = String::new(); let _ = request.as_reader().read_to_string(&mut body);
        let submitted = body.split('&').filter_map(|p| {
            let mut kv = p.splitn(2,'=');
            if kv.next() == Some("pwd") { kv.next().map(|v| v.to_string()) } else { None }
        }).next().unwrap_or_default();
        if let Some(ref pwd) = s.password {
            if submitted == *pwd {
                let cookie = format!("auth={}; Path=/; Max-Age=86400", pwd);
                let h = Header::from_bytes(&b"Set-Cookie"[..], cookie.into_bytes()).unwrap();
                let _ = request.respond(html_response(build_root_page(&s.folders)).with_header(h));
                return;
            }
        }
        let _ = request.respond(html_response(build_pwd_gate()).with_status_code(StatusCode(403)));
        return;
    }

    if !authed {
        let _ = request.respond(html_response(build_pwd_gate()));
        return;
    }

    // POST /rename/<folder>/<path...>
    if method == tiny_http::Method::Post && url.starts_with("/rename/") {
        let rest = &url["/rename/".len()..];
        let parts: Vec<&str> = rest.splitn(2,'/').collect();
        if parts.len() >= 2 {
            let folder_name = parts[0];
            let old_path = percent_decode(parts[1]);
            let mut body = String::new(); let _ = request.as_reader().read_to_string(&mut body);
            let new_name = body.split('&').filter_map(|p| {
                let mut kv = p.splitn(2,'=');
                if kv.next() == Some("name") { kv.next().map(|v| percent_decode(v)) } else { None }
            }).next().unwrap_or_default();
            if !new_name.is_empty() {
                if let Some(folder) = s.folders.iter().find(|f| f.name == folder_name && f.active) {
                    let old = PathBuf::from(&folder.path).join(&old_path);
                    let new = PathBuf::from(&folder.path).join(&new_name);
                    if old.exists() && !new.exists() {
                        let _ = fs::rename(&old, &new);
                    }
                }
            }
        }
        let _ = request.respond(html_response("ok".into()).with_status_code(StatusCode(200)));
        return;
    }

    // POST /delete/<folder>/<path...>
    if method == tiny_http::Method::Post && url.starts_with("/delete/") {
        let rest = &url["/delete/".len()..];
        let parts: Vec<&str> = rest.splitn(2,'/').collect();
        if parts.len() >= 2 {
            let folder_name = parts[0];
            let path = percent_decode(parts[1]);
            if let Some(folder) = s.folders.iter().find(|f| f.name == folder_name && f.active) {
                let full = PathBuf::from(&folder.path).join(&path);
                let _ = if full.is_dir() { fs::remove_dir_all(&full) } else { fs::remove_file(&full) };
            }
        }
        let _ = request.respond(html_response("ok".into()));
        return;
    }

    // POST /upload/<folder>
    if method == tiny_http::Method::Post && url.starts_with("/upload/") {
        let folder_name = &url["/upload/".len()..];
        if let Some(folder) = s.folders.iter().find(|f| f.name == folder_name && f.active) {
            let mut body = Vec::new();
            // Read the multipart body
            let content_type = request.headers().iter()
                .find(|h| h.field.equiv("Content-Type"))
                .map(|h| h.value.to_string())
                .unwrap_or_default();

            if let Some(boundary) = content_type.split("boundary=").nth(1) {
                let boundary = boundary.trim_matches('"');
                let _ = request.as_reader().read_to_end(&mut body);

                // Parse multipart: find filename and file data
                if let Some(filename) = extract_multipart_filename(&body, boundary) {
                    if let Some(data) = extract_multipart_data(&body, boundary) {
                        let dest = PathBuf::from(&folder.path).join(&filename);
                        if !dest.exists() {
                            let _ = fs::write(&dest, data);
                        }
                    }
                }
            }
        }
        // Redirect back to folder page
        let redirect = format!("<meta http-equiv=\"refresh\" content=\"0;url=/files/{}\">",
            folder_name);
        let _ = request.respond(html_response(redirect));
        return;
    }

    // GET /files/<folder>/<path...>
    if url.starts_with("/files/") {
        let rest = &url["/files/".len()..];
        let path = percent_decode(rest);
        let parts: Vec<&str> = path.splitn(2,'/').collect();
        let folder_name = parts[0];
        let sub_path = if parts.len() > 1 { parts[1] } else { "" };
        if let Some(folder) = s.folders.iter().find(|f| f.name == folder_name && f.active) {
            let full = PathBuf::from(&folder.path).join(sub_path);
            if full.is_dir() {
                let html = build_folder_page(folder, sub_path);
                let _ = request.respond(html_response(html));
            } else if full.is_file() {
                serve_file(request, &full);
            } else {
                let _ = request.respond(html_response("Not Found".into()).with_status_code(StatusCode(404)));
            }
        } else {
            let _ = request.respond(html_response("Not Found".into()).with_status_code(StatusCode(404)));
        }
        return;
    }

    // GET /dl/<folder>/<file>
    if url.starts_with("/dl/") {
        let rest = &url["/dl/".len()..];
        let path = percent_decode(rest);
        let parts: Vec<&str> = path.splitn(2,'/').collect();
        if parts.len() >= 2 {
            let folder_name = parts[0];
            let file_path = parts[1];
            if let Some(folder) = s.folders.iter().find(|f| f.name == folder_name && f.active) {
                let full = PathBuf::from(&folder.path).join(file_path);
                serve_file(request, &full);
                return;
            }
        }
        let _ = request.respond(html_response("Not Found".into()).with_status_code(StatusCode(404)));
        return;
    }

    // Root
    let html = build_root_page(&s.folders);
    let _ = request.respond(html_response(html));
}

// ── File serving ──
fn serve_file(request: tiny_http::Request, path: &Path) {
    match fs::read(path) {
        Ok(data) => {
            let mime = mime_guess(path);
            let h = Header::from_bytes(&b"Content-Type"[..], mime.into_bytes()).unwrap();
            let _ = request.respond(Response::from_data(data).with_header(h));
        }
        Err(_) => { let _ = request.respond(html_response("Not Found".into()).with_status_code(StatusCode(404))); }
    }
}

fn mime_guess(path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "html"|"htm"=>"text/html; charset=utf-8","css"=>"text/css; charset=utf-8","js"=>"application/javascript; charset=utf-8",
        "json"=>"application/json; charset=utf-8","png"=>"image/png","jpg"|"jpeg"=>"image/jpeg",
        "gif"=>"image/gif","svg"=>"image/svg+xml","pdf"=>"application/pdf","txt"=>"text/plain; charset=utf-8",
        "mp3"=>"audio/mpeg","mp4"=>"video/mp4","zip"=>"application/zip","wasm"=>"application/wasm",
        _=>"application/octet-stream"
    }.to_string()
}

// ── Multipart parsing (simple, no external crate) ──
fn extract_multipart_filename(body: &[u8], _boundary: &str) -> Option<String> {
    let header_end = find_bytes(body, b"\r\n\r\n")?;
    let headers = std::str::from_utf8(&body[..header_end]).ok()?;
    for line in headers.lines() {
        if line.contains("filename=\"") {
            let start = line.find("filename=\"")? + 10;
            let end = line[start..].find('"')?;
            let name = &line[start..start+end];
            // Get just the filename from the path
            return Some(Path::new(name).file_name()?.to_string_lossy().to_string());
        }
    }
    None
}

fn extract_multipart_data(body: &[u8], boundary: &str) -> Option<Vec<u8>> {
    let header_end = find_bytes(body, b"\r\n\r\n")?;
    let data_start = header_end + 4;
    // Find end boundary
    let end_marker = format!("\r\n--{}", boundary);
    let end_pos = find_bytes(&body[data_start..], end_marker.as_bytes())?;
    Some(body[data_start..data_start+end_pos].to_vec())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().and_then(|c| c.to_digit(16));
            let h2 = chars.next().and_then(|c| c.to_digit(16));
            if let (Some(h1), Some(h2)) = (h1, h2) { result.push(char::from_u32(h1*16+h2).unwrap_or('?')); }
        } else if c == '+' { result.push(' '); }
        else { result.push(c); }
    }
    result
}
