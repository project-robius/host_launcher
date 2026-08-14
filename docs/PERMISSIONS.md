# Mini-app permissions and host services

How a sandboxed Splash mini-app gets at anything beyond its own UI and its
private storage jail: a mobile-OS-style permission system, deliberately a
hybrid of the three models it imitates.

- **Containers (deny-by-default):** an isolate starts with nothing. `mod.fs`,
  `mod.run`, `mod.res` are stripped, the net runtime is absent, and `fs` is a
  per-app jail. Every capability below is granted by the host, per app, per
  feature, and enforced host-side where script code can never reach.
- **Android (manifest declaration):** an app's manifest lists the permissions
  it may ever use (`permissions: ["network", "location"]`). Anything
  undeclared is not just denied, it is ungrantable: requests fail immediately
  and no prompt is ever shown. Declarations are visible in App Info before
  and after install.
- **iOS / Android-runtime (prompt at first use, revocable any time):**
  dangerous permissions prompt the user the moment the app first needs them,
  with an explanation of what the permission does. The user's answer is
  persisted per app and can be flipped at any time in App Info. Changing a
  permission while the app is running restarts the app (Android's semantics
  for runtime-permission changes; nothing keeps half-granted state).

## Permission catalog

| id                | tier    | grants access to |
|-------------------|---------|------------------|
| `network`         | runtime | the isolate's own net runtime: `net.http_request`, `net.web_socket`, `net.socket_stream` |
| `location`        | runtime | one-shot geolocation via the host (CoreLocation, IP fallback) |
| `notifications`   | runtime | posting launcher notifications / icon badge counts |
| `clipboard-read`  | runtime | reading the system clipboard |
| `ipc`             | runtime | sending messages to OTHER apps (same-app messaging is free) |
| `clipboard-write` | normal  | writing the system clipboard |
| `open-url`        | normal  | opening http(s)/mailto URLs in the system browser |
| `files`           | normal  | host file access THROUGH the system picker (the picker itself is the per-file consent, like iOS documents / Android SAF) |
| `share`           | normal  | the native share sheet |
| `auth`            | normal  | biometric/system authentication (the OS shows its own prompt) |

Tiers: a **normal** permission auto-grants when declared (still shown, still
revocable); a **runtime** permission defaults to Ask and prompts on first use.
`network` is runtime here even though Android makes INTERNET install-time,
because the whole point of this launcher is untrusted/generated apps.

Grant states per (app, permission): `Ask` (default), `Granted`, `Denied`.
Effective policy: undeclared -> denied always; declared normal -> granted
unless the user said Denied; declared runtime -> the stored state, where Ask
parks the request and prompts.

## Enforcement layers

1. **makepad (generic bridge, zero policy)** — `widgets/src/splash_host.rs` on
   the `splash_host_services` branch. Registers `mod.host` in every isolate
   (bound as `host` by the Splash prefix, like `fs`). One entry point:

   ```
   host.request("location.get", {args}, |r| { ... })  -> request id
   ```

   The bridge serializes args to JSON, queues
   `{app_tag, heap_key, req_id, service, args_json, may_prompt}` on a
   thread-local the host drains (`take_splash_host_requests`), and holds the
   callback as a `ScriptFnRef` keyed by (heap, req_id). The host answers with
   `splash_host_respond(cx, heap_key, req_id, result)`; the bridge re-enters
   the right isolate under the standard budget and calls the callback with
   `{is_ok, data, error}` — `is_ok`, NOT `ok`, because `ok` is a script
   keyword (the ok-test operator) and `r.ok` does not parse as a field
   access. Dead isolates GC their queued requests and callbacks alongside the
   storage-jail roots.

   `app_tag` and `may_prompt` are host-assigned (`Splash::set_host_tag` /
   `set_host_prompts`), so neither request identity nor prompting rights can
   be spoofed by script. `host.capabilities()` reads a host-pushed per-heap
   grant list (`Splash::set_host_caps`) so apps can adapt their UI; it is
   informational, every real decision happens host-side at response time.

   The `network` permission is NOT brokered: a granted app's isolate gets the
   real net runtime at VM alloc (`set_allow_net`), same as before, because
   the runtime is baked in at allocation. Grant/revoke while running
   therefore restarts the app's isolates.

