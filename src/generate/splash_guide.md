# Splash mini-app guide (host_launcher dialect)

You are writing ONE self-contained Splash script for a phone-launcher mini-app.
Splash is Makepad's small scripting DSL. THIS dialect is exactly what the
examples below use — nothing more. Do not import anything, do not invent
widgets or properties that are not shown here.

## Script shape

A script is: optional module-level state (`let`), optional functions (`fn`),
optional timers, then EXACTLY ONE root `View{...}` as the final expression.

- NO `use` imports (the host injects the prelude), NO `Root{}`, NO `Window{}`,
  NO `live_design!`, NO `sys.*` helpers, NO network access, NO file access.
- `//` line comments are allowed.
- Statements are newline-separated; no semicolons needed.

## State and reactivity — THE MOST IMPORTANT RULE

There is NO automatic re-render. Mutating a variable changes nothing on
screen. You update the UI imperatively:

- Give a widget a name with `:=` (e.g. `display := Label{...}`), then call
  setters on `ui.<name>` from handlers: `ui.display.set_text("hi")`.
- `set_text` takes a string; build strings with `+` ("" + n converts numbers).
  It works on Label, buttons, and TextInput.
- `set_visible(true/false)` works on `View{...}` containers ONLY — not on
  Label or buttons. To show/hide a label or button, wrap it in a named View
  and toggle that: `msg_wrap := View{height: Fit msg := Label{...}}` then
  `ui.msg_wrap.set_visible(false)`.
- Reading input text: `ui.my_input.text()`.
- `ui.<name>` only resolves names declared with `:=` in THIS script. Calling
  a method on a name that doesn't exist is a runtime error — double-check
  every `ui.` path against your `:=` declarations.

```splash
let count = 0
fn show(){ ui.display.set_text("" + count) }

View{
    width: Fill height: Fit flow: Down spacing: 14 padding: 16
    align: Align{x: 0.5}
    glass.H1{text: "Counter"}
    glass.Card{ width: Fill height: Fit align: Align{x: 0.5, y: 0.5} padding: 26
        display := Label{
            text: "0"
            draw_text +: { color: #x9dccff text_style: theme.font_bold{font_size: 52} }
        }
    }
    View{width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 0.5}
        glass.GlassButton{text: "−" width: 90 height: 52 on_click: || { count -= 1 show() }}
        glass.GlassButtonProminent{text: "+" width: 90 height: 52 on_click: || { count += 1 show() }}
    }
}
```

## Layout

- `View{...}` is the container. Properties: `width`/`height` (`Fill`, `Fit`,
  or a number), `flow: Down|Right|Overlay`, `spacing: N`,
  `padding: N` or `padding: Inset{top: N, bottom: N, left: N, right: N}`,
  `margin` (same forms), `align: Align{x: 0..1, y: 0..1}`.
- The root View should be `width: Fill height: Fit flow: Down` with padding.
- `ScrollYView{...}` scrolls vertically (give it a fixed `height: N`).
- Color literals: `#ffffff`, `#x9dccff` (with alpha: `#xffffff55`).

## The app runs at ANY size (split screen, resizable windows)

The host may show the app fullscreen on a phone-shaped window, in one pane of
a split screen (content as narrow as ~190 or as short as ~250), or in a wide
desktop window (~1600). Two tools make a layout survive all of that:

- Cap and center the whole column so wide windows don't stretch it: wrap the
  content in `View{width: Fill height: Fit flow: Down align: Align{x: 0.5}
  col := View{width: Fill{max: 520.0} height: Fit flow: Down ...}}`.
  `Fill{max: N}` fills the host up to N points, then stays capped.
- Define the optional hook `fn on_app_resize(w, h){ ... }` — the host calls it
  with the content box (points) on open and on every size change. Fonts and
  fixed sizes can NOT change at runtime, so pre-declare alternate layouts as
  sibling Views (`visible: false` on the non-default) and flip them here with
  `ui.<id>.set_visible(bool)`; shorten labels with `set_text`. Give every
  toggled wrapper a `:=` id, keep buttons `width: Fill` so rows compress, and
  hide a large title row when `h < 520` (the host header already names the
  app). Any value shown by two tiers must be written to both labels.

## `on_render` closures: emission notes

Emitting widgets from `if`/`else` branches, `elif` chains, `match` arms, and
`for x in xs` loops works. Recommended style that stays easiest to reason
about and debug:

- When the item count is small and fixed, prefer NO `on_render` at all:
  declare the rows statically with `:=` ids and update them via `set_text` /
  `set_visible`.
- For dynamic lists, plain `View`/`RoundedView` row roots with prototype
  children read best; value-driven layout (`let cell_h = 42.0  if compact {
  cell_h = 32.0 }`) keeps a single emission path.
- A widget as the closure's FINAL statement is committed as the last child.
- Do NOT start a statement line with a bare identifier right after a line
  ending in `}` — it can glue onto the previous statement. Read results via
  `let out = r` on a fresh line.

## Text

- CAUTION: Makepad gives `Label` a NON-ZERO default padding and margin, which
  silently offsets layouts that assume 0. Set them explicitly whenever exact
  placement matters: `Label{ padding: 0 margin: 0 ... }` (or the values you
  actually want) — never assume a Label contributes no extra space.
