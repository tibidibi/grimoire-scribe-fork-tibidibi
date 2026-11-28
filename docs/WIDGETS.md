# Writing and publishing a widget package

Widgets are small frames over the notebook: the calculator, a timer, anything a package adds. The
editor's **Widgets** button opens the catalog; a tap on a row opens a frame that can be dragged by
its title bar, minimized, resized from its bottom-right grip and closed. Its place and size are kept
in `widgets.ini` at the data root, so it comes back where it was.

This page is the developer's guide: the repository, the manifest, the artifact, testing and
publishing. The exact field rules and the installed layout are in
[FORMAT.md](FORMAT.md#widget-packages-22-issue-200); this page explains the workflow around them.

A widget's behavior lives entirely in its package: Grimoire has no widget implementations in its
source tree. The host draws the frame, routes input and saves state; the package supplies a
WebAssembly module that draws its body and answers taps. This page covers that module's ABI as
well as the repository, the manifest, testing and publishing.

## The repository

A package is one repository with the manifest at its root:

```
my-timer/
├── package.yaml
├── timer.bin
├── LICENSE
└── README.md
```

`package.yaml` is required and must be at the root. `timer.bin` is whatever single file
`artifact.path` names; the name and extension are yours. `LICENSE` and `README.md` are optional.

An install copies only two files to the Kindle: `package.yaml` and the artifact. The license is not
copied; **View license** reads it from `license_url` or from the repository when the reader asks.
The README never reaches the device. The files a widget writes at runtime belong in
`packages/widgets/<id>/state/`, which survives an update and a removal (see
[State](#state)).

The repository must be reachable without credentials. Grimoire fetches with `curl`; there is no
token, no private-repository support and no zip or tarball distribution.

## The address

The widget package manager's **Install from repository...** (the manager is opened by **Manage
widgets...** at the foot of a notebook's Widgets popup) and the Package Store's repository button
accept two forms:

| address | what is fetched |
|---|---|
| `user/repo` | GitHub: `https://api.github.com/repos/user/repo` gives the default branch, then `https://raw.githubusercontent.com/user/repo/<branch>/package.yaml` (the fallback when the API cannot be reached is `main`) |
| `https://host/path/package.yaml` | that manifest directly; any HTTPS host works |

Only `https://` is fetched, and the address may be at most 1024 characters, with no space and no
control character in it. Every redirect an answer sends must be `https://` too, and at most five are
followed. The address is taken as written: braces and brackets in it are part of the one address.
The GitHub API is unauthenticated, so a check is subject to GitHub's rate limit.

The artifact's URL is the manifest's directory plus `artifact.path`, unless `releases` has an entry
for the manifest's version, in which case that entry's `url` is used. This is why a repository can
keep its source and its built artifact in one tree, or attach the artifact to a GitHub release and
point at it. The path is joined as it is written, so one with a space in it makes an address that
is not fetched.

An update re-fetches the stored install address (`source_url`). A package installed as
`user/repo` is updated through GitHub; one installed from a full `.yaml` address is updated from
that address.

## The manifest

A complete example:

```yaml
name: Timer
id: com.example.timer
type: widget
version: 1.0.0
author: Jane Doe
description: A small timer with one countdown.
license: MIT
license_url: https://github.com/janedoe/timer/blob/main/LICENSE
min_grimoire_version: 2.2.0
platforms: [all]
repository: https://github.com/janedoe/timer
capabilities: [editor:overlay, storage:widget-local]
settings:
  - key: show_seconds
    label: Show seconds
    type: toggle
    default: true
  - key: layout
    label: Layout
    type: choice
    default: compact
    choices: [compact, large]
artifact:
  path: timer.bin
  sha256: 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
  size: 4
releases:
  - version: 1.0.0
    url: https://github.com/janedoe/timer/releases/download/v1.0.0/timer.bin
    sha256: 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
spec:
  runtime: wasm
  entry: _start
  icon: timer
  minimizable: true
  size:
    mode: fixed
    default: [220, 160]
```

| field | |
|---|---|
| `name` | required; the row and the frame's title. At most 1,023 bytes, as `version`, `author` and `description` are: what the index of installed packages keeps of each |
| `id` | required; the package's permanent identity. Lowercase letters, digits, dots and hyphens only, at most 128, no leading or trailing dot and no `..`, and not ending in `.previous` (an update keeps the replaced version's folder as `<id>.previous`, so that name is no package's). The install, update and remove paths all use it, and so does the saved layout (`widgets.ini` reads an id by this same rule, whatever its length up to 128), so never change it between versions |
| `type` | must be `widget` |
| `version` | required; `MAJOR.MINOR.PATCH`, with an optional prerelease suffix. The updater orders `-beta.N` and `-rc.N`; another suffix is accepted but is not ordered as newer. No leading zeroes in a number. At most 1,023 bytes. An update is installed only when this sorts above the installed version, so bump it for every published change |
| `author` | optional; shown in the detail page and the widget's frame metadata. At most 1,023 bytes |
| `description` | optional; shown in the list, the detail page and the frame. At most 1,023 bytes |
| `license` | optional; the SPDX name or a short name. When its text is found, the install page asks the reader to agree before installing |
| `license_url` | optional; an HTTPS page or raw file with the license text. Without it, `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `COPYING` and `COPYING.md` beside the manifest are tried, in that order |
| `min_grimoire_version` | optional; the same `MAJOR.MINOR.PATCH` form, with an optional `-beta.N` or `-rc.N` suffix. The install is refused on an older Grimoire |
| `platforms` | optional; `all` (the default when the key is omitted), `kindle-scribe-2022`, `kindle-scribe-2024`, or a list of them. An explicit empty list is refused. `all` matches every device |
| `repository` | optional; the human-facing project address. A GitHub address also gets **View QR code** in the detail page |
| `capabilities` | required; a list that holds `editor:overlay` (the widget draws in its frame), and `storage:widget-local` as well when the widget keeps saved state. They are the two there are: any other value is refused, and so is a manifest that does not declare `editor:overlay`. A module is given only what is declared here: see [Capabilities](#capabilities). The ABI itself has no network, filesystem or process access; a module can reach only the frame, its settings and its own state |
| `settings` | optional; up to 32 settings the package declares for its own Settings page. Each has a `key`, a `label`, a `type` (`toggle` or `choice`), and a `default`; a choice also has 2 to 16 `choices`. An optional `icon` names one of the icons above |
| `artifact.path` | required; a safe relative path, at most 240 bytes: no leading `/`, no backslash, no empty, `.` or `..` component, and neither `package.yaml` nor anything under `state/`, whatever the case of their letters (`Package.yaml` and `STATE/x` are refused too: a storage that ignores case finds the manifest and the state folder by those names). No component begins with a dot, or ends in a dot, a space or `.sdr`, and the first is not the name of one of the storage's system folders (`system`, `extensions` and the like): the module is read through the walk Grimoire reads the Kindle's storage with, which takes no such name |
| `artifact.sha256` | required; 64 hexadecimal characters, over the artifact's bytes |
| `artifact.size` | required; the artifact's size in bytes, at least 1 and at most 32 MiB. It is used even when `releases` supplies the URL and digest |
| `releases` | optional; `version`, `url` (use HTTPS) and `sha256` per entry. Only the entry whose `version` equals the manifest's is used |
| `spec.runtime` | required; `wasm`. Any other value is refused |
| `spec.entry` | optional; accepted and ignored. The host calls the exports `widget_draw` and `widget_tap`, not this name |
| `spec.icon` | optional; one of Grimoire's icons: `calculator`, `package`, `clock`, `timer`/`hourglass`, `music`, `notebook`, `pdf`, `image`/`photo`, `files`/`folder`, `search`, `download`, `language`, `gear`/`settings`, `update`, `paper`/`page`, `library`, `link`, `live`, `gamepad`, `eye`, `style`/`theme`. A `md-` prefix is accepted. An unknown or omitted icon is the widget grid |
| `spec.minimizable` | optional, default `true`. Accepted for forward compatibility; this build always draws the frame's minimize button |
| `spec.size.mode` | `fixed` (default) or `resizable` |
| `spec.size.default` | optional; `[width, height]` at 300 dpi, default `[220, 160]`, within the same bounds as `min` and `max` |
| `spec.size.min`, `spec.size.max` | `resizable` only; each side must be at least 120x120 at 300 dpi and at most 70% of the editor canvas either way (about 1302x1736 on a 1860x2480 Scribe). `min` may not exceed `max`. A `fixed` widget uses `default` for all three |

Unknown top-level keys are ignored, so a newer manifest can carry fields an older Grimoire does not
know; unknown keys inside `spec` are ignored too. Values of the known fields are checked where
they affect the install: a missing name, a bad id, version, path, digest or size, an unknown
platform, capability or runtime is refused. The package page says why: each error it shows is at
most 256 bytes, cut between characters. An unknown icon falls back to the widget grid.

Limits: the manifest may be at most 256 KiB, the artifact at most 32 MiB, at most 64 packages may
be installed, and at most 8 widget frames may be open at once. A `wasm` artifact is one file; a
release asset is a single file, never an archive.

### Sizes

`spec.size` values are pixels at 300 dpi, the Scribe's panel. Grimoire scales them by the panel's
dpi when it draws the frame, so the widget is the same physical size on every device. The floor is
120x120 at 300 dpi; the ceiling is 70% of the editor canvas in either direction. A frame is placed
at the top right of the editor, cascading down as more open, and is clamped onto the screen after
a rotation or a window-size change.

`mode: resizable` gives the bottom-right grip and clamps to `min` and `max`; `mode: fixed` omits the
grip and keeps the frame at `default`.

The floor's height is what a frame is made of: the title bar (64), the gap and margin above and
below the body (22) and the smallest body (34, one line of small text). The body is the frame less
its title bar and margins, 28 narrower and 86 shorter, so the smallest frame has a body of 92x34 to
draw on. A manifest whose `default`, `min` or `max` is below the floor is refused, and a frame is
never opened, restored or resized below it, whatever an installed manifest says.

### Capabilities

A module has the capabilities its manifest declares, and no other:

| capability | what it gives the module |
|---|---|
| `editor:overlay` | drawing in its frame over the notebook: `rect`, `text`, `button` and `hit`, the drawing calls and hit regions of the package kit ([KIT.md](KIT.md)), and asking for a drawing pass with `request_redraw`. A manifest must declare it (below) |
| `storage:widget-local` | its own saved state: `state_get`, `state_set` and the file `state/runtime.bin`. Without it `state_get` returns `0`, `state_set` does nothing, and no state file is read or written for the package |

A module may import every function of the ABI whatever it declares; a call it has no capability
for returns at once. `status`, `setting` and `now` need no capability. The
package's settings are the reader's, changed on its Settings page and kept by the host with or
without `storage:widget-local`.

**A widget package declares `editor:overlay`.** A widget is a frame that its module draws in, and
without the capability nothing the module draws is shown: the frame would open with an empty body
and nothing to say why. So the manifest check refuses a manifest whose `capabilities` do not hold
`editor:overlay`, when the manifest is read and before the reader is asked anything, with

```text
`capabilities` must declare `editor:overlay`; without it nothing a widget draws is shown
```

and an update to such a manifest is refused the same way. A widget that draws and remembers
something declares both:

```yaml
capabilities: [editor:overlay, storage:widget-local]
```

A package that an earlier build installed without `editor:overlay` stays installed and still opens:
`rect`, `text`, `button`, `hit` and `request_redraw` do nothing for it, so its frame has an empty
body and no hit regions and is not drawn again for the module's asking, and Grimoire's log says that
the manifest does not declare the capability. It draws again
once it is updated to a version that declares it.

### Settings

A package can declare a Settings page of its own. When `settings` is not empty, the package's
detail page gets a **Settings...** button above **Check for updates**; opening it lists one row per
setting. A `toggle` row has a switch. A `choice` row shows the current value after its label, and
tapping it moves to the next choice. There is no separate Save button: a tap writes the new value
immediately, and **Reset all to defaults** puts every value back.

```yaml
settings:
  - key: show_seconds
    label: Show seconds
    type: toggle
    default: true
    icon: timer
  - key: accuracy
    label: Accuracy
    type: choice
    default: seconds
    choices: [seconds, tenths]
```

| setting field | |
|---|---|
| `key` | required; at most 64 characters of lowercase letters, digits, `.`, `-` and `_`; no leading or trailing dot and no `..`. It is the key the widget reads and must be unique within the package |
| `label` | required; the row's text, at most 160 bytes |
| `type` | required; `toggle` or `choice` |
| `default` | optional; `true` or `false` for a toggle (default `true`), or one of the choices for a choice (default the first) |
| `choices` | a choice needs 2 to 16 unique values, each at most 64 bytes; the row shows the chosen value |
| `icon` | optional; an icon name as in `spec.icon`; the gear when omitted |

Values are saved in `packages/widgets/<id>/state/settings.json`, beside the widget's own state, and
an update or a remove that keeps state carries them along. The file holds strings, keyed by the
setting's `key`; a missing or invalid value reads as its default. That path is reserved for the
host: a widget should keep its other data in another file under `state/`.

The module reads these values through the ABI's `setting(key_ptr, key_len, out_ptr, out_cap)`
import. It returns the number of bytes written, or `0` when the key is missing; the host caps the
copy at `out_cap`. Values are the same strings the Settings page shows (`true`, `false`, or one of
a choice's values).

A package can use settings however it likes. The test calculator package, for example, chooses the
calculator a new frame opens with and which ones its mode dropdown offers:

```yaml
settings:
  - key: default_calculator
    label: Default calculator
    type: choice
    default: Simple
    choices: [Simple, Scientific, Programming]
  - key: enable_simple
    label: Simple calculator
    type: toggle
    default: true
  - key: enable_scientific
    label: Scientific calculator
    type: toggle
    default: true
  - key: enable_programming
    label: Programming calculator
    type: toggle
    default: true
```

The package above is ordinary widget code in its own repository, not part of Grimoire. An open
frame reads its settings again after the Settings page saves them; a package decides what a change
means, and one that exports the kit's `kit_event` is told of each (a SETTINGS event,
[KIT.md](KIT.md#events)). A new frame reads the saved values on its first draw.

## The WASM ABI

The artifact is one WebAssembly module. It exports `memory`, `widget_draw(width, height,
scale_milli)` and `widget_tap(hit_id)`, and may import these functions from the module named
`grimoire`, and the package kit's from `grimoire_kit` ([KIT.md](KIT.md)): the theme's standard sizes, text by
role, and Grimoire's own elements at the theme's sizes, which an app draws with too. A kit element given an id is a
hit region, and a tap on it calls `widget_tap` with that id. The kit's calls are drawn with the module's own, in
the order it calls them, and count toward the limits below. A module may also export the kit's
`kit_event(kind, id, a, b, c) -> i32`, to be told of taps, long presses and drags in its body and of a change of
its settings ([KIT.md](KIT.md#events)).

```text
rect(x, y, w, h, style)
text(x, y, w, h, ptr, len, px, center)
button(id, x, y, w, h, ptr, len, style)
hit(id, x, y, w, h)
status(ptr, len)
request_redraw(after_ms)
setting(key_ptr, key_len, out_ptr, out_cap) -> i32
state_get(ptr, cap) -> i32
state_set(ptr, len)
now() -> i64
```

`widget_draw` is called for each frame. `width` and `height` are the body's size in screen pixels;
`scale_milli` is the UI scale times 1000 (300 dpi is 1000), so a package can keep dimensions at
300 dpi by multiplying with `scale_milli / 1000.0`. Drawing coordinates are relative to the body's
top-left.

The body is all a widget draws on. Every rectangle, text and button is clipped to it, whatever
coordinates and sizes the module passes: what reaches past an edge of the body is cut off there, and
nothing is drawn on the frame's title bar, on another frame, on the toolbar or on the page. A hit
region is the part of it that lies inside the body, and a tap in the frame's margin, beside the
body, is a tap on no region.

| import | |
|---|---|
| `rect` | a rectangle in the frame; `style` is `0` normal, `1` selected or `2` disabled. `rect`, `text`, `button`, `hit` and `request_redraw` need `editor:overlay` |
| `text` | UTF-8 text at `ptr`, `len` in the module's memory, `px` high (1 to 512: a smaller or larger value is drawn at the nearer end), centered when `center` is nonzero |
| `button` | a button with its own label; it is also a hit region with `id` |
| `hit` | a hit region with `id`, without drawing anything |
| `status` | the text the editor's `editor.widget.N.status` probe reads, for tests and debugging |
| `request_redraw` | asks for `widget_draw` again after `after_ms` (100 to 86,400,000; a value outside is taken as the nearer end); a clock calls it once per draw, and a package that never calls it is only drawn when something else repaints. One request stands at a time, the soonest asked for, whichever call asked (a draw, a tap, an event): a later call does not put an earlier one off. When the time has come the frame is drawn again and its rectangle is refreshed once, and a pass drawn then for another reason answers it. A minimized frame draws no body, so its request waits until the frame is restored, and a request waits while a finger or the pen is down or a menu or sheet is open. A redraw also waits while the pen is writing or near the screen, and comes once when the pen has left |
| `setting` | copies one saved setting's value into `out_ptr` and returns its byte length, or `0`; `out_cap` caps the copy |
| `state_get` | copies the saved state into `ptr` and returns its byte length, or `0`; `cap` caps the copy. `state_get` and `state_set` need `storage:widget-local` |
| `state_set` | replaces the saved state with the bytes at `ptr`; the first 64 KiB are kept |
| `now` | Unix time in seconds, when a widget needs a clock |

`widget_tap` is called with the `id` of the hit region under the tap, or `-1` for a tap elsewhere
in the body: at the lift of a contact that stayed within 5 mm of where it came down, held or not. It
should update the package's state; the next `widget_draw` shows the result (what a tap itself draws
is not shown: a pass holds the commands of its own `widget_draw` only). That pass is drawn once
after the tap, and only the part of the body it changed is refreshed (nothing, when the picture is the
same). A module that exports the kit's `kit_event` is told of a tap by a TAP event instead of
`widget_tap`, and of long presses and drags too, whose passes are drawn at most every 120 ms while
the contact is down, refreshed fast, and what they refreshed gets one full-depth refresh after the
lift ([KIT.md](KIT.md#events)). A widget gets no keys: they stay the notebook's
shortcuts. A
module may call `state_set` during a tap or a draw; the host writes the state file after the call,
at `packages/widgets/<id>/state/runtime.bin`. For a timer or a sticky note, that is the whole
persistence model.

Each call runs with a fixed instruction budget, from which the host's work for the imports it
calls is paid as well ([Limits](#limits)), and a module has one linear memory, which may grow to at
most 32 MiB. A module that loops past the budget is stopped and its frame shows the error instead of
freezing Grimoire. Imports outside the `grimoire` and `grimoire_kit` modules fail to link, a call of
`grimoire_kit` this Grimoire does not have keeps the module from starting (KIT.md, Levels), and the module has no
filesystem, network or process access.

### Limits

The host calls a module's `widget_draw` (a drawing pass), its `widget_tap` and its `kit_event`. Each
call is held to the limits below, and the counts start again with every call:

| limit | |
|---|---|
| a call's budget | 20,000,000, counted in instructions: the module's own, and the host's work for its imports ([below](#what-a-calls-budget-pays-for)) |
| the module's memories | 1, of at most 32 MiB (512 pages) |
| the module's tables | 1, of at most 65,536 entries |
| drawing commands in one pass (`rect`, `text`, `button` and the kit's drawing calls together) | 512 |
| hit regions in one pass (`button`, `hit` and the kit's together) | 512 |
| bytes of one text (a `text`'s, a `button`'s label, and each text the kit reads) | 1,024 |
| bytes of all the texts of one pass | 16,384 (16 KiB) |
| bytes of the status line | 256 |
| bytes `state_get` and `state_set` move in one call, all their uses together | 1,048,576 (1 MiB) |
| the saved state | 65,536 bytes (64 KiB): `state_set` keeps the first 64 KiB of what it is given |
| a text's height, `px` | 1 to 512; a value outside is drawn at the nearer end |
| `request_redraw`'s delay | 100 ms to 86,400,000 ms (24 hours); a value outside is taken as the nearer end, so a package has itself drawn again at most ten times a second |
| a key passed to `setting` | 64 bytes, the longest a setting's key can be; a longer one is no setting's and returns `0` |

A length is compared with its limit before the host reads the module's memory. A call that goes past
the command, hit region, text, status line or state limit is stopped the way a call that runs past
its instruction budget is: the frame shows the same error in place of its body (as much of it as the
body has room for), the module is not called again until the frame is closed and opened, and
Grimoire's log names the limit.

A module has one linear memory and at most one table. Memory past 32 MiB and table entries past
65,536 are not given: `memory.grow` and `table.grow` fail, as WebAssembly defines it (they return
`-1`), and the module runs on. A module that declares a second memory or a second table, or one that
starts larger than its limit, is not started: its frame says that the widget could not start.

The limits leave room. The calculator package of `tests/fixtures/widgetcalc/` draws at most 39
commands, 37 hit regions and 118 bytes of text in a pass, with a status line of 47 bytes. Its largest
call takes about 300,000 from its budget in the frame it opens with (1.5%; about 180,000 of them are
its own instructions) and about 430,000 in its largest frame.

#### What a call's budget pays for

A call's budget is counted in instructions, and it pays for two things: the instructions the module
runs, and the work the host does for the imports the module calls. The second is no instruction of
the module's, so each import takes a charge for it from the same budget: a fixed one for every call,
and one by size where the work grows with what the import is given.

| import | what a call of it takes from the budget |
|---|---|
| every import | 500 |
| `rect` | and 1 for every 4 pixels of the rectangle that lie inside the body |
| `button` | and 1 for every 2 pixels of the button that lie inside the body; and its label as a `text` 48 pixels high, with all of its bytes counted in the square |
| `text` | and, for each byte, 32 for each pixel of its height (1 to 512) up to 64 pixels; above 64 pixels, half the square of the height for each byte, since a tall letter costs by its area; and 16 times the square of its bytes, counted to 256 bytes (a line that does not fit is cut short, and a label is broken in two, by steps that each measure the text again) |
| `status`, `setting`, `state_get`, `state_set` | and 4 for each byte read from the module's memory or written to it |

The package kit's calls ([KIT.md](KIT.md)) are paid from the same budget, as KIT.md's "What a call's
budget pays for" says.

A call whose budget cannot pay a charge ends exactly as a call that ran out of instructions: its frame
shows the same error, and nothing of the pass is drawn. A call of an import the package has no
capability for pays the fixed charge and returns.

The charges were set by measuring, in a release build on a desktop machine, what each import's work
costs beside a module's own instructions. There, the longest drawing measured that a budget pays for
takes about four times as long as the same budget spent on instructions alone (4.1): one label of 512
one-letter words that Grimoire's label rule balances on two lines, a FIT text, a row or a checkbox of
the package kit, in Literata; about two and a half times in Gothic, and about once in Terminess. The
widget's own `button` with such a label, or with one word of 1,023 letters, takes about once at most.
They have not been measured on a Kindle.

Some sums, for a sense of scale:

- a clock's `12:34` as high as a text may be, 512 pixels: 5 x 512 x 512 / 2 + 16 x 25 + 500, about
  656,000 (3% of the budget);
- a line of 75 letters 34 pixels high: 75 x 34 x 32 + 16 x 75 x 75 + 500 = 172,100; a body full of
  them, 41 lines: about 7,100,000 (35%);
- a key of 120x120 pixels with a label of 3 bytes: 500 + 7,200 + 3 x 48 x 32 + 16 x 9 = 12,452;
  a hundred of them: about 1,250,000 (6%);
- a rectangle over the whole of the largest body (1274x1650): about 526,000, so a pass has room for
  38 of them, not 512; and 16 KiB of text is paid for only when it is small and in short pieces
  (512 texts of 32 letters fit the budget up to 20 pixels high).

A module must be self-contained: the same bytes run on every supported Scribe, so there is nothing
to cross-compile for armv7. A Rust `cdylib` built for `wasm32-unknown-unknown` with `extern "C"`
imports and `#[no_mangle] pub extern "C"` exports is one working toolchain; any language that
produces a plain WebAssembly module works.

## The artifact

The installer downloads the artifact (at most 32 MiB, with a 45-second timeout), checks its length
and SHA-256 against the manifest, and only then writes anything. A mismatch refuses the install
with nothing changed. There is no signature and no inspection beyond that check; the module is
first instantiated when its frame opens.

Each time a frame opens, before the module is loaded, its file is checked again:

- the installed manifest's `artifact.path` is still a path the manifest check takes, and the index
  (`packages/index.json`) names the same file, `packages/widgets/<id>/<artifact.path>`;
- it is a plain file inside the package's own folder, with no link in its place or between the
  package's folder and it;
- it is no larger than 32 MiB, and its SHA-256 is the `sha` the index recorded when the package was
  installed.

A file that fails a check is not loaded: the frame says "The widget's artifact could not be read."
in place of its body, and Grimoire's log says which check it was. (A frame whose module failed says
the reason first, in as many lines as its body holds, and under it, where lines are left, the
package's name, version and author; a body with room for one line shows the start of the reason. Each
text a widget's frame shows there is at most 256 bytes, cut between characters.) The file is read and hashed once
for each load, which is once for a frame that opens; drawing does not read it again.

`tests/fixtures/widgetcalc/` holds a compiled calculator package used by the scenarios;
`tests/fixtures/widgetpkg/` holds a small package used for the install-path checks. Neither is
built by Grimoire's build, and neither adds code to Grimoire itself.

## Packaging

Build the artifact first. Then, from a Grimoire checkout:

```sh
tools/widget-pack.sh path/to/package path/to/built.bin
```

The script copies the built file to the manifest's `artifact.path`, writes `artifact.size` and
`artifact.sha256`, and verifies the result. Run it without the second argument to check an existing
pair:

```sh
tools/widget-pack.sh path/to/package
```

In the manifest it only touches those two numbers. Bump `version` yourself, and run it again after
every rebuild.

The manifest must use the block form of `artifact:` with `path`, `sha256` and `size` present; the
script refuses an inline mapping rather than rewriting YAML it cannot see.

A release entry has its own `sha256` and URL. The script does not update it: if `releases` has an
entry for the current version, update that entry by hand (or leave it out and let the artifact be
served from the repository tree). The `artifact.size` must still describe the file at the release
URL.

### Versioning

`version` is the package's version, not the artifact's. Bump it whenever the artifact or the
manifest changes: an update is installed only when it sorts above the installed version. A
prerelease such as `1.2.0-beta.1` sorts below `1.2.0`. Use `-beta.N` or `-rc.N` for one: another
suffix passes the manifest check but the updater does not order it as newer.

Tags and GitHub releases are optional. If you use them, the release entry's `url` must be the
artifact file itself, and its `sha256` and the manifest's `artifact.size` must describe that file.
A release page URL is not an artifact URL.

### License

Set `license` to the license you actually publish under. When the text is found, the install page
shows it and asks the reader to agree before the download starts. The text is read from
`license_url`, or from one of the conventional file names beside the manifest. If only the name is declared and no file is found, the
install still proceeds; a declared `license_url` that does not answer stops it. Keep the license
file in the repository even when you set `license_url`, so the source tree is self-contained.

## Testing

There is no interactive Rust window. The Rust build is exercised by the headless runner, which can
take PNG screenshots of what it drew. The C simulator (`make sim`) predates widgets and does not
have them. The fixtures and scenarios under `tests/` are the working examples:

- `tests/fixtures/widgetcalc/` is an installed calculator package whose artifact is a WASM module
  (`packages/index.json` plus `packages/widgets/com.grimoire.calculator/`).
- `tests/fixtures/widgetpkg/` is a small package, used for the install-path checks.
- `tests/scenarios/z_widgets.qs` opens the catalog, opens the calculator, calculates in all three
  modes, opens the unit converter, minimizes, restores, drags and closes it. Its taps name the
  package's hit regions (`widget.0.hit.N`), not controls built into Grimoire.
- `tests/scenarios/z_calculator_settings.qs` changes the calculator's default and enabled modes,
  then checks the open frame and a newly opened one.
- `tests/scenarios/z_widget_packages.qs` drives install, update, license and remove through the
  package manager.
- `tests/fixtures/kit/` holds widgets (and an app) drawn with the package kit, written by `rust/tests/kitfix`
  (`make kit-fixtures`); `tests/scenarios/z_kit_widget.qs` opens them in both themes and taps their kit
  elements by name, and `rust/tests/test_kit.rs` holds the kit's rules ([KIT.md](KIT.md), Testing).
- `tests/fixtures/widgethost/` holds six small packages for the host's own rules. Their modules are
  written byte by byte by `rust/tests/widgetfix` (`make widget-fixtures` writes the folder, and
  `rust/tests/test_widget_fixtures.rs` checks it is what the writer makes), so each can pass the
  host exactly the numbers its rule is about.
- `tests/scenarios/z_widget_host.qs` opens each of them: one that draws far outside its body, one
  that declares no capability, one a command past a pass's limit, one whose file is not the one
  installed, one resized to the smallest frame, and one whose id is 128 bytes long.
  `z_widget_host_kept.qs` starts Grimoire again on that data and finds the last one's frame.
- `tests/fixtures/widgetruns/` holds the packages whose modules run for long or again and again,
  written the same way, and `tests/scenarios/z_widget_runs.qs` opens them with app time on the
  script's clock: one whose drawing pass never ends, which is stopped when its instruction budget is
  used up and whose frame, the default size, says the reason first; and one that asks to be drawn again each second, whose passes and refreshes are counted
  while its frame is open and while it is minimized, and again in a package that declares no
  capability.
- `tests/scenarios/z_widget_pen.qs` opens the one that asks to be drawn again each second and brings
  the pen: no pass and no refresh of the frame while the pen is near, while it writes, and after a
  lift with the pen still near; one pass and one refresh when the pen has left; and a pen's tap on
  the frame, which is drawn at its lift.
- `rust/tests/test_widget_host.rs` holds the same rules as unit tests of the host, pixel by pixel
  and limit by limit.

A scenario can copy a fixture into a fresh data root:

```
# copy: tests/fixtures/widgetcalc .
```

To try your own package without publishing it, make a fixture with the installed layout and add its
entry to `packages/index.json`:

```sh
mkdir -p /tmp/mypkg/packages/widgets
cp -r path/to/my-timer /tmp/mypkg/packages/widgets/com.example.timer
# then add the entry to /tmp/mypkg/packages/index.json (see FORMAT.md)
```

The entry's `path` is relative to the data root (`packages/widgets/<id>/<artifact.path>`), `sha` is
the installed artifact's digest, and `source_url` is the address an update should re-fetch. Both
`path` and `sha` are checked when the frame opens (see [The artifact](#the-artifact)): with a `sha`
that is not the file's SHA-256 the module is not loaded. Then
run a scenario with `# copy: /tmp/mypkg .` (or copy it to `GRIMOIRE_ROOT` yourself) and inspect the
shots:

```sh
make scenarios SCENARIOS="tests/scenarios/z_widgets.qs" SCEN_JOBS=1
```

The runner writes screenshots under `build/shots/<scenario>/`. `make headless` builds the runner.
Useful environment for widget checks:

| variable | |
|---|---|
| `GRIMOIRE_ROOT=DIR` | the data root (default `./grimoire-data` off the Kindle) |
| `GRIMOIRE_WIDGET_PLATFORM=kindle-scribe-2022` | stands in for a device's model, for `platforms` checks; off the Kindle it is otherwise `unknown` |
| `GRIMOIRE_SCRIPT=FILE` | the scenario to run in a direct `grimoire-headless` invocation |
| `GRIMOIRE_SHOTS=DIR` | where screenshots go in a direct invocation |

The platform values are `all`, `kindle-scribe-2022` and `kindle-scribe-2024`. On a Kindle the model
comes from `/proc/device-tree/model`; the environment variable is the simulator's and a test's way
to stand in for it.

The scenario scripts cannot name a row longer than 32 bytes, so `packages.row.com.grimoire.calculator`
is not a valid panel name. The package controls also accept a row's index (`packages.row.0`) and a
setting's index (`packages.setting.0`), or tap the coordinates instead, as `z_widget_packages.qs`
does.

## Publishing

1. Put `package.yaml` and the artifact at the repository root and push them to a public
   repository. GitHub's default branch is used for `user/repo`; a direct raw URL works on any
   branch.
2. Check the install from a clean data root: in a notebook, the Widgets popup's **Manage
   widgets...** → **Install from repository...**, enter `user/repo`, read the trust page and the
   license, and install.
3. Open the widget from the catalog, move and resize it, close it and reopen it. If it has a
   license, check **View license**. On a GitHub package, check **View QR code**.
4. Publish a new version by changing the artifact and `version`, re-running the packaging script,
   and pushing. Existing readers use **Check for updates** or **Update all**.
5. Do not move or rewrite a published version. If the artifact was wrong, publish a new version.

Removal deletes the installed manifest, artifact and frame, and keeps `state/`. An update keeps
the old directory as `<id>.previous` until the new one is in place; a failed write rolls back. No
package's id ends in `.previous`, so that directory is never another package's.

## State

A package that needs to remember something across launches declares `storage:widget-local` and
writes it through `state_set`; the host keeps the bytes in `packages/widgets/<id>/state/runtime.bin`. That folder is not part of the
artifact and is never overwritten by an update. Removing the package keeps it, so reinstalling
restores the widget's data. Nothing else of the package remains after a removal.

Keep state small and treat it as the reader's data: it is not backed up, not uploaded, and not
shared between packages. There is no state migration helper; a widget that changes its format must
read the old one and write the new one itself.

## Trust and safety

Installing is an explicit trust decision: the trust page names the package and its author, and the
install only starts after it is accepted. Grimoire checks that the manifest is well formed, that
the platform matches, and that the artifact's length and SHA-256 match the manifest, but it does
not review the code and cannot tell a good widget from a malicious one. Install from sources you
trust, and read the manifest before installing.

The manifest and artifact are fetched over HTTPS, and so is every redirect on the way to them (at
most five). The SHA-256 protects against a truncated or
corrupted download; it is not a signature, and it comes from the same repository as the artifact.
The check before each load compares the installed module with the digest kept in
`packages/index.json`, a file of the same data folder: it finds a module that is no longer the file
that was installed, and it is not a signature either. There is no package signing and no revocation
list.

A widget is confined to its frame, its settings and its own state file. The module has no network,
filesystem or process access; the only files the host touches for it are the artifact it was
installed from and `state/runtime.bin`. It draws in its frame's body and nowhere else on the screen;
what it shows there can still be misleading, so the trust decision remains yours.

## Checklist

- [ ] `package.yaml` at the repository root, `type: widget`
- [ ] a stable `id`, a new `version` for this publish
- [ ] an artifact at `artifact.path`, one file, at most 32 MiB
- [ ] `artifact.size` and `artifact.sha256` match it (`tools/widget-pack.sh`)
- [ ] `license` and a license file or `license_url`
- [ ] `spec.runtime: wasm`, and the module exports `memory`, `widget_draw` and `widget_tap`
- [ ] `capabilities` declares `editor:overlay` (every widget; the install is refused without it), and `storage:widget-local` for a widget that saves state
- [ ] `spec.size` within 120x120 and 70% of the canvas
- [ ] `platforms` correct, `min_grimoire_version` honest
- [ ] any `settings` keys, labels, defaults and choices are final; changing a key loses its saved value
- [ ] tested from a clean data root: install, open, move, close, update, remove; open Settings and toggle each row
- [ ] README says which Grimoire version runs the widget, and what it does
