//! Integration tests for the PILBroadcast tiny_http server.
//!
//! These boot the *real* server (`server::start_server`) on a loopback port and
//! drive it over raw TCP, exactly like a browser would. They cover the public
//! routing contract, the password gate, and — importantly — the untrusted-input
//! surface (malformed query strings, path traversal, reflected XSS attempts).
//!
//! A tiny hand-rolled HTTP client is used so the test-suite has no extra
//! dependency (the crate does not ship a client).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pil_broadcast::server::{start_server, ServerState};

/// Pick a free loopback port (bind to :0, then release it). There is a tiny TOCTOU
/// window but it is irrelevant on localhost in a test process.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn make_state(port: u16) -> Arc<Mutex<ServerState>> {
    Arc::new(Mutex::new(ServerState::new(port)))
}

/// Send a raw request and return (status_code, full_response_text).
fn raw(port: u16, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to loopback server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    let status = buf
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, buf)
}

fn get(port: u16, path: &str, extra_headers: &str) -> (u16, String) {
    raw(port, &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{extra_headers}\r\n"))
}

fn post_form(port: u16, body: &str, extra_headers: &str) -> (u16, String) {
    raw(
        port,
        &format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n{extra_headers}\r\n{body}",
            body.len()
        ),
    )
}

// ── happy-path routing ──

#[test]
fn viewer_served_without_password() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    let (status, body) = get(port, "/", "");
    assert_eq!(status, 200);
    assert!(body.contains("PILBroadcast"), "viewer page should brand the app");
}

#[test]
fn unknown_falls_back_to_viewer() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    let (status, body) = get(port, "/some/garbage/path", "");
    assert_eq!(status, 200, "unknown routes fall through to the viewer, not 500");
    assert!(body.contains("PILBroadcast"));
}

#[test]
fn status_endpoint_reports_json() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    let (status, body) = get(port, "/status", "");
    assert_eq!(status, 200);
    assert!(body.contains("application/json"));
    assert!(body.contains("\"capture\""));
    assert!(body.contains("\"frames\""));
    assert!(body.contains("\"fps\""));
}

#[test]
fn frame_poll_with_empty_queue_is_204() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    let (status, _body) = get(port, "/frame", "");
    // 204 No Content must not carry a body by HTTP semantics, so we only check the code.
    assert_eq!(status, 204);
}

#[test]
fn double_start_rejected_while_active() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();
    let second = start_server(state.clone());
    assert!(second.is_err(), "second start while active must error");
    assert!(second.unwrap_err().contains("already active"));
}

// ── password gate ──

#[test]
fn password_gate_blocks_then_allows() {
    let port = free_port();
    let state = make_state(port);
    {
        let mut s = state.lock().unwrap();
        s.password = Some("s3cret".to_string());
    }
    start_server(state.clone()).unwrap();

    // unauthenticated GET → gate page (still 200 HTML, but not the viewer)
    let (status, body) = get(port, "/", "");
    assert_eq!(status, 200);
    assert!(body.contains("需要密码才能访问"), "must show the password gate");
    assert!(!body.contains("/stream"), "must NOT leak the stream to anonymous visitors");

    // wrong password → 403
    let (status, _) = post_form(port, "pwd=wrong", "");
    assert_eq!(status, 403, "wrong password must be rejected");

    // correct password → 200 + Set-Cookie
    let (status, body) = post_form(port, "pwd=s3cret", "");
    assert_eq!(status, 200);
    assert!(body.contains("Set-Cookie"), "login must set a session cookie");
    assert!(body.contains("/stream"), "authenticated viewer should expose the stream");

    // reuse the cookie → viewer
    let (status, body) = get(port, "/", "Cookie: auth=s3cret\r\n");
    assert_eq!(status, 200);
    assert!(body.contains("/stream"));
}

