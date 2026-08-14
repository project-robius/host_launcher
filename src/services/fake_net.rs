//! A tiny deterministic HTTP server the launcher runs for itself in headless
//! test runs (or with HOST_LAUNCHER_FAKE_NET=1): the `env` service then hands
//! apps 127.0.0.1 URLs instead of the real endpoints, so the WHOLE network
//! path — script `net.http_request`, the platform HTTP stack, response
//! routing back into the isolate — is exercised without touching the
//! internet or flaking on it. Explicit env URL overrides still win.

use std::io::{Read, Write};
use std::net::TcpListener;

pub struct FakeNet {
    pub port: u16,
}

/// Starts the server when this run should be hermetic. The thread is
/// detached; it dies with the process.
pub fn maybe_start() -> Option<FakeNet> {
    let fake_requested = std::env::var("HOST_LAUNCHER_FAKE_NET").is_ok_and(|v| v == "1");
    if !(fake_requested || crate::is_fresh_env()) {
        return None;
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    std::thread::Builder::new()
        .name("fake_net".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                // Serial handling is plenty: a test makes a handful of requests.
                let _ = serve_one(stream);
            }
        })
        .ok()?;
    Some(FakeNet { port })
}

fn serve_one(mut stream: std::net::TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    // Read to end-of-headers; every request we serve is a bodyless GET.
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk)?;
        if n == 0 || buf.len() > 16 * 1024 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let request = String::from_utf8_lossy(&buf);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let body = route(path);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())
}

fn route(path: &str) -> String {
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    match route {
        "/geo" => r#"{"city":"Testville","lat":37.7,"lon":-122.4}"#.to_string(),
        "/weather" => weather_body(),
        "/news" => news_body(),
        "/tz" => tz_body(query),
        _ => "{}".to_string(),
    }
}

/// Open-Meteo shape, trimmed to what weather.splash reads: 5 daily rows and
/// hourly temperatures. Values are fixed so tests can assert on them.
fn weather_body() -> String {
    let hourly_temps: Vec<String> = (0..120).map(|h| format!("{}", 10 + (h % 12))).collect();
    format!(
        r#"{{"daily":{{"time":["2026-08-14","2026-08-15","2026-08-16","2026-08-17","2026-08-18"],
"weather_code":[0,2,3,61,95],
"temperature_2m_max":[21,23,19,17,16],
"temperature_2m_min":[12,13,11,10,9]}},
"hourly":{{"temperature_2m":[{}]}}}}"#,
        hourly_temps.join(",")
    )
}

/// HN Algolia shape, trimmed to what news.splash reads.
fn news_body() -> String {
    let hits: Vec<String> = (1..=6)
        .map(|i| {
            format!(
                r#"{{"title":"Fake wire headline {i}","url":"https://example.com/story{i}","points":{},"author":"tester{i}"}}"#,
                i * 37
            )
        })
        .collect();
    format!(r#"{{"hits":[{}]}}"#, hits.join(","))
}

/// timeapi.io shape: fixed offsets per zone, so clock rows are predictable.
fn tz_body(query: &str) -> String {
    let seconds = if query.contains("Tokyo") {
        32400
    } else if query.contains("London") {
        3600
    } else if query.contains("New_York") {
        -14400
    } else {
        0
    };
    format!(r#"{{"currentUtcOffset":{{"seconds":{seconds}}}}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_server_serves_all_routes() {
        // SAFETY: test-local env flip; the server only reads it once here.
        unsafe { std::env::set_var("HOST_LAUNCHER_FAKE_NET", "1") };
        let fake = maybe_start().expect("server starts");
        let get = |path: &str| {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", fake.port)).unwrap();
            s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
                .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        };
        assert!(get("/geo").contains("Testville"));
        assert!(get("/weather").contains("temperature_2m_max"));
        assert!(get("/news").contains("Fake wire headline 1"));
        assert!(get("/tz?timeZone=Asia/Tokyo").contains("32400"));
        assert!(get("/tz?timeZone=America/New_York").contains("-14400"));
    }
}