2. **host_launcher policy + brokers** — `src/permissions.rs` (model, store,
   `<data_dir>/permissions.json`, atomic writes, uninstall cleanup) and
   `src/services/` (the broker). Every event pass drains the bridge queue:
   check declaration, check grant, then dispatch / park-and-prompt / refuse.
   Robius crates do the platform work (`robius-location`, `robius-open`,
   `robius-file-picker`, `robius-share`, `robius-authentication`); their
   off-thread callbacks come back through an mpsc + `SignalToUI`.

## Service catalog (script API)

All via `host.request(service, args, cb)`; `cb(r)` gets
`r.is_ok`, `r.data`, `r.error` (see above for why it is not `r.ok`).
Permission-free services: `env` (per-app config: app id + endpoint URLs,
overridable by env vars for tests), `permissions.query` (the app's own
declared/granted map), `permissions.request` (`{perm}`, triggers the prompt
early, answers `{granted: bool}` — a denial is an answer, not an error).

| service            | permission      | args -> data |
|--------------------|-----------------|--------------|
| `location.get`     | location        | `{}` -> `{lat, lon, city?, source: "gps"\|"ip"}` |
| `clipboard.read`   | clipboard-read  | `{}` -> `{text}` |
| `clipboard.write`  | clipboard-write | `{text}` -> `{}` |
| `url.open`         | open-url        | `{url}` (http/https/mailto only) -> `{}` |
| `notify.post`      | notifications   | `{count?, title?, body?}` -> `{}` (badge on the app's icon) |
| `notify.clear`     | notifications   | `{}` -> `{}` |
| `files.pick`       | files           | `{kind?: "file"\|"image"}` -> `{name, size, text}` (text files, capped 1MB) |
| `files.save`       | files           | `{name, data}` -> `{}` (system save dialog) |
| `auth.check`       | auth            | `{reason}` -> `{}` (ok = authenticated) |
| `share`            | share           | `{text}` -> `{}` |
| `ipc.send`         | ipc (free to self) | `{to, data}` -> `{delivered}`; receiver defines `fn on_ipc_message(from, json_string)` |
| `permissions.overview` | (settings builtin only) | `{}` -> all apps' declared permissions + states, read-only |

`ipc.send` with `to: "self"` reaches the app's OTHER running isolates (its
widget tiles, its home tile) without any permission; the app+widget pair is
one sandbox. Cross-app delivery is asymmetric: the SENDER needs its `ipc`
grant (prompted at first send), while the RECEIVER consents by declaring
`ipc` and defining `on_ipc_message` — its user can still block it by denying
its `ipc` in App Info. The target must be running to receive.

## Prompting

One prompt at a time, queued. Sources: a parked runtime-tier bridge request,
`permissions.request`, or opening a fullscreen app whose declared `network`
is still Ask (net is alloc-time, so the launcher asks as the app opens; the
app opens immediately without net, and a grant restarts it with net). The
prompt names the app, the permission, and what it means; Allow/Don't Allow
persist. Dismissing the scrim is "not now": nothing persists, parked
requests fail once, the app will ask again.

Surfaces that never prompt: home-screen widget tiles and live home app tiles
are marked `may_prompt: false`, so an Ask-state request from one FAILS
cleanly (the script falls back) instead of popping a consent dialog the user
never invited. A tile lent out to fullscreen flips to prompting for as long
as it is the foreground app. Scrim-dismissing a prompt is "not now": nothing
persists, parked requests fail once, and that (app, permission) stops asking
for the rest of the session so a looping script cannot nag its way to an
accidental Allow.

## Managing grants

App Info gains a PERMISSIONS section: one row per declared permission with
its state; tapping toggles Granted/Denied (from Ask, tap grants). Any change
force-stops the app's running isolates. Apps with no declarations show
"None - fully sandboxed". The settings mini-app shows a read-only privacy
overview through the privileged `permissions.overview` service, with App
Info as the write path.

## What deliberately did NOT change

- The fs jail, quotas, and `..`/symlink defense (see splash_storage.rs).
- Widget tiles share the app's jail and now also its grants, but never
  prompt; an Ask-state widget simply runs without the capability until the
  user answers a prompt from the fullscreen app (or App Info).
- Grants live in the launcher's `permissions.json`, never inside
  `app_data/<id>/` (an app must not be able to edit its own grants), and
  never in exported bundles (declarations travel, grants are local).
- `gallery` (real images) and `music` (real audio) stay mocked: those need
  image/audio capabilities in the Splash dialect first, not permissions.
