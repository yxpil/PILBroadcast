use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use base64::Engine;
use tiny_http::{Header, Response, Server, StatusCode};

use crate::capture::{CaptureState, Frame};

// ── CSS ──
const STYLESHEET: &str = r#"
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:#A8E6CF;min-height:100vh;color:#1b4332}
.header{display:flex;align-items:center;justify-content:space-between;padding:14px 20px;background:rgba(255,255,255,0.45);backdrop-filter:blur(10px);border-bottom:1px solid rgba(255,255,255,0.3)}
.header h1{font-size:20px;font-weight:700}
.header .badge{font-size:13px;color:#6b9080}
.container{max-width:960px;margin:0 auto;padding:16px}
.stream-wrapper{background:#1b4332;border-radius:14px;overflow:hidden;box-shadow:0 4px 20px rgba(0,0,0,0.12);margin-bottom:12px;display:flex;align-items:center;justify-content:center;min-height:200px;position:relative}
.stream-wrapper img{width:100%;height:auto;display:block}
.stream-wrapper .waiting-text{position:absolute;color:rgba(255,255,255,0.6);font-size:14px;pointer-events:none}
.source-tabs{display:flex;gap:8px;margin-bottom:12px;justify-content:center}
.source-tab{padding:8px 20px;border:none;border-radius:10px;background:rgba(255,255,255,0.7);color:#2d6a4f;font-size:14px;font-weight:600;cursor:pointer;transition:background .15s}
.source-tab:hover{background:#fff}
.source-tab.active{background:#40916c;color:#fff}
.pwd-gate{max-width:400px;margin:80px auto;text-align:center}
.pwd-gate h1{font-size:28px;margin-bottom:12px}
.pwd-gate p{margin-bottom:20px;color:#6b9080}
.pwd-gate form{display:flex;gap:10px;justify-content:center}
.pwd-gate input{padding:12px 16px;border:none;border-radius:10px;font-size:16px;background:rgba(255,255,255,0.85);color:#1b4332;outline:none;width:220px}
.pwd-gate input::placeholder{color:#95d5b2}
.pwd-gate button{padding:12px 24px;border:none;border-radius:10px;background:rgba(255,255,255,0.9);color:#2d6a4f;font-size:16px;font-weight:600;cursor:pointer}
.paused{text-align:center;padding:60px;color:#6b9080}
footer{text-align:center;padding:20px;font-size:12px;color:#6b9080;opacity:.7}
"#;

pub struct ServerState {
    pub password: Option<String>,
    pub active: bool,
    port: u16,
    shutdown_flag: Arc<AtomicBool>,
    pub capture: CaptureState,
}

impl ServerState {
    pub fn new(port: u16) -> Self {
        Self {
            password: None, active: false, port,
            shutdown_flag: Arc::new(AtomicBool::new(true)),
            capture: CaptureState::new(),
        }
    }
}

fn page_wrapper(title: &str, body: String, extra: &str) -> String {
    format!(r#"<!DOCTYPE html><html lang="zh-CN">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} - PILBroadcast</title>
<style>{STYLESHEET}</style></head>
<body>{body}<footer>Powered by PILBroadcast</footer>{extra}</body></html>"#)
}

fn html_response(content: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let h = Header::from_bytes(&b"Content-Type"[..], "text/html; charset=utf-8").unwrap();
    Response::from_string(content).with_header(h)
}

fn jpeg_response(data: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    let h = Header::from_bytes(&b"Content-Type"[..], "image/jpeg".as_bytes().to_vec()).unwrap();
    let cache = Header::from_bytes(&b"Cache-Control"[..], "no-cache, no-store, must-revalidate".as_bytes().to_vec()).unwrap();
    Response::from_data(data).with_header(h).with_header(cache)
}

fn build_pwd_gate() -> String {
    let body = r#"<div class="pwd-gate"><h1>PILBroadcast</h1><p>需要密码才能访问</p>
        <form method="post" action="/"><input type="password" name="pwd" placeholder="输入密码" autofocus/><button type="submit">解锁</button></form></div>"#;
    page_wrapper("PILBroadcast", body.to_string(), "")
}

fn build_viewer_page() -> String {
    let body = r#"<div class="header"><h1>PILBroadcast</h1><span class="badge">直播</span></div>
<div class="container">
    <div class="stream-wrapper"><img src="/stream" onload="this.style.display='block';var w=document.getElementById('waiting-text');if(w)w.style.display='none'" /><span class="waiting-text" id="waiting-text">等待视频流...</span></div>
</div>"#;
    page_wrapper("PILBroadcast", body.to_string(), "")
}

fn build_paused_page() -> String {
    let body = r#"<div class="pwd-gate"><h1>PILBroadcast</h1><p>直播已暂停</p><p style="font-size:13px;color:#6b9080">请等待管理员重新开启</p></div>"#;
    page_wrapper("PILBroadcast", body.to_string(), "")
}

// ── Audio PCM stream ──

// ── MJPEG stream body ──
use std::collections::VecDeque;

struct MjpegBody {
    frames: Arc<Mutex<VecDeque<Frame>>>,
    active: Arc<AtomicBool>,
    last_seq: u64,
    buf: Vec<u8>,
    pos: usize,
}

const MJPEG_BOUNDARY: &str = "--mjpegframe";

impl MjpegBody {
    fn new(frames: Arc<Mutex<VecDeque<Frame>>>, active: Arc<AtomicBool>) -> Self {
        Self { frames, active, last_seq: 0, buf: Vec::new(), pos: 0 }
    }
}

impl Read for MjpegBody {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        // Drain buffered data first
        if self.pos < self.buf.len() {
            let n = (self.buf.len() - self.pos).min(out.len());
            out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            if self.pos >= self.buf.len() { self.buf.clear(); self.pos = 0; }
            return Ok(n);
        }

        // Wait for a new frame
        loop {
            if !self.active.load(Ordering::SeqCst) {
                return Ok(0); // EOF — capture stopped
            }
            let queue = self.frames.lock().unwrap();
            let frame = queue.iter().find(|f| f.seq > self.last_seq).cloned();
            drop(queue);
            if let Some(f) = frame {
                self.last_seq = f.seq;
                let header = format!(
                    "--{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    MJPEG_BOUNDARY, f.data.len()
                );
                self.buf = header.into_bytes();
                self.buf.extend_from_slice(&f.data);
                self.buf.extend_from_slice(b"\r\n");
                self.pos = 0;
                let n = self.buf.len().min(out.len());
                out[..n].copy_from_slice(&self.buf[..n]);
                self.pos = n;
                return Ok(n);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

// ── Server start/stop ──
pub fn start_server(state: Arc<Mutex<ServerState>>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.active { return Err("Server already active".into()); }
    s.active = true;

    if s.shutdown_flag.load(Ordering::SeqCst) {
        s.shutdown_flag.store(false, Ordering::SeqCst);
        let port = s.port;
        let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .map_err(|e| format!("Bind failed: {}", e))?;
        let _ = listener.set_nonblocking(false);
        let server = Server::from_listener(listener, None)
            .map_err(|e| format!("Server failed: {}", e))?;
        let shutdown_flag = Arc::clone(&s.shutdown_flag);
        drop(s);
        let state_clone = Arc::clone(&state);
        thread::spawn(move || loop {
            if shutdown_flag.load(Ordering::SeqCst) { break; }
            match server.recv_timeout(Duration::from_millis(500)) {
                Ok(Some(req)) => handle_request(req, &state_clone),
                Ok(None) => continue,
                Err(_) => break,
            }
        });
    } else { drop(s); }
    Ok(())
}

pub fn stop_server(state: Arc<Mutex<ServerState>>) -> Result<(), String> {
    state.lock().map_err(|e| e.to_string())?.active = false;
    Ok(())
}

fn handle_request(mut request: tiny_http::Request, state: &Arc<Mutex<ServerState>>) {
    let url = request.url().to_string();
    let method = request.method().clone();
    let s = state.lock().unwrap();

    if !s.active { let _ = request.respond(html_response(build_paused_page())); return; }

    let mut authed = false;
    if let Some(ref pwd) = s.password {
        for h in request.headers() {
            if h.field.equiv("Cookie") && h.value.as_str().contains(&format!("auth={}", pwd)) { authed = true; }
        }
    } else { authed = true; }

    if method == tiny_http::Method::Post && url == "/" {
        let mut body = String::new(); let _ = request.as_reader().read_to_string(&mut body);
        let sub = body.split('&').filter_map(|p| { let mut kv = p.splitn(2,'='); if kv.next()==Some("pwd") { kv.next().map(|v| v.to_string()) } else { None } }).next().unwrap_or_default();
        if let Some(ref pwd) = s.password {
            if sub == *pwd {
                let cookie = format!("auth={}; Path=/; Max-Age=86400", pwd);
                let h = Header::from_bytes(&b"Set-Cookie"[..], cookie.into_bytes()).unwrap();
                let _ = request.respond(html_response(build_viewer_page()).with_header(h));
                return;
            }
        }
        let _ = request.respond(html_response(build_pwd_gate()).with_status_code(StatusCode(403)));
        return;
    }

    if !authed { let _ = request.respond(html_response(build_pwd_gate())); return; }


    // MJPEG stream — browser <img> auto-refreshes, zero JS needed
    if url == "/stream" {
        let frames = Arc::clone(&s.capture.frames);
        let active = Arc::clone(&s.capture.active);
        drop(s);
        let body = MjpegBody::new(frames, active);
        let ct = format!("multipart/x-mixed-replace; boundary={}", MJPEG_BOUNDARY);
        let h = Header::from_bytes(&b"Content-Type"[..], ct.into_bytes()).unwrap();
        let resp = Response::new(StatusCode(200), vec![h], Box::new(body), None, None);
        // Spawn thread so the main server loop isn't blocked by the persistent stream
        thread::spawn(move || { let _ = request.respond(resp); });
        return;
    }

    // Frame polling — returns latest JPEG frame
    if url == "/frame" || url.starts_with("/frame?") {
        // Parse ?seq=N query param
        let after_seq: Option<u64> = url.split('?').nth(1).and_then(|q| {
            q.split('&').filter_map(|p| {
                let mut kv = p.splitn(2, '=');
                if kv.next() == Some("seq") { kv.next().and_then(|v| v.parse().ok()) } else { None }
            }).next()
        });

        let queue = s.capture.frames.lock().unwrap();
        let frame = if let Some(after) = after_seq {
            if after == 0 {
                // First request after page load: give the latest frame
                queue.back().cloned()
            } else {
                // Incremental: next frame after the one we already have
                queue.iter().find(|f| f.seq > after).cloned()
            }
        } else {
            queue.back().cloned()
        };
        drop(queue);
        drop(s);

        if let Some(f) = frame {
            let seq_h = Header::from_bytes(&b"X-Frame-Seq"[..], f.seq.to_string().into_bytes()).unwrap();
            let _ = request.respond(jpeg_response(f.data).with_header(seq_h));
        } else {
            let _ = request.respond(Response::from_string("no frame").with_status_code(StatusCode(204)));
        }
        return;
    }

    // Frame as base64 JSON — returns {"seq":N,"data":"base64..."}
    if url.starts_with("/frameb64") {
        let after_seq: Option<u64> = url.split('?').nth(1).and_then(|q| {
            q.split('&').filter_map(|p| {
                let mut kv = p.splitn(2, '=');
                if kv.next() == Some("seq") { kv.next().and_then(|v| v.parse().ok()) } else { None }
            }).next()
        });

        let queue = s.capture.frames.lock().unwrap();
        let frame = if let Some(after) = after_seq {
            if after == 0 { queue.back().cloned() }
            else { queue.iter().find(|f| f.seq > after).cloned() }
        } else { queue.back().cloned() };
        drop(queue);
        drop(s);

        if let Some(f) = frame {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&f.data);
            let json = format!(r#"{{"seq":{},"data":"{}"}}"#, f.seq, b64);
            let h = Header::from_bytes(&b"Content-Type"[..], "application/json".as_bytes().to_vec()).unwrap();
            let _ = request.respond(Response::from_string(json).with_header(h));
        } else {
            let _ = request.respond(Response::from_string(r#"{"seq":null,"data":null}"#)
                .with_status_code(StatusCode(204)));
        }
        return;
    }

    // Status endpoint for debugging
    if url == "/status" {
        let queue = s.capture.frames.lock().unwrap();
        let frame_count = queue.len();
        let latest_seq = queue.back().map(|f| f.seq);
        drop(queue);
        let cap_active = s.capture.active.load(Ordering::SeqCst);
        let fps = *s.capture.fps.lock().unwrap();
        drop(s);
        let json = format!(r#"{{"capture":{},"frames":{},"latest_seq":{},"fps":{}}}"#,
            cap_active, frame_count,
            latest_seq.map(|s| s.to_string()).unwrap_or_else(|| "null".into()),
            fps);
        let h = Header::from_bytes(&b"Content-Type"[..], "application/json".as_bytes().to_vec()).unwrap();
        let _ = request.respond(Response::from_string(json).with_header(h));
        return;
    }


    let html = build_viewer_page();
    drop(s);
    let _ = request.respond(html_response(html));
}
