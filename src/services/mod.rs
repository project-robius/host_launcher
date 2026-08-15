//! The host-service broker: the launcher half of the `host.request(...)`
//! bridge (docs/PERMISSIONS.md). Drains queued requests from every Splash
//! isolate, applies the permission policy, does the platform work through the
//! robius crates, and answers back into the requesting isolate.
//!
//! Split of responsibilities with `App`: the broker decides and executes
//! everything it can locally; anything that must touch launcher state or
//! widgets (showing a prompt, delivering IPC into other isolates, badge
//! counts) is returned as a [`BrokerAsk`] for `App::handle_actions`-side code
//! to perform. Robius callbacks land on other threads, so they come home
//! through an mpsc channel drained on the next event pass (`SignalToUI` wakes
//! one promptly).

pub mod fake_net;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use makepad_widgets::splash_host::{
    splash_host_respond, take_splash_host_requests, SplashHostRequest,
};
use makepad_widgets::*;

use crate::app::AppState;
use crate::mini_apps::registry::MiniAppId;
use crate::permissions::{Effective, Permission};

/// Where a service answer must go: one isolate, one request.
#[derive(Clone, Copy, Debug)]
pub struct Reply {
    pub heap_key: usize,
    pub req_id: u64,
}

impl Reply {
    fn of(req: &SplashHostRequest) -> Reply {
        Reply { heap_key: req.heap_key, req_id: req.req_id }
    }
}

