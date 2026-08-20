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

1. **makepad (generic bridge, zero policy)** — `widgets/src/splash_host.rs`,
   upstream since makepad/makepad#1181. Registers `mod.host` in every isolate
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

## Abuse control: when an app does not play along

Permissions answer "may this app do X". They say nothing about "may it do X
four thousand times a second", and a sandbox that contains a hostile app but
lets it wedge the launcher is only half a sandbox. `src/services/limits.rs`
answers the second question.

Nothing here is about *containment* — that is settled by the isolate itself. A
mini-app cannot reach `mod.fs` outside its jail, cannot read `mod.run` or
`mod.res` (stripped), cannot call `cx.quit`, cannot set its own `app_tag` or
`may_prompt` (host-assigned per heap), cannot grant itself anything (grants
live in the launcher's `permissions.json`, never in the app's jail), and
cannot see another app's heap. A request for something undeclared is refused
without a prompt, and a denied one is refused whether or not the script likes
the answer. The script cannot do anything about any of that.

What it *can* do is ask, endlessly. So:

- **Every request is priced.** Each app holds a token bucket (30 tokens,
  refilling 6/s). Requests the host answers from memory (`env`,
  `permissions.query`) cost 0.5; ones that touch launcher state or another
  isolate (`notify.post`, `ipc.send`, `clipboard.write`) cost 1; ones that
  leave the process (`location.get`, `clipboard.read`) cost 5; ones that put
  OS UI on screen cost 8. Ordinary use — including a burst at wake-up — never
  comes near the limit; a loop drains it in well under a second.
- **Draining the bucket costs a 3-second cooldown**, during which every
  request from that app is refused immediately, and the app comes back with a
  third of a bucket rather than a full one so a loop hits the wall again fast.
- **OS dialogs are foreground-only and one-at-a-time.** `files.pick`,
  `files.save`, `auth.check`, `share` and `url.open` are refused outright from
  a background surface (a home-screen widget the user never opened), and the
  three that stay on screen until answered cannot be stacked. Without this an
  app could trap the user in a wall of file pickers, or open dialogs from a
  tile they are not even looking at.
- **Four cooldowns and the launcher stops the app.** Refusing a request is
  still work, and an app willing to spend a whole run being refused is not
  going to stop on its own. It is force-stopped (fullscreen host and home
  tiles), marked restricted in `permissions.json`, and the user gets a modal
  saying what happened. Strikes decay after two quiet minutes, so an app is
  judged on what it is doing now.

A restricted app does not run: `effective()` returns `Denied` for every
capability it holds, its home tiles stay placeholders, and tapping its icon
re-shows the notice instead of launching it. The flag is persisted on purpose
— an app that hammered its way to a stop must not get a clean slate by being
restarted — and only the user clears it, from the notice or the App Info
banner ("Let it run again"). A full permission reset deliberately leaves
restrictions in place; freeing a stopped app is its own decision.

The `sandbox_probe` built-in has a "Flood the host with requests" button that
demonstrates the budget live: it fires 80 permission-free requests and reports
how many came back refused.

Stopping an app is also the moment the launcher is most exposed, and one thing
had to be got right for it to be safe: **the broker stops answering a
condemned app's remaining requests instead of refusing them one by one**. Every
answer is a synchronous re-entry into an isolate that is about to be torn down
in the same event pass, and a script that answers by touching its own UI leaves
paused threads and queued widget calls behind it. Dropping is the bridge's
documented behaviour for a request nobody drains — it simply never resolves —
and the parked callbacks are reaped with the isolate moments later. The same
applies to anything still queued from an app that is already restricted.

That is not a theoretical tidiness point. Before it, force-stopping a flooding
app reliably panicked the whole launcher in makepad's script GC
(`index out of bounds` in `gc.rs`), because a dead isolate's widgets were still
being routed into the app VM. That is fixed upstream in makepad
(`script_ref_vm_id` now tells a reclaimed heap apart from the app VM's and
drops its calls); the launcher-side drop is the belt to that braces, and is
what keeps this safe on a makepad that predates the fix.

## How much it may use

The request budget covers the host bridge. What an app does *inside* its own
isolate — burning the frame on a fast timer, growing a structure forever,
forty downloads at once — is a separate question with a separate answer:
`src/resources.rs` here, and `widgets/src/splash_limits.rs` in makepad.

Only the VM can meter its own execution, so the MECHANISMS are makepad's: a
per-isolate CPU allowance across a window (not just per entry), a timer count
cap and interval floor, a post-collection heap ceiling, and an in-flight
request cap. The POLICY is the launcher's: the numbers, and who may change
them.

Three layers:

1. **Defaults by surface.** A foreground app gets the full share; a
   home-screen tile gets a fraction of it, because a tile competes with
   eleven others for one frame and is by definition not what the user is
   waiting on. The same app running in both places gets both numbers.
2. **Per-app amounts**, set by the user in App Info → RESOURCES, one resource
   at a time, from exact presets. Persisted in the launcher's `resources.json`
   — never in the app's jail, for the same reason grants are not.
3. **Crossings are counted.** makepad reports each one; the launcher maps it
   back to an app and feeds it into the same strike ladder as request
   flooding. Eight crossings and the app is stopped and restricted, exactly
   as a flooder is. Memory is the exception: an isolate over its heap ceiling
   is stopped on the first crossing, because it is not going to shrink.

A crossing is coalesced per app per KIND per pass. An app that asks for thirty
timers past its cap has crossed one limit once, not thirty times; counting
each refusal separately turned one greedy loop into an instant stop.

What an app sees: a refused timer answers `nil` rather than raising, so a
script can check and cope; over-budget CPU means its entry gets what is left
of the window rather than a fresh slice; an over-cap request errors like any
other failed request.

**Nesting cannot widen privilege.** `Splash` is in a mini-app's namespace and
`allow_net` is a live property, so a script can *write*
`Splash{allow_net: true}` in its own body. Nothing evaluates such a widget
today (only the host's `set_text` triggers evaluation), so it is inert — but
one future script binding would have made it a network grant nobody gave. A
nested isolate now gets at most what the isolate creating it has.

**Known gaps.** A single long native call (a pathological regex, a huge JSON
parse) is still not interruptible: both budgets are sampled between opcodes,
and a native runs inside one. The fix there is containment rather than
preemption — strip the dangerous native — not a check on the interpreter's
hot path.

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