- `Label{ text: "..." draw_text +: { color: #ffffff text_style:
  theme.font_regular{font_size: 15} } }` — fonts: `theme.font_regular`,
  `theme.font_bold`. Note the `+:` when overriding draw_text/draw_bg.
- Shorthands from the glass kit: `glass.H1{text}`, `glass.H2{text}`,
  `glass.Body{text}`, `glass.Caption{text}` (small uppercase label),
  `glass.OptionLabel{text}`.
- Emoji work in any text. Symbol characters mostly DON'T: the app fonts have
  no ✕ ✗ ➜ ↻ or arrow glyphs (they render as empty boxes). For icons use
  emoji (🗑 ➕ ▶️), plain words, or these known-good characters: × ○ ● − ﹀ ︿

## Glass kit (liquid-glass styled widgets)

`glass.Card{...}` translucent card container; `glass.Panel{...}` heavier
panel; `glass.Group{...}` compact grouping row; `glass.ListRow{...}` a row
for lists; `glass.GlassButton{text, on_click}` regular button;
`glass.GlassButtonProminent{...}` accent button;
`glass.TextInput{empty_text, on_return: |text| ...}` input field;
`glass.GlassSlider{...}`, `glass.GlassRadio{...}`, `glass.Toggle{...}`,
`glass.Chip{text}`, `glass.Badge{text}`.
Buttons take `width`/`height` numbers, `text`, and `on_click: || {...}`.

## Handlers

- `on_click: || { ... }` on buttons; `on_return: |text| add(text)` on inputs.
- Handlers are closures; they may call your `fn`s and mutate module state.
- A widget defined with `label := ...` inside a template can be addressed
  from a handler through `ui.<outer>.<inner>` paths only if each level is
  named; keep it simple and name what you need directly.

## Timers

- `start_interval(secs, || {...})` repeats; `start_timeout(secs, || {...})`
  fires once. Assign to a discard: `let _tick = start_interval(0.1, || ...)`.
- `time_now()` returns seconds (float). Math helpers: `floor(x)`, `abs(x)`,
  `min(a,b)`, `max(a,b)`.

## Dynamic lists

Prefer a named container with `on_render` and explicit re-render calls:

```splash
let laps = []
lap_list := ScrollYView{
    width: Fill height: 220 flow: Down spacing: 6
    on_render: || {
        if laps.len() == 0 { glass.Body{text: "Nothing yet." width: Fill} }
        else {
            for lap in laps {
                glass.ListRow{ width: Fill glass.Body{text: lap width: Fill} }
            }
        }
    }
}
```

After changing the array call `ui.lap_list.render()`. Rows built inside
`on_render` must NOT carry `on_click` handlers — if rows must be tappable,
pre-declare a fixed set of row widgets (e.g. `row_0`..`row_7`, hidden via
`set_visible(false)`) and fill them from a `refresh()` function instead.

## Saving data (persistence)

Every app has its own private storage — a small sandboxed filesystem rooted
at `/` (like a phone app's private data dir). Use it so data survives the
app being closed:

```splash
let items = []

fn save(){ fs.write("/items.json", items.to_json()) }

fn load(){
    if fs.exists("/items.json") {
        let parsed = fs.read("/items.json").parse_json()
        if parsed.is_array() { items = parsed }
    }
}

let _init = load()
let _boot = start_timeout(0.05, || refresh())
```

- `fs.write(path, text)`, `fs.read(path)` → string, `fs.exists(path)`,
  `fs.append(path, text)`, `fs.remove(path)`, `fs.mkdir(path)`,
  `fs.list(path)` → array of names (dirs end with "/").
- Paths are inside YOUR app only; `/` is your app's root, quota ~1MB/file.
- Serialize with `.to_json()` on any value; parse with `"...".parse_json()`.
  Always guard the parse result (`.is_array()` / `.is_object()`) so a
  corrupt file can't crash the app.
- Call `save()` after every mutation. Call `load()` once at TOP LEVEL (as
  shown — it needs no `ui`, so it runs at eval time before any handler can
  fire and overwrite the file); defer only `refresh()`, which needs the `ui`
  handles that exist after eval. Apps that track user data (lists, notes,
  scores, settings) SHOULD persist it this way.

## Data

- Arrays: `[a, b]`, `.push(x)`, `.len()`, `.clear()`, `.retain(|x| cond)`,
  index `items[i]`, iterate `for item in items { ... }`.
- Objects: `{text: "hi" done: false}` (NO commas needed between fields),
  field access `item.done`, update-merge `items[i] += {done: true}`.
- Strings: concatenation with `+`, `.trim()`. Convert: `"" + number`.
- `if`/`else`, `return`, `let`, `+=`, `-=`, `!`, `==`, `<`, `>` as usual.

## Hard rules

1. Reply with the COMPLETE script; it must be self-contained and runnable.
2. Exactly one root `View{` as the last expression.
3. Never use: `use`, `import`, `Root`, `Window`, `live_design`, `sys.`,
   `Http`, `fetch`, `Image{`, `<` JSX `>`, CSS, or HTML.
4. Every interactive element updates the UI through `ui.<name>.set_*` /
   `.render()` calls — never assume a mutation redraws by itself.
5. Keep it small: under ~150 lines. Polished and readable at ANY host size:
   width-capped + centered for wide windows (`Fill{max: N}` + `align`), and
   usable in a narrow or short split-screen pane (`fn on_app_resize` +
   pre-declared tiers when fixed sizes must change).