/// Answers one request. `Ok` carries the `data` JSON, `Err` the user-visible
/// error string.
pub fn respond(cx: &mut Cx, reply: Reply, result: Result<&str, &str>) {
    let outcome = splash_host_respond(cx, reply.heap_key, reply.req_id, result);
    if std::env::var("HOST_LAUNCHER_TRACE_SERVICES").is_ok() {
        let line = format!(
            "respond heap={} req={} ok={} outcome={:?}\n",
            reply.heap_key,
            reply.req_id,
            result.is_ok(),
            outcome
        );
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/host_launcher_services.log")
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// Work only `App` can do, returned from [`Broker::process`].
pub enum BrokerAsk {
    /// Queue a runtime-permission prompt. `request`, when present, is parked
    /// until the user answers (re-dispatch on allow, deny-respond on deny).
    Prompt {
        app_id: MiniAppId,
        perm: Permission,
        request: Option<SplashHostRequest>,
    },
    /// Deliver an (already policy-checked) IPC message to `to`'s running
    /// isolates, then answer `reply` with the delivered count. `from_heap`
    /// identifies the SENDING isolate so a `to: "self"` broadcast doesn't
    /// echo the message back to its own sender.
    IpcDeliver {
        reply: Reply,
        from: MiniAppId,
        from_heap: usize,
        to: MiniAppId,
        data_json: String,
    },
    /// Change the notification badge on an app's icon. Resolved against live
    /// state at APPLY time, so several bumps in one drained batch stack.
    Badge { app_id: MiniAppId, op: BadgeOp },
    /// An app actually USED a capability: record it for the access log and
    /// light the in-use indicator. Only real, policy-passed uses reach here.
    Used { app_id: MiniAppId, perm: Permission },
    /// Put the launcher's permission manager on screen (Settings only).
    OpenPermissionManager,
}

pub enum BadgeOp {
    Set(u64),
    Bump,
    Clear,
}

/// Async results coming home from robius callbacks on other threads.
enum Completion {
    Respond(Reply, Result<String, String>),
    /// CoreLocation produced a fix: answer every waiting location request.
    LocationFix { lat: f64, lon: f64 },
    /// CoreLocation failed (denied / unavailable): fall back to IP geolocation.
    LocationFailed,
}

pub struct Broker {
    tx: Sender<Completion>,
    rx: Receiver<Completion>,
    /// Kept alive so CoreLocation's delegate keeps reporting; created lazily
    /// on first use (must be on the main thread, which `process` is).
    location_manager: Option<robius_location::Manager>,
    /// Location requests waiting on CoreLocation or the IP fallback.
    pending_locations: Vec<Reply>,
    /// In-flight host-side IP-geolocation fetches, by request LiveId.
    pending_geo: HashMap<LiveId, Reply>,
    /// The offline stand-in server (headless tests / HOST_LAUNCHER_FAKE_NET=1).
    fake: Option<fake_net::FakeNet>,
}

/// Endpoint set handed to apps via the permission-free `env` service. Env
/// vars override, the fake server overrides defaults, production URLs last.
struct EnvUrls {
    weather: String,
    news: String,
    geo: String,
    tz: String,
}

impl Broker {
    pub fn new() -> Broker {
        let (tx, rx) = channel();
        Broker {
            tx,
            rx,
            location_manager: None,
            pending_locations: Vec::new(),
            pending_geo: HashMap::new(),
            fake: fake_net::maybe_start(),
        }
    }

    fn env_urls(&self) -> EnvUrls {
        let base = self.fake.as_ref().map(|f| format!("http://127.0.0.1:{}", f.port));
        let pick = |var: &str, fake_path: &str, prod: &str| {
            std::env::var(var).ok().unwrap_or_else(|| match &base {
                Some(b) => format!("{b}{fake_path}"),
                None => prod.to_string(),
            })
        };
        EnvUrls {
            weather: pick(
                "HOST_LAUNCHER_WEATHER_URL",
                "/weather",
                "https://api.open-meteo.com/v1/forecast",
            ),
            news: pick(
                "HOST_LAUNCHER_NEWS_URL",
                "/news",
                "https://hn.algolia.com/api/v1/search?tags=front_page&hitsPerPage=12",
            ),
            geo: pick("HOST_LAUNCHER_GEO_URL", "/geo", "https://ipapi.co/json/"),
            // Apps append a zone name, so the fake path must already carry
            // the query prefix or "/tzAsia/Tokyo" routes nowhere.
            tz: pick(
                "HOST_LAUNCHER_TZ_URL",
                "/tz?timeZone=",
                "https://timeapi.io/api/timezone/zone?timeZone=",
            ),
        }
    }

    /// One full pass: everything isolates asked since the last event, plus
    /// every async completion that came home. Call once per `handle_event`.
    pub fn process(&mut self, cx: &mut Cx, state: &AppState) -> Vec<BrokerAsk> {
        let mut asks = Vec::new();
        for req in take_splash_host_requests() {
            self.dispatch(cx, state, req, &mut asks);
        }
        self.drain_completions(cx);
        asks
    }

    /// Re-runs a parked request after the user granted its permission.
    pub fn dispatch_after_grant(&mut self, cx: &mut Cx, state: &AppState, req: SplashHostRequest) -> Vec<BrokerAsk> {
        let mut asks = Vec::new();
        self.dispatch(cx, state, req, &mut asks);
        asks
    }

    /// Answers a parked request after the user denied its permission.
    /// `permissions.request` gets its documented `{granted: false}` shape (a
    /// denial IS its answer); everything else gets the error.
    pub fn respond_denied(cx: &mut Cx, req: &SplashHostRequest, perm: Permission) {
        if req.service == "permissions.request" {
            return respond(cx, Reply::of(req), Ok("{\"granted\": false}"));
        }
        respond(
            cx,
            Reply::of(req),
            Err(&format!("permission denied: {}", perm.as_str())),
        );
    }

    fn dispatch(
        &mut self,
        cx: &mut Cx,
        state: &AppState,
        req: SplashHostRequest,
        asks: &mut Vec<BrokerAsk>,
    ) {
        if std::env::var("HOST_LAUNCHER_TRACE_SERVICES").is_ok() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/host_launcher_services.log")
            {
                let _ = f.write_all(
                    format!(
                        "dispatch app={} svc={} heap={} req={} prompt={} grants={:?} args={}\n",
                        req.app_tag,
                        req.service,
                        req.heap_key,
                        req.req_id,
                        req.may_prompt,
                        crate::permissions::snapshot_grants_for(&req.app_tag),
                        req.args_json
                    )
                    .as_bytes(),
                );
            }
        }
        let reply = Reply::of(&req);
        // Untagged isolates (previews, validation dry-runs) get a clean
        // refusal; the tag is host-assigned, so this cannot be spoofed away.
        if req.app_tag.is_empty() {
            return respond(cx, reply, Err("host services are not available here"));
        }
        let Some(manifest) = state.registry.get(&req.app_tag).cloned() else {
            return respond(cx, reply, Err("unknown app"));
        };
        let args: serde_json::Value =
            serde_json::from_str(&req.args_json).unwrap_or(serde_json::Value::Null);

        // Same-app IPC is inside one sandbox: no permission involved.
        let ipc_target = (req.service == "ipc.send").then(|| {
            let to = args["to"].as_str().unwrap_or_default().to_string();
            if to.is_empty() || to == "self" { manifest.id.clone() } else { to }
        });
        let needs = match req.service.as_str() {
            "env"
            | "permissions.query"
            | "permissions.request"
            | "permissions.overview"
            | "permissions.open_manager" => None,
            "location.get" => Some(Permission::Location),
            "clipboard.read" => Some(Permission::ClipboardRead),
            "clipboard.write" => Some(Permission::ClipboardWrite),
            "url.open" => Some(Permission::OpenUrl),
            "share" => Some(Permission::Share),
            "notify.post" | "notify.clear" => Some(Permission::Notifications),
            "files.pick" | "files.save" => Some(Permission::Files),
            "auth.check" => Some(Permission::Auth),
            "ipc.send" if ipc_target.as_deref() == Some(manifest.id.as_str()) => None,
            "ipc.send" => Some(Permission::Ipc),
            _ => {
                return respond(cx, reply, Err(&format!("unknown service '{}'", req.service)));
            }
        };
        if let Some(perm) = needs {
            match state.permissions.effective(&manifest, perm) {
                // A capability actually being exercised — the only place a
                // "used" record can honestly come from.
                Effective::Granted => {
                    asks.push(BrokerAsk::Used { app_id: manifest.id.clone(), perm });
                }
                Effective::Denied => return Self::respond_denied(cx, &req, perm),
                Effective::Undeclared => {
                    return respond(
                        cx,
                        reply,
                        Err(&format!("permission not declared: {}", perm.as_str())),
                    );
                }
                Effective::NeedsPrompt => {
                    // Background surfaces (widget tiles) never pop consent
                    // dialogs: their Ask-state requests fail cleanly and the
                    // script falls back, exactly as docs/PERMISSIONS.md says.
                    if !req.may_prompt {
                        return Self::respond_denied(cx, &req, perm);
                    }
                    asks.push(BrokerAsk::Prompt {
                        app_id: manifest.id.clone(),
                        perm,
                        request: Some(req),
                    });
                    return;
                }
            }
        }

        match req.service.as_str() {
            "env" => {
                let urls = self.env_urls();
                let data = serde_json::json!({
                    "app_id": manifest.id,
                    "weather_url": urls.weather,
                    "news_url": urls.news,
                    "geo_url": urls.geo,
                    "tz_url": urls.tz,
                    "fake_net": self.fake.is_some(),
                });
                respond(cx, reply, Ok(&data.to_string()));
            }
            "permissions.query" => {
                let mut map = serde_json::Map::new();
                for (perm, _) in state.permissions.declared_states(&manifest) {
                    let s = match state.permissions.effective(&manifest, perm) {
                        Effective::Granted => "granted",
                        Effective::Denied => "denied",
                        _ => "ask",
                    };
                    map.insert(perm.as_str().to_string(), s.into());
                }
                respond(cx, reply, Ok(&serde_json::Value::Object(map).to_string()));
            }
            "permissions.request" => {
                let Some(perm) = args["perm"].as_str().and_then(Permission::from_str) else {
                    return respond(cx, reply, Err("unknown permission"));
                };
                match state.permissions.effective(&manifest, perm) {
                    Effective::Granted => respond(cx, reply, Ok("{\"granted\": true}")),
                    Effective::Denied => respond(cx, reply, Ok("{\"granted\": false}")),
                    Effective::Undeclared => {
                        respond(cx, reply, Err(&format!("permission not declared: {}", perm.as_str())));
                    }
                    Effective::NeedsPrompt if !req.may_prompt => {
                        respond(cx, reply, Ok("{\"granted\": false}"));
                    }
                    Effective::NeedsPrompt => {
                        asks.push(BrokerAsk::Prompt {
                            app_id: manifest.id.clone(),
                            perm,
                            request: Some(req),
                        });
                    }
                }
            }
            "permissions.open_manager" => {
                // Same gate as the overview: only the stock settings app can
                // put the launcher's own privacy UI on screen.
                if !(manifest.id == "settings" && manifest.builtin) {
                    return respond(cx, reply, Err("that is reserved for Settings"));
                }
                asks.push(BrokerAsk::OpenPermissionManager);
                respond(cx, reply, Ok("{}"));
            }
            "permissions.overview" => {
                // Privileged: only the stock settings app may see other apps'
                // grants. Keyed to the builtin id, not a permission.
                if !(manifest.id == "settings" && manifest.builtin) {
                    return respond(cx, reply, Err("permissions.overview is reserved for Settings"));
                }
                let apps: Vec<serde_json::Value> = state
                    .registry
                    .iter()
                    .filter(|m| !m.permissions.is_empty())
                    .map(|m| {
                        let perms: Vec<serde_json::Value> = state
                            .permissions
                            .declared_states(m)
                            .into_iter()
                            .map(|(perm, _)| {
                                serde_json::json!({
                                    "id": perm.as_str(),
                                    "title": perm.title(),
                                    "state": match state.permissions.effective(m, perm) {
                                        Effective::Granted => "granted",
                                        Effective::Denied => "denied",
                                        _ => "ask",
                                    },
                                })
                            })
                            .collect();
                        serde_json::json!({ "id": m.id, "name": m.name, "icon": m.icon, "permissions": perms })
                    })
                    .collect();
                respond(cx, reply, Ok(&serde_json::json!({ "apps": apps }).to_string()));
            }
            "location.get" => self.location_get(cx, reply),
            "clipboard.read" => {
                // Off-thread: pbpaste is usually instant but it IS a child
                // process, and a wedged pasteboard must not stall the UI.
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let out = read_clipboard()
                        .map(|text| serde_json::json!({ "text": text }).to_string());
                    tx.send(Completion::Respond(reply, out)).ok();
                    SignalToUI::set_ui_signal();
                });
            }
            "clipboard.write" => {
                let Some(text) = args["text"].as_str() else {
                    return respond(cx, reply, Err("clipboard.write needs {text}"));
                };
                cx.copy_to_clipboard(text);
                respond(cx, reply, Ok("{}"));
            }
            "url.open" => {
                let Some(url) = args["url"].as_str() else {
                    return respond(cx, reply, Err("url.open needs {url}"));
                };
                // Scheme allowlist: `file:` and friends reach places a
                // sandboxed app must not send the user.
                let ok_scheme = ["http://", "https://", "mailto:"]
                    .iter()
                    .any(|s| url.starts_with(s));
                if !ok_scheme || url.len() > 2048 {
                    return respond(cx, reply, Err("only http(s) and mailto links can be opened"));
                }
                match robius_open::Uri::new(url).open() {
                    Ok(()) => respond(cx, reply, Ok("{}")),
                    Err(e) => respond(cx, reply, Err(&format!("couldn't open link: {e:?}"))),
                }
            }
            "share" => {
                let Some(text) = args["text"].as_str() else {
                    return respond(cx, reply, Err("share needs {text}"));
                };
                match robius_share::ShareSheet::new().add_text(text).share() {
                    Ok(()) => respond(cx, reply, Ok("{}")),
                    Err(e) => respond(cx, reply, Err(&format!("couldn't share: {e:?}"))),
                }
            }
            "notify.post" => {
                // Absolute count when given, else a one-notification bump.
                let op = match args["count"].as_u64() {
                    Some(n) => BadgeOp::Set(n.min(999)),
                    None => BadgeOp::Bump,
                };
                asks.push(BrokerAsk::Badge { app_id: manifest.id.clone(), op });
                respond(cx, reply, Ok("{}"));
            }
            "notify.clear" => {
                asks.push(BrokerAsk::Badge { app_id: manifest.id.clone(), op: BadgeOp::Clear });
                respond(cx, reply, Ok("{}"));
            }
            "files.pick" => {
                let tx = self.tx.clone();
                let result = robius_file_picker::FileDialog::new().pick_file(move |res| {
                    tx.send(Completion::Respond(reply, picked_to_json(res))).ok();
                    SignalToUI::set_ui_signal();
                });
                if let Err(e) = result {
                    respond(cx, reply, Err(&format!("couldn't open the file picker: {e:?}")));
                }
            }
            "files.save" => {
                let Some(name) = args["name"].as_str().filter(|n| !n.is_empty()) else {
                    return respond(cx, reply, Err("files.save needs {name, data}"));
                };
                let Some(data) = args["data"].as_str() else {
                    return respond(cx, reply, Err("files.save needs {name, data}"));
                };
                let tx = self.tx.clone();
                let bytes = data.as_bytes().to_vec();
                let result = robius_file_picker::FileDialog::new()
                    .set_file_name(name)
                    .save_data(bytes, move |res| {
                        let out = match res {
                            Ok(Some(_)) => Ok("{\"saved\": true}".to_string()),
                            Ok(None) => Ok("{\"saved\": false}".to_string()),
                            Err(e) => Err(format!("save failed: {e:?}")),
                        };
                        tx.send(Completion::Respond(reply, out)).ok();
                        SignalToUI::set_ui_signal();
                    });
                if let Err(e) = result {
                    respond(cx, reply, Err(&format!("couldn't open the save dialog: {e:?}")));
                }
            }
            "auth.check" => {
                let reason = args["reason"].as_str().unwrap_or("Confirm it's you").to_string();
                self.auth_check(cx, reply, &manifest.name, &reason);
            }
            "ipc.send" => {
                let to = ipc_target.unwrap_or_default();
                let data_json = args["data"].to_string();
                if to != manifest.id {
                    // Cross-app consent is asymmetric on purpose: SENDING is
                    // the privileged act (prompted, sender-side). RECEIVING is
                    // opted into by declaring ipc + defining the hook, and the
                    // user can still shut a receiver off by denying its ipc.
                    let Some(target) = state.registry.get(&to).cloned() else {
                        return respond(cx, reply, Err("no such app"));
                    };
                    let blocked = !target.declares(Permission::Ipc)
                        || state.permissions.state(&to, Permission::Ipc)
                            == crate::permissions::GrantState::Denied;
                    if blocked {
                        return respond(cx, reply, Err("that app doesn't accept messages"));
                    }
                }
                asks.push(BrokerAsk::IpcDeliver {
                    reply,
                    from: manifest.id.clone(),
                    from_heap: req.heap_key,
                    to,
                    data_json,
                });
            }
            _ => unreachable!("service list checked above"),
        }
    }

    /// CoreLocation first; on any failure every waiting request falls back to
    /// IP geolocation (city-level) via the host's own HTTP stack.
    fn location_get(&mut self, cx: &mut Cx, reply: Reply) {
        let already_waiting = !self.pending_locations.is_empty();
        self.pending_locations.push(reply);
        // Hermetic runs skip CoreLocation: its delegate callbacks ride the
        // main NSRunLoop, which the headless harness never pumps, so a real
        // attempt would hang the request instead of failing over.
        if self.fake.is_some() {
            return self.start_ip_geolocation(cx);
        }
        if already_waiting {
            return;
        }
        if self.location_manager.is_none() {
            self.location_manager = robius_location::Manager::new(LocationHandler {
                tx: Mutex::new(self.tx.clone()),
            })
            .ok();
        }
        let ok = self
            .location_manager
            .as_ref()
            .is_some_and(|m| m.update_once().is_ok());
        if !ok {
            // No manager (or a sync failure): go straight to the fallback.
            self.start_ip_geolocation(cx);
        }
    }

    fn start_ip_geolocation(&mut self, cx: &mut Cx) {
        let urls = self.env_urls();
        for reply in std::mem::take(&mut self.pending_locations) {
            let id = LiveId::unique();
            self.pending_geo.insert(id, reply);
            cx.http_request(id, HttpRequest::new(urls.geo.clone(), HttpMethod::GET));
        }
    }

    fn auth_check(&mut self, cx: &mut Cx, reply: Reply, app_name: &str, reason: &str) {
        let Some(policy) = robius_authentication::PolicyBuilder::new().build() else {
            return respond(cx, reply, Err("authentication is not available here"));
        };
        let text = robius_authentication::Text {
            android: robius_authentication::AndroidText {
                title: app_name,
                subtitle: None,
                description: Some(reason),
            },
            apple: reason,
            // Truncating constructor: the reason is script-supplied, and the
            // plain new() panics past Windows' length caps.
            windows: robius_authentication::WindowsText::new_truncated(app_name, reason),
        };
        let tx = self.tx.clone();
        let context = robius_authentication::Context::new(());
        let result = context.authenticate(text, &policy, move |res| {
            let out = match res {
                Ok(()) => Ok("{}".to_string()),
                Err(e) => Err(format!("not authenticated: {e:?}")),
            };
            tx.send(Completion::Respond(reply, out)).ok();
            SignalToUI::set_ui_signal();
        });
        if let Err(e) = result {
            respond(cx, reply, Err(&format!("authentication unavailable: {e:?}")));
        }
    }

    fn drain_completions(&mut self, cx: &mut Cx) {
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Completion::Respond(reply, Ok(json)) => respond(cx, reply, Ok(&json)),
                Completion::Respond(reply, Err(e)) => respond(cx, reply, Err(&e)),
                Completion::LocationFix { lat, lon } => {
                    let data = serde_json::json!({
                        "lat": lat, "lon": lon, "city": "", "source": "gps",
                    })
                    .to_string();
                    for reply in std::mem::take(&mut self.pending_locations) {
                        respond(cx, reply, Ok(&data));
                    }
                }
                Completion::LocationFailed => self.start_ip_geolocation(cx),
            }
        }
    }

    /// Host-side network completions (the IP-geolocation fallback). Call from
    /// `Event::NetworkResponses`; unrelated request ids are left alone.
    pub fn handle_network(&mut self, cx: &mut Cx, responses: &[NetworkResponse]) {
        for response in responses {
            let request_id = match response {
                NetworkResponse::HttpResponse { request_id, .. }
                | NetworkResponse::HttpError { request_id, .. } => *request_id,
                _ => continue,
            };
            let Some(reply) = self.pending_geo.remove(&request_id) else {
                continue;
            };
            match response {
                NetworkResponse::HttpResponse { response, .. } => {
                    let parsed = response
                        .get_string_body()
                        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                        .and_then(|v| geo_json_to_location(&v));
                    match parsed {
                        Some(data) => respond(cx, reply, Ok(&data)),
                        None => respond(cx, reply, Err("couldn't determine your location")),
                    }
                }
                _ => respond(cx, reply, Err("couldn't determine your location")),
            }
        }
    }
}

