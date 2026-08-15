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
| `background`      | normal  | keeping this app's home-screen tiles (and their timers) alive while you are elsewhere |
| `storage-large`   | normal  | more than the standard 16 MB in its private jail (64 MB) |

`background` and `storage-large` are enforced by the thing they describe, not
by a label: revoke the first and the app's widget tiles stop running (the
tile says so), revoke the second and writes past the standard cap start
failing. Both only constrain apps that DECLARE them — an app that never asked
keeps today's behavior, and declaring is what gives the user the switch.

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

Four surfaces, because "what may this app do?" and "who can see my
location?" are different questions and a phone answers both.

**App Info -> PERMISSIONS** is the per-app view: a row per declared
permission with a colour-coded state (green allowed, amber asks, red
blocked) and a Change button. Change opens a **choice sheet** with all three
states spelled out — Allow / Ask every time / Don't allow — so a grant is
never a one-way door, plus the tier, the app's own reason, and when it last
actually used the capability. Revoking something a *running* app holds
confirms first (it restarts the app). For apps the user owns, the sheet can
also remove the declaration outright, and an "Add a capability" picker can
grant a generated app powers its author never declared.

**The permission manager** (background menu -> "Permissions…", or Settings ->
Manage) is the per-capability view: every capability with a tally of how
many apps hold it, drilling into the list of apps that declare it — each
editable in place — plus the recent-access log.

**The prompt** offers Allow / Allow Once / Don't Allow. "Allow Once" is a
session grant: it never touches disk and is dropped when the app's isolates
are torn down (force stop, uninstall, relaunch), so it cannot silently become
forever. The choice sheet adds **Allow for 1 hour** — persisted, because a
grant that ends by itself is safe to remember, and retired by a heartbeat so
it expires on the clock rather than on your next tap.

**Strict mode** ("Ask for everything", in the manager) stops normal-tier
permissions auto-granting, so every capability has to be allowed on purpose.
It exists because an imported app otherwise arrives holding open-url, share,
files, auth and clipboard-write. **Block all** (in the sheet) shuts one app
out in a tap; **Reset every app's permissions** returns the whole table to
first-run.

**Before install**, the store and the importer both list what an app
declares (with its reasons) — declaring is not granting, and the list says
so. The settings mini-app keeps a live privacy overview via the privileged
`permissions.overview` service and can open the manager with
`permissions.open_manager`.

## Losing a capability while running

Revocation is not a restart for most permissions, so apps are told: the host
pushes the new list to every live isolate of that app AND calls the script's
optional `fn on_permissions_changed(caps)` with it (a JSON array string).
Apps re-check `host.has(...)` at use time rather than caching a boot-time
answer, so a capability stops being used the moment it is taken away, and an
affordance that no longer works is hidden or explains itself instead of
failing silently. `network` is the exception: its runtime is fixed at VM
alloc, so a change restarts the app's isolates and boot code re-runs.

## Access log and the in-use indicator

Every capability a granted app actually exercises is recorded (app,
permission, timestamp; bounded ring, collapsed per minute) and surfaced in
App Info rows, the choice sheet, and the manager's recent-activity list. It
is a privacy receipt: on-device only, no request contents. While a
capability is in use a small pill appears in the launcher chrome naming the
app and the capability, the way a phone lights a dot for the microphone.

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