#[test]
fn wrong_cookie_does_not_authenticate() {
    let port = free_port();
    let state = make_state(port);
    {
        let mut s = state.lock().unwrap();
        s.password = Some("s3cret".to_string());
    }
    start_server(state.clone()).unwrap();

    // attacker guesses/forges a cookie; it must not match the real password
    let (_, body) = get(port, "/", "Cookie: auth=nots3cret\r\n");
    assert!(body.contains("需要密码才能访问"), "forged cookie must not grant access");
    let (_, body) = get(port, "/", "Cookie: auth=\r\n");
    assert!(body.contains("需要密码才能访问"));
}

// ── injection / malicious input ──

#[test]
fn injection_malformed_seq_query_does_not_panic() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    // non-numeric, negative, overflow-ish, and XSS payloads in the seq query.
    // Each must be parsed as "no after_seq" and answered (204), never panic/500.
    // Note: a space inside the request line is itself rejected by tiny_http as a
    // 400 Bad Request — that is the HTTP layer correctly refusing a malformed
    // request, which is also a safe outcome (no panic, no 5xx from our code).
    for evil in [
        "/frame?seq=abc",
        "/frame?seq=-1",
        "/frame?seq=99999999999999999999999",
        "/frame?seq=<script>alert(1)</script>",
        "/frame?seq=",
        "/frame?malicious=true",
    ] {
        let (status, _) = get(port, evil, "");
        assert!(
            status == 204 || status == 200,
            "{} should not crash the server, got {}",
            evil,
            status
        );
    }
    // A request line containing a raw space (SQLi-style with spaces) must be
    // rejected cleanly by the HTTP layer (400), never crash the worker.
    let (status, _) = get(port, "/frame?seq=' OR 1=1--", "");
    assert!(
        status == 400 || status == 200 || status == 204,
        "space-laden malformed request should be rejected cleanly, got {}",
        status
    );

    // server still healthy after the abuse
    let (status, body) = get(port, "/status", "");
    assert_eq!(status, 200, "server must keep serving after malformed input");
    assert!(body.contains("\"capture\""));
}

#[test]
fn injection_path_traversal_is_not_served() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    // Classic traversal + encoded variants. The server has no static-file handler,
    // so these must fall through to the viewer — never read a file off disk.
    for evil in [
        "/../../../../etc/passwd",
        "/..%2f..%2f..%2fwindows/win.ini",
        "/%2e%2e/%2e%2e/secret",
        "/../../../C:/Windows/System32/drivers/etc/hosts",
    ] {
        let (status, body) = get(port, evil, "");
        assert!(status == 200, "{} should not 500", evil);
        assert!(!body.contains("root:"), "must not leak /etc/passwd via {}", evil);
        assert!(!body.contains("[fonts]"), "must not leak win.ini via {}", evil);
        assert!(body.contains("PILBroadcast"), "unknown path falls back to the viewer");
    }
}

#[test]
fn injection_xss_payload_in_password_is_not_reflected() {
    let port = free_port();
    let state = make_state(port);
    {
        let mut s = state.lock().unwrap();
        s.password = Some("real".to_string());
    }
    start_server(state.clone()).unwrap();

    // Submit a script tag as the password guess. It must be rejected AND must not
    // be echoed back into the HTML (no reflected XSS).
    let (status, body) = post_form(port, "pwd=<script>alert('xss')</script>", "");
    assert_eq!(status, 403, "the guess is wrong, expect 403");
    assert!(
        !body.contains("<script>alert('xss')</script>"),
        "password input must not be reflected into the response HTML"
    );
    // The gate page itself must be plain static markup.
    assert!(body.contains("需要密码才能访问"));
}

#[test]
fn injection_huge_query_and_weird_method_are_tolerated() {
    let port = free_port();
    let state = make_state(port);
    start_server(state.clone()).unwrap();

    // A very long query string must be handled, not crash the worker.
    let long_q = format!("/frame?seq={}", "1".repeat(5000));
    let (status, _) = get(port, &long_q, "");
    assert!(status == 200 || status == 204, "huge query should be tolerated, got {}", status);

    // Unknown method verb still gets a response (falls through routing).
    let (status, body) = raw(port, "DELETE / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(status > 0, "server must respond even to odd methods");
    let _ = &body;
}