struct LocationHandler {
    tx: Mutex<Sender<Completion>>,
}

impl robius_location::Handler for LocationHandler {
    fn handle(&self, location: robius_location::Location<'_>) {
        let tx = self.tx.lock().unwrap();
        match location.coordinates() {
            Ok(coords) => {
                tx.send(Completion::LocationFix { lat: coords.latitude, lon: coords.longitude })
                    .ok();
            }
            // A fix without coordinates still has to settle the waiters, or
            // every later location.get piles behind them forever.
            Err(_) => {
                tx.send(Completion::LocationFailed).ok();
            }
        }
        SignalToUI::set_ui_signal();
    }

    fn error(&self, _error: robius_location::Error) {
        let tx = self.tx.lock().unwrap();
        tx.send(Completion::LocationFailed).ok();
        SignalToUI::set_ui_signal();
    }
}

/// Accepts every common IP-geolocation response shape (ipapi.co, ip-api.com,
/// and the fake server) and normalizes to the service's `{lat, lon, city}`.
fn geo_json_to_location(v: &serde_json::Value) -> Option<String> {
    let lat = v["lat"].as_f64().or_else(|| v["latitude"].as_f64())?;
    let lon = v["lon"].as_f64().or_else(|| v["longitude"].as_f64())?;
    let city = v["city"].as_str().unwrap_or("");
    Some(serde_json::json!({ "lat": lat, "lon": lon, "city": city, "source": "ip" }).to_string())
}

