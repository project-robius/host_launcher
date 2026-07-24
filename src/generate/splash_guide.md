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
- Other setters: `set_visible(true/false)`.
- Reading input text: `ui.my_input.text()`.

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

## Text

- `Label{ text: "..." draw_text +: { color: #ffffff text_style:
  theme.font_regular{font_size: 15} } }` — fonts: `theme.font_regular`,
  `theme.font_bold`. Note the `+:` when overriding draw_text/draw_bg.
- Shorthands from the glass kit: `glass.H1{text}`, `glass.H2{text}`,
  `glass.Body{text}`, `glass.Caption{text}` (small uppercase label),
  `glass.OptionLabel{text}`.
- Emoji work in any text.

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
5. Keep it small: under ~120 lines. Polished, readable, phone-sized layout.
