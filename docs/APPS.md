# Apps

Apps are full-screen programs packaged for Grimoire. They are installed and managed in the
**Package Store** and with **Manage apps...** at the foot of the **All apps** page, open from the
home app row or the **All apps** page, and run in a WebAssembly host that gives them the whole
display. They are a package kind of their own: an app is not a widget, and a widget is not an app.
Anyone can publish one from a GitHub repository or a HTTPS `package.yaml`, exactly as widget
packages are published.

The package manager (**Manage apps...** at the foot of the **All apps** page), trust prompt,
license prompt, settings page, update check and Remove are the same as the widget package
manager's (**Manage widgets...** at the foot of a notebook's Widgets popup). What differs is the
folder (`packages/apps` instead of `packages/widgets`), the index (`packages/apps-index.json`), the
manifest type (`type: app`) and the runtime.

## Package Store

The home app row's fourth built-in app is **Package Store**. It is a catalog of apps, widgets and themes
([THEMES.md](THEMES.md)) kept outside Grimoire, in a GitHub repository whose `index.json` names each package's manifest,
artifact, icon, screenshots and description. Opening a package shows its screenshots and metadata,
then Install downloads the manifest and artifact and runs the package manager's trust prompt,
license prompt, digest checks and installer. The Store is also where installed packages are
managed: its **Installed** tab lists every installed app and widget, an installed package's page has
**Uninstall** (and **Update to** the newer version when the catalog has one), and the header's
repository button installs a package that is not in the catalog from a GitHub repository
(`user/repo`) or an HTTPS `.yaml` address. A package's own page, with its settings, is in the
package manager: **Manage apps...** at the foot of the **All apps** page opens it for apps, and
**Manage widgets...** at the foot of a notebook's Widgets popup for widgets.