fn picked_to_json(
    res: Result<Option<robius_file_picker::PickedFile>, robius_file_picker::Error>,
) -> Result<String, String> {
    /// Read cap: results ride through a script heap as one string.
    const MAX_PICK_BYTES: u64 = 1024 * 1024;
    match res {
        Ok(None) => Ok("{\"cancelled\": true}".to_string()),
        Ok(Some(file)) => {
            // size() can be None on desktop; fall back to fs metadata so a
            // huge file is rejected BEFORE read_bytes buffers all of it.
            let size = file.size().or_else(|| {
                file.path()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .map(|m| m.len())
            });
            if size.is_some_and(|n| n > MAX_PICK_BYTES) {
                return Err("file is too large (1MB max)".to_string());
            }
            let bytes = file.read_bytes().map_err(|e| format!("couldn't read the file: {e:?}"))?;
            if bytes.len() as u64 > MAX_PICK_BYTES {
                return Err("file is too large (1MB max)".to_string());
            }
            let text = String::from_utf8(bytes)
                .map_err(|_| "only text files are supported for now".to_string())?;
            let name = file.display_name().unwrap_or("file").to_string();
            Ok(serde_json::json!({
                "name": name,
                "size": text.len(),
                "text": text,
            })
            .to_string())
        }
        Err(e) => Err(format!("file picker failed: {e:?}")),
    }
}

/// Clipboard read has no makepad API; on macOS the system `pbpaste` is the
/// host-side (never script-side) door. 64KB cap keeps a giant clipboard from
/// flooding a script heap; bytes are capped BEFORE the lossy conversion so a
/// multibyte boundary can never panic a String::truncate.
fn read_clipboard() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/pbpaste")
            .output()
            .map_err(|e| format!("clipboard unavailable: {e}"))?;
        let mut bytes = out.stdout;
        bytes.truncate(64 * 1024);
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("clipboard read is not available on this platform".to_string())
    }
}