The catalog is cached at `grimoire/package-store/index.json`, and its images at
`grimoire/package-store/assets/`, so the last store still opens without a network. The Store itself
never decodes an image: the catalog's icons and screenshots are decoded by the sandboxed picture
worker, as the pictures of Files are ([DESIGN.md](DESIGN.md), "Packages' pictures"), and the
Store draws the gray pictures the worker left in `grimoire/.cache/packages/`. So an icon or a
screenshot shows a moment after its page; until then, and when the worker refuses a picture or
cannot run, a card and the detail page show the package glyph and the screenshot's frame its
glyph. A catalog icon is an `.ico`, a PNG or a JPEG of at most 256 KiB, shown at most 256 px a
side; a screenshot is a PNG or a JPEG of at most 8 MiB, kept at most 2,560 px a side. Grimoire's
catalog is [grimoire-notes/package-store-contents](https://github.com/grimoire-notes/package-store-contents); the
repository's README documents the format, and `tools/check.py` validates a catalog before it is
published. A mirror can be used with `GRIMOIRE_PACKAGE_STORE_URL`.

Nothing is read from a cache on the way: each open and each Refresh asks the catalog's address
with a query that is new every time (`?v=<time>`), and a manifest, an artifact and a license file
are fetched the same way. GitHub's raw host keeps a file it served for five minutes
(`cache-control: max-age=300`) and keys its copies by the whole address, so without that query a
catalog replaced a moment ago would not be seen. Icons and screenshots are not asked for again:
their files stay in `package-store/assets/` as they were first downloaded.

### Trying a package before it is published

A package that is still being worked on need not be in the catalog, or installed by hand from its
repository: clone the repository into `grimoire/packages/import/` (either into that folder itself,
as `git clone URL .` does, or into a folder of its own) and opening the Store installs it. The folder
holds the `package.yaml` and the artifact it names, beside it, and an app's `.ico` when its
`spec.icon` names one; the manifest's sizes and SHA-256 are checked as for any install, and the
package then shows in the Launcher or the Widgets popup and in the Store's Installed tab.

This happens each time the Store opens, and on Refresh, but only when the folder holds a newer
version than the installed one, or the same version rebuilt (its `artifact.sha256` changed). Its
state folder is kept, as an update's is. A folder older than the installed package is left alone.
What was imported, and what could not be, is said on a line under the Store's list, with the
folder's name leading a failure's message. Nothing is published by this: the catalog entry is still
what other people install.

## A manifest

```yaml
name: Example
id: com.example.app             # lowercase letters, digits, dots and hyphens
version: 1.0.0
type: app
author: Jane Doe
description: A full-screen example.
license: MIT
min_grimoire_version: 2.3.0
platforms: [all]
repository: https://github.com/example/app
artifact:
  path: app.wasm
  sha256: <64 lowercase or uppercase hex digits>
  size: 12345
capabilities: [storage:app-local, network:http]
settings:
  - key: show_clock
    label: Show clock
    type: toggle
    default: true
spec:
  runtime: wasm
  entry: _start                # ignored by the host
  icon: icon.ico               # a built-in icon name, or a bundled .ico
  icon_sha256: <64 hex digits> # required with a .ico icon
  icon_size: 12345             # required with a .ico icon
  status_bar: true             # apps only: start with the shared status bar
```

An omitted `platforms` means every supported device. The artifact is at most 32 MiB. At most 64
app packages are installed. A manifest is at most 256 KiB. The artifact is checked against its size
and SHA-256 before anything is written, and an update is staged before it replaces the old version.

Three fields follow the rules of a widget's manifest ([WIDGETS.md](WIDGETS.md#the-manifest)), and a
manifest that breaks one is refused when it is read, before anything is asked or written:

- `id` is lowercase letters, digits, dots and hyphens, at most 128, with no leading or trailing dot
  and no `..`, and does not end in `.previous`: an update keeps the replaced version's folder as
  `<id>.previous`, so that name is no package's.
- `name`, `version`, `author` and `description` are at most 1,023 bytes each, what the index of
  installed apps keeps of each.
- `artifact.path` is a safe relative path of at most 240 bytes: no leading `/`, no backslash, no
  empty, `.` or `..` component, and neither `package.yaml` nor anything under `state/`, whatever the
  case of their letters (`Package.yaml` and `STATE/x` are refused too: a storage that ignores case
  finds the manifest and the state folder by those names). No component begins with a dot, or ends
  in a dot, a space or `.sdr`, and the first is not the name of one of the storage's system folders
  (`system`, `extensions` and the like): the module is read through the walk Grimoire reads the
  Kindle's storage with, which takes no such name.

The package page and the Package Store say why a package was refused or could not be fetched: each
error they show is at most 256 bytes, cut between characters, as an app's toast is.

`spec.icon` may name one of Grimoire's built-in icons, or a `.ico` bundled beside `package.yaml`.
A bundled icon is a second package file: the installer fetches it from the manifest's folder, checks
its `spec.icon_sha256` and `spec.icon_size` (at most 256 KiB), and saves it with the artifact. Its
path is never one inside the app's `state/` folder, whatever case the manifest writes that name in
(the storage takes `State/` for `state/`). The
launcher never reads the icon file: the sandboxed picture worker decodes its PNG entries (8 bits a
channel, gray or RGB, with or without alpha; none with a palette or 16 bits) and
classic 1, 4, 8, 24 and 32-bit BMP entries (at most 256 px a side; the largest entry of at most
64 px is taken, else the smallest) to a small grayscale bitmap, kept in
`grimoire/.cache/packages/icons/`, which the home app row, All Apps, the package manager's list
and its detail page draw ([DESIGN.md](DESIGN.md), "Packages' pictures"). A built-in name still
works, and the launcher shows it (or the grid) until the icon's picture is made, and for good if
the icon file is missing, the worker refuses it, or the worker cannot run.

The index is `packages/apps-index.json`:

```json
{
  "version": 1,
  "packages": [
    {"id":"com.example.app","name":"Example","version":"1.0.0","author":"Jane Doe",
     "description":"A full-screen example.","icon":"icon.ico","path":"packages/apps/com.example.app/app.wasm",
     "sha":"…","installed_at":1,"source_url":"https://example.org/app.yaml"}
  ]
}
```

An app's `state/` folder survives an update and a Remove that keeps state. `state/settings.json`
belongs to the host (the values of the manifest's `settings` rows); put the app's own data in
another file under `state/`.

## The module's file

The installer checks the artifact's length and SHA-256 against the manifest, and only then writes
anything. Each time an app is opened, before its module is loaded, its file is checked again:

- the installed manifest's `artifact.path` is still a path the manifest check takes, and the index
  (`packages/apps-index.json`) names the same file, `packages/apps/<id>/<artifact.path>`;
- it is a plain file inside the app's own folder, with no link in its place or between the app's
  folder and it;
- it is no larger than 32 MiB, and its SHA-256 is the `sha` the index recorded when the app was
  installed.

A file that fails a check is not loaded: the app does not open, a toast says "The app's artifact
could not be read.", and Grimoire's log says which check it was. The file is read and hashed once
for each launch; drawing does not read it again. The check compares the installed module with the
digest kept in `packages/apps-index.json`, a file of the same data folder: it finds a module that
is no longer the file that was installed, and it is not a signature.

## Capabilities

A module has the capabilities its manifest declares, and no other:

| capability | what it gives the module |
|---|---|
| `storage:app-local` | its own saved state: `state_get`, `state_set` and the file `state/runtime.bin`. Without it `state_get` returns `0`, `state_set` does nothing, and no state file is read or written for the app |
| `network:http` | the `http_*` host calls. Without it `http_start` returns `-2` and no request is started |

A package may declare neither, either or both. Unknown capabilities are refused at install.

A module may import every function of the ABI whatever it declares; a call it has no capability
for returns at once. Drawing needs no capability: an app is the screen its module draws, so an app
that declares neither still draws, gets its touches and its keys, and can use every other call
(`status`, `setting`, `toast`, `close`, `request_redraw`, the clock, the battery and the Wi-Fi
state). The app's settings are the reader's, changed on its Settings page and kept by the host in
`state/settings.json` with or without `storage:app-local`; an open app that exports `kit_event` is told when the
page saves them (a SETTINGS event, [KIT.md](KIT.md#events)).

An app that keeps something and fetches something declares both:

```yaml
capabilities: [storage:app-local, network:http]
```

## Lifecycle and exports

The module exports `memory` and `app_draw`; the rest are optional:

```text
app_draw(width, height, scale_milli, elapsed_ms)
app_pointer(phase, x, y, source)
app_key(code, mods, down)
app_status_bar() -> i32
app_stop()
kit_event(kind, id, a, b, c) -> i32
```

`app_draw` is called whenever Grimoire redraws the app. Width and height are the app body in screen
pixels (the area under the status bar when it is shown), and `scale_milli` is 1000 at the normal UI
scale. `elapsed_ms` is the time since the previous drawing pass by the app's clock (the clock `now_ms`
reads, which a scenario's `clock` line holds still), for animation.

`app_pointer` receives every pointer phase in the body: `0` down, `1` move, `2` up. `source` is `0`
for touch and `1` for pen. Coordinates are body-local, so the app never needs to subtract the status
bar itself. A lift outside the body is still delivered so a contact that began in the app can end
cleanly.

A touch calls the module at once, and the drawing pass it owes is drawn once for all the input since
the last pass: none while the contact may still be a tap, so that a tap is one pass, at its lift; at
most one every 120 ms while a finger drags or is held; and only the part of the screen the pass
changed is refreshed, by every pass of the app's, the status bar's minute too. What those passes
refreshed fast while a finger was down gets one full-depth refresh after the lift
([KIT.md](KIT.md#how-a-pass-follows-input)). `kit_event`, the package kit's
optional export, is also told what the contact was: a tap (with the kit's hit region under it), a
long press or a drag with its samples ([KIT.md](KIT.md#events)).

`app_key` receives hardware keys. `code` is the Grimoire key code, `mods` its modifier bits, and
`down` is 1 for a press and 0 for a release. On the Kindle a paired keyboard and a controller are
read only while Settings > Shortcuts is on: an open app gets keys while Shortcuts is on, and none are
read with it off. While an app is open every key of a paired keyboard, and every button of a
controller, is the app's, pressed and let go (`app_key`, and a KEY event for `kit_event`), before
Grimoire's shortcuts; no shortcut acts but the two that leave the app: a key bound to `back` (Esc, in
the keys Grimoire writes) or to `home` closes the app as the two-finger swipe does (`app_stop` runs),
and does nothing more. A controller's buttons reach the app as keys too, and one leaves it only if
the controller's own bindings bind it to `back` or `home`, which none of the bindings Grimoire writes
does; a finger can always leave, by the two-finger swipe. A keyboard of Grimoire's open over the app takes the keys that type.
The kit's `key_char` gives the character a key types.

`app_status_bar` is called once, before the first frame. Returning 0 starts with the bar hidden and
1 with it shown; the manifest's `spec.status_bar` is the fallback. The app can change it at any time
with `show_status_bar`.

`app_stop` is called exactly once when the app leaves, whether by the system gesture, its own
`close()`, an update or removal. It is the safe place to flush the app's state. The host saves
`state_set` data after every call anyway, but a last network or file step belongs in `app_stop`.

## The `grimoire` ABI

All strings are UTF-8 in the module's linear memory; a `ptr, len` pair names one. Coordinates and
sizes are screen pixels in the app body. Each call of the module runs with a fixed instruction
budget, from which the host's work for the imports it calls is paid as well, and is held to the
[limits](#limits) below.

A module may also import the package kit, `grimoire_kit` ([KIT.md](KIT.md)), which a widget draws with too: the
theme's standard sizes, text by role, and Grimoire's own elements at the theme's sizes. Its commands are drawn
with the module's own, in the order it calls them, and count toward the limits below. A kit element given an id
is a hit region; the regions of the last drawing pass are what the headless runner taps by name. A call of
`grimoire_kit` this Grimoire does not have keeps the app from opening (KIT.md, Levels).

### Drawing and Grimoire's UI

```text
fill_rect(x, y, w, h, gray)
outline_rect(x, y, w, h, thickness, gray)
round_rect(x, y, w, h, radius, gray, filled, thickness)
line(x0, y0, x1, y1, width, gray)
text(x, y, w, h, ptr, len, px_milli, face, center, gray)
text_at(x, baseline, ptr, len, px_milli, face, gray)
text_width(ptr, len, px_milli, face) -> i32
text_height(px_milli, face) -> i32
icon(name_ptr, name_len, x, y, w, h, gray)
button(x, y, w, h, label_ptr, label_len, icon_ptr, icon_len, style)
chip(x, y, w, h, ptr, len, selected)
row(x, y, w, h, icon_ptr, icon_len, label_ptr, label_len, style, switch)
surface(x, y, w, h, filled, thickness)
select(x, y, w, h, caption_ptr, caption_len, value_ptr, value_len)
app_tile(x, y, w, h, icon_ptr, icon_len, label_ptr, label_len, selected)
fab(x, y, w, h, icon_ptr, icon_len)
card(x, y, w, h, style)
text_field(x, y, w, h, icon_ptr, icon_len, text_ptr, text_len, hint_ptr, hint_len)
clip(x, y, w, h)
clip_clear()
```

Gray is `0` (black) to `255` (paper white). `px_milli` is a text's height in thousandths of a pixel
(`34000` is 34 px; [Limits](#limits) has the range). `face` is 0 regular, 1 bold, 2 title, 3 caps, 4 fixed,
5 fixed bold. `style` is 0 plain, 1 selected, 2 disabled, 3 primary. `switch` is `-1` for no switch,
0 off or 1 on. The UI calls draw the same themed elements Settings, the Library and the editor use,
so an app follows the owner's Gothic/Classic theme, fonts and scale.

The body is all an app draws on. Every command is clipped to it, whatever coordinates and sizes the
module passes: what reaches past an edge of the body is cut off there, and nothing is drawn on the
status bar while it is shown. `clip` makes the part drawn on smaller, never larger: it is the part
of the rectangle that lies inside the body, and `clip_clear` gives the whole body back.

### State, settings and system

```text
state_get(ptr, cap) -> i32
state_set(ptr, len)
setting(key_ptr, key_len, out_ptr, out_cap) -> i32
status(ptr, len)
request_redraw(after_ms)
close()
show_status_bar(on)
toast(ptr, len, ms)
now() -> i64
now_ms() -> i64
time_text(ptr, cap) -> i32
date_text(ptr, cap) -> i32
battery_percent() -> i32
battery_charging() -> i32
wifi_connected() -> i32
wifi_ssid(ptr, cap) -> i32
screen_width() -> i32
screen_height() -> i32
ui_scale_milli() -> i32
theme() -> i32
status_bar_height() -> i32
```

`theme` is the theme in use: 0 Gothic, 1 Classic, 2 and up another (its place in Settings > Display >
Style).

`state_get`/`state_set` read and write `state/runtime.bin`, capped at 256 KiB; they need
`storage:app-local` ([Capabilities](#capabilities)). `setting` returns the
saved value for one manifest setting; a missing or invalid value reads as its default. `status`
sets a line a test or a debug view can read. `request_redraw` asks to be drawn again after the
delay (100 ms to 24 hours; a value outside is taken as the nearer end); timers and polling belong
behind it rather than a busy loop. `close`
asks the host to run `app_stop`, save and return Home. `toast` shows a short Grimoire message.

One redraw request stands at a time: the soonest asked for, whichever call of the module asked (a
drawing pass, a touch, a key, an event), so a later call does not put an earlier one off. When its time has
come, the app is drawn once, and the request is gone (a pass drawn then for another reason answers it): a
pass that wants another asks again. Only the part of the screen the pass changed is refreshed. A pass
that changes nothing refreshes nothing and does not count for what follows. A pass is back to back when it
comes within the pause a touch's cleaning waits (150 ms, 1.5 s with the pen near) of the app's last pass of
its own asking that refreshed something, or of its last fast refresh. One that is not back to back, while no
fast refresh is owed a clean, is a pass after a rest: it is refreshed at full depth, as Grimoire's screens
are, and owes nothing (with the pen away, a clock's tick or a poll that brought a change). Any other is
refreshed fast, and what those fast refreshes covered gets one full-depth refresh the pause after the last
of them, or after the last pass back to back that changed nothing: an animation's frames are cleaned once,
after it stops, even with a frame in it that changed nothing. With the pen near, a clock that asks every
second is back to back, so its ticks after the first are fast, cleaned once the pen has left. An app that
asks in every pass is so drawn, and the screen refreshed, at most ten times a second; an app that asks for
nothing is drawn only when it is touched, a key is pressed or Grimoire itself redraws. A refresh of an
e-ink panel takes a few hundred milliseconds and its share of the battery: ask for the next pass when there
is something new to show (the next minute for a clock, the next poll for a request in flight).

`battery_percent` returns -1 when the device cannot tell. `wifi_connected` returns 1, 0 or -1.
`wifi_ssid` returns the connected network's name, empty when there is none. `time_text` and
`date_text` write human-readable local time and date into the caller's buffer, returning the number
of bytes written. `screen_width`/`screen_height` are the full display, including any status bar;
`status_bar_height` is 0 when no bar is showing.

### HTTP

```text
http_start(method_ptr, method_len, url_ptr, url_len,
           headers_ptr, headers_len, body_ptr, body_len) -> i32
http_poll() -> i32
http_status() -> i32
http_body_len() -> i32
http_body(ptr, cap) -> i32
http_error(ptr, cap) -> i32
http_cancel()
```

`http_start` returns 0 when curl started, -1 for a bad argument or a failed start, -2 when the
manifest lacks `network:http`, and -3 while another request is in flight. The method is 1 to 16
ASCII letters, such as `GET`, `POST`, `PUT`, `PATCH`, `DELETE` or `HEAD`: lower case is read as
upper case, and white space around the method is not part of it (with it, the method's text is at
most 64 bytes). Any other method is a bad argument.

The address begins with `http://` or `https://`, is at most 4096 bytes, has no space and no
control character, and is taken as written: brackets and braces in it are part of the address.
Any other address is a bad argument: `http_start` returns -1 and nothing is sent. A space in an
address is written `%20`. Redirects are followed, at most 5 of them, and only to `http` and
`https` addresses; a request that is redirected further, or elsewhere, ends as a failure. A
request goes to the address it names: a proxy set in Grimoire's environment is not used.

Headers are one `Name: value` per line, at most 32 lines of at most 2048 bytes each (and 32 KiB
together). A header line is a name, a colon and a value: the name is one or more HTTP token
characters (the ASCII letters and digits, and ``! # $ % & ' * + - . ^ _ ` | ~``), the colon
follows the name directly, and the value has no control character. Spaces after the colon are
fine, white space around a line is not part of it, and an empty line is skipped. A request with
any other line, a longer line or more lines is refused whole: `http_start` returns -1 and nothing
is sent. A line with an empty value sends no header: curl then leaves out its own header of that
name, if it has one.

A request without a `User-Agent` line is sent with `User-Agent: Grimoire-App/2.3`. A request
with a `User-Agent` line of its own is sent with that line in its place, never with both. An app
should send its own.

A `HEAD` request has no upload: with one, `http_start` returns -1. It ends when the answer's
header lines have arrived, and those lines are its answer: `http_body` is the status line, the
header lines and the empty line behind them, as the server sent them, of each answer in turn
when redirects were followed. The length those lines state is not counted against the answer's
limit.

The upload is at most 2 MiB and is sent as it is. A request with an upload and no `Content-Type`
line of its own is sent with curl's `Content-Type: application/x-www-form-urlencoded`: an app
that uploads anything else says so with its own line.

The answer is at most 8 MiB (8,388,608 bytes). A longer answer ends
the request: at once when the server states its length, and otherwise once it has grown past the
limit, whether or not the app polls. Such a request has no status code (`http_status` is -1), no
answer, and its `http_error` is `The answer is too large.`

A request has 30 seconds, its redirects included: one that has not ended by then ends as a
failure.

`http_poll` returns 0 while the request runs and 1 when it has finished; after that `http_status`
is the HTTP status code, `http_body` is the answer and `http_error` is empty. A request that curl
ended with a failure (no connection, the time limit, a connection that closed before the whole
answer had arrived) has no status code (`http_status` is -1) and no answer, and its `http_error`
is curl's one-line failure: the part of an answer that had arrived is not given. Poll it after a
`request_redraw`; none of these calls blocks drawing or touching the screen. The host looks after
a request between the app's polls: a request that has ended keeps its outcome for the next
`http_poll`, until the next request starts (`http_start` returns 0) or `http_cancel`. An
`http_start` that returns anything else leaves that outcome as it was.

The host does not provide filesystem or shell access.

## Limits

The host calls a module's `app_draw` (a drawing pass), `app_pointer`, `app_key`, `app_status_bar`,
`app_stop` and `kit_event`. Each call is held to the limits below, and the counts start again with every call:

| limit | |
|---|---|
| a call's budget | 30,000,000, counted in instructions: the module's own, and the host's work for its imports ([below](#what-a-calls-budget-pays-for)) |
| the module's memories | 1, of at most 48 MiB (768 pages) |
| the module's tables | 1, of at most 65,536 entries |
| drawing commands in one call (the eighteen calls from `fill_rect` to `clip_clear` and the kit's drawing calls, all kinds together; `text_width` and `text_height` draw nothing and are not counted) | 4,096 |
| hit regions in one call (the kit's elements given an id, and its `hit`) | 512 |
| bytes of one text (a `text`'s, a `text_at`'s, what `text_width` measures, each label, caption, value, text and hint of `button`, `chip`, `row`, `select`, `app_tile` and `text_field`, and each text the kit reads) | 1,024 |
| bytes of all those texts of one call | 65,536 (64 KiB) |
| bytes of the status line | 256 |
| bytes `state_get` and `state_set` move in one call, all their uses together | 4,194,304 (4 MiB) |
| the saved state | 262,144 bytes (256 KiB): `state_set` keeps the first 256 KiB of what it is given |
| a toast's text | its first 256 bytes are read (a toast shows 255) |
| an icon's name | 64 bytes, longer than any icon's; a longer one is not read and names no icon, so the grid is drawn, as for any name Grimoire does not know |
| a key passed to `setting` | 64 bytes, the longest a setting's key can be; a longer one is no setting's and returns `0` |
| what `wifi_ssid` writes | 32 bytes at most, the longest a network's name is |
| `request_redraw`'s delay | 100 ms to 86,400,000 ms (24 hours); a value outside is taken as the nearer end, so an app has itself drawn again, and the screen refreshed, at most ten times a second |
| a text's height, `px_milli` | 1 to 768,000 (768 px); a value outside is taken as the nearer end, by `text`, `text_at`, `text_width` and `text_height` alike |
| an icon | 1,024 px each way; in a larger rectangle it is drawn at that size, in the middle. The icons of `button`, `row`, `app_tile`, `fab` and `text_field` are held to it too |
| a line's width | 0.1 to 4,096 px; a value outside is taken as the nearer end |
| a place or a size | any number: an edge of a rectangle and a text's place farther than 65,536 px outside the body are taken at that distance; a line is cut where it leaves that distance, and one with an end that is not a number (a NaN, an infinity) is not drawn; a thickness is taken within 131,072 px of nothing |

A length is compared with its limit before the host reads the module's memory. A call that goes
past the command, text, status line or state limit is stopped the way a call that runs past its
instruction budget is (below), and Grimoire's log names the limit. What a call records is its own:
the drawing commands of a touch, a key or an event are not kept for the next drawing pass.

A module may pass any numbers as places and sizes. What it draws with numbers near its body is
drawn exactly as given; the bounds of the last four rows lie far outside what a screen holds, and
are there so that no sum the drawing makes overflows and nothing is drawn at a size without bound.
The font has a greatest size of its own, for every caller: no glyph's bitmap is made larger than
4,096 px each way or 4 MiB in all ([DESIGN.md](DESIGN.md), Classic's fonts). An app's text and
icons, held to the sizes above, are far under it.

A module has one linear memory and at most one table. Memory past 48 MiB and table entries past
65,536 are not given: `memory.grow` and `table.grow` fail, as WebAssembly defines it (they return
`-1`), and the module runs on. A module that declares a second memory or a second table, or one that
starts larger than its limit, is not started: the launch says that the app could not start.

A call that needs more than its budget is stopped, and so is a call in which the module fails by
itself: the app is closed, Home comes back, and a toast says why ("The app stopped: all fuel
consumed by WebAssembly" for the budget). A module whose start does not end within the budget is not
started. An `app_stop` that is stopped ends the app all the same.

A function's first call also pays for making it ready to run, which the interpreter does when a
function is first called: about 7 for each byte of its code (an `app_draw` of 3,000,000 bytes of
code takes 22,000,623 from its first call's budget and 1,000,511 from each later one).

The limits leave room. The fixture app of `tests/fixtures/apppkg/` draws 3 commands and has 22 bytes
read in a pass, which takes about 175,000 from its budget (0.6%). The pass of one of every command
that `rust/tests/test_app_host.rs` draws, a page of a settings screen and a small board, is 47
commands and 348 bytes read, and takes about 2,300,000 (8%).

### What a call's budget pays for

A call's budget is counted in instructions, and it pays for two things: the instructions the module
runs, and the work the host does for the imports the module calls. The second is no instruction of
the module's, so each import takes a charge for it from the same budget: a fixed one for every
call, and one by size where the work grows with what the import is given. A charge counts the part
of a rectangle that lies inside the body: what a call draws outside it costs nothing by its area.

| import | what a call of it takes from the budget |
|---|---|
| every import | 500 |
| `fill_rect`, `outline_rect` | and 1 for every 32 pixels of the rectangle |
| `round_rect` | the same, and 2 for each pixel of it that is within its radius (half its smaller side at most), or the thickness it is given, and 2 px of its left or right side: those pixels are worked out one by one |
| `line` | and, for each row of the body the line crosses, 16 and 2 for each pixel of its width along that row |
| `text`, `text_at` | and, for each byte, 32 for each pixel of its height (a part of a pixel counts as one) up to 64 px; above 64 px, half the square of the height for each byte, since a tall letter costs by its area. In the bold, the title and the caps face, the cube of the height over 128 more for each byte: where such a face has no bold of its own, each letter is printed over again and again, more times the taller it is. A `text` wider than its room is cut short by steps that each measure it again: 16 times the square of its bytes more, counted to 256 bytes |
| `text_width` | and 32 for each byte measured |
| `icon` | and 2 for each pixel of the square of its side (the smaller side of its rectangle, 1,024 px at most), wherever it is drawn, and 4 for each byte of its name |
| `card`, `surface`, `row`, `app_tile`, `button` | and, for each frame the element draws (one for a card and a row, two for a surface and an app tile, three for a button), 1 for every 32 pixels of it and 2 for each pixel within 32 px of its left or right side. The outline of a `surface` that is not filled: within its thickness and 32 px |
| `chip`, `select`, `text_field`, `fab` | the same with round ends: two frames (one for `fab`), and 2 for each pixel within half the element's smaller side and 32 px of its left or right side |
| the texts of those elements | each as a `text` 48 px high that is fitted to its room, whether or not it is cut: 16 times the square of its bytes, counted to 256 bytes, and all its bytes for the label of a `button` or a `row`, which may be broken into two lines by steps that measure all of it |
| the icons of those elements | a `button`'s and an `app_tile`'s as an `icon` as large as the element's smaller side, and a `fab`'s as one half that size; a `text_field`'s as an `icon` as large as the field is high, whatever its width (the kit draws it that tall), 64 px at least; a `row`'s icon, a `select`'s chevron and the mark that clears a `text_field` as an icon of 64 px; a `row`'s switch 30,000; and 4 for each byte of an icon's name |
| `status`, `toast`, `setting` | and 4 for each byte read from the module's memory or written to it |
| `state_get`, `state_set`, `http_body`, `http_error` | and 1 for every 4 bytes copied |
| `time_text`, `date_text`, `battery_percent`, `battery_charging`, `http_poll` | and 20,000: each asks the system for what it answers |
| `http_start` | and, for a request that is read (the capability declared, none in flight, its lengths within their limits), 7,500,000 for the start of its curl and 1 for every 4 bytes of the request, whether or not the request then starts |

A call whose budget cannot pay a charge ends exactly as a call that ran out of instructions: the app
is closed with the same words, and nothing of the pass is drawn. A call of an import the app has no
capability for pays the fixed charge and returns. The charges are the same in both themes: where
the themes draw an element differently (a chip's ends are round in one and cut in the other), the
charge is for the one that takes more work. One charge follows the font: whether a `text` is wider
than its room, and so pays for being cut short, is measured in the font it is drawn in.

The charges were set by measuring, in a release build on a desktop machine, what each import's work
costs beside a module's own instructions. There, the longest drawing measured that a budget pays for
takes about two and a half times as long as the same budget spent on instructions alone (2.7): one
label of 512 one-letter words that Grimoire's label rule balances on two lines, a `row`'s of the
app's own or a FIT text, a row or a checkbox of the package kit, in Literata. How long a label takes
also follows the font the reader has chosen in Classic, for the same charge: that label takes about
three and a half times as long in Literata as in Terminess (1.7 times the budget's time in Gothic,
0.8 in Terminess). A loop of any one of the imports above is over in a third of the time the
budget's instructions alone take; the package kit's calls are measured in KIT.md. They have not been
measured on a Kindle.

Some sums, for a sense of scale:

- a rectangle over the whole body under the status bar (1860x2402): 500 + 139,616, about 140,000
  (0.5% of the budget);
- a line of 90 letters 34 px high: 90 x 34 x 32 + 500 = 98,420; a page full of them, 70 lines: about
  6,900,000 (23%);
- a clock's `12:34` 400 px high: 5 x 400 x 400 / 2 + 500 = 400,500 in the regular face, and
  5 x 400 x 400 x 400 / 128 = 2,500,000 more in the bold one (10% together);
- a key of 160x160 with a label of 3 bytes: 500 + 3 x (800 + 20,480) + 3 x 48 x 32 + 16 x 9 =
  69,092; a hundred of them: about 6,900,000 (23%);
- a row 1,740 px wide and 120 high with an icon, a switch and a label of 20 bytes: about 97,700;
  thirty of them: about 2,900,000 (10%);
- a round rectangle over the whole screen with corners of 30 px: about 462,000 (1.5%); one whose
  radius is half the screen, where every pixel is worked out by itself: about 9,400,000, so a call
  has room for three;
- an icon of 96 px: 500 + 96 x 96 x 2 = 18,932; one of 1,024 px: about 2,100,000 (7%).

## Returning Home

The system gesture is **two fingers swiping up from the bottom of the screen to the middle**. It
works in every app, is not configurable per app, and cannot be disabled by the app. The geometry is
a start in the bottom 28% of the screen, an end no lower than 62% and at least 12% above the start,
with at most 22% horizontal drift.

The app can also call `close()` at any time; both paths run `app_stop`, save state and return to
the Library. Removing the package while it runs does the same before deleting its files.

## Testing

- `tests/fixtures/apppkg/` holds a small installed app, the fixture `tests/scenarios/z_apps.qs` opens
  from the launcher, closes by its own `close()` and by the system swipe, and manages from the All
  apps page's **Manage apps...**.
- `tests/fixtures/apphost/` holds small apps for the host's own rules. Their modules are written
  byte by byte by `rust/tests/appfix` (`make app-fixtures` writes the folder, and
  `rust/tests/test_app_fixtures.rs` checks it is what the writer makes), so each can pass the host
  exactly the numbers its rule is about. `tests/scenarios/z_app_host.qs` opens them with app time on
  the script's clock: one whose drawing pass never ends, which is stopped when its instruction
  budget is used up, one that counts its drawing passes, one that shows the status bar and
  draws from far outside its body, and one whose first drawing pass asks for a second that never
  ends, which is closed when that pass is stopped. `tests/scenarios/z_app_redraw.qs` opens the three that ask to
  be drawn again (a second later, as soon as an app may, and only when touched) and counts their
  drawing passes and the screen's refreshes.
- `rust/tests/test_app_host.rs` holds the same rules as unit tests of the host, and compares a
  drawing pass of one of every command with the kit's own drawing, pixel for pixel.
- `tests/fixtures/kit/` holds an app (and widgets) drawn with the package kit, written by `rust/tests/kitfix`
  (`make kit-fixtures`); `tests/scenarios/z_kit_app.qs` opens it in both themes and taps its controls by name, and
  `rust/tests/test_kit.rs` holds the kit's rules ([KIT.md](KIT.md), Testing).

A scenario can copy a fixture into a fresh data root:

```
# copy: tests/fixtures/apppkg .
```

## Sharing a library

No app-specific code, names or URLs live in Grimoire. The package manager only knows
`type: app`; an app is whatever its module draws. A word game, a GitHub client, a browser or a
calculator with a full-screen mode all use this same ABI. Tests use a fixture app; production code
does not refer to any particular app.
