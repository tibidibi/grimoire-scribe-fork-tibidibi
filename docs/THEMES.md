# Themes

Settings > Display > **Style** lists every theme: Gothic and Classic, built into Grimoire, then the theme packages
installed from the Package Store. A tap applies one at once; settings.ini keeps it by its id (`theme=`).

A built-in theme is a `Look` in `rust/src/looks/` (one file each, listed in `LOOKS`): it may draw any piece of the
interface its own way. A **theme package** carries no code: it is Classic's drawing with the measures and choices
its `theme.ini` sets, so anyone can publish one without building Grimoire.

## A theme package

```yaml
name: Paper Cut
id: com.example.paper-cut       # the rule for a package's id (APPS.md)
version: 1.0.0
type: theme
author: Jane Doe
description: Classic's type with straight-cut corners.
license: MIT
min_grimoire_version: 2.3.0
repository: https://github.com/example/paper-cut
artifact:
  path: theme.ini
  sha256: <64 hex digits>
  size: 123                      # at most 16,384
```

The manifest follows an app's rules (APPS.md, "A manifest") for `id`, `name`, `version`, `author`,
`description`, `min_grimoire_version`, `platforms` and `artifact`. A theme asks for no `capabilities` and has no
`spec`. It is installed, updated and removed as an app is: from the Store's catalog (its **Themes** chip), from a
repository (`user/repo`) or an HTTPS `.yaml` with the header's repository button, or, while it is being made, from
a folder in `grimoire/packages/import/` (APPS.md, "Trying a package before it is published"). It lands in
`packages/themes/<id>/`, listed in `packages/themes-index.json`. At most 32 are installed.

The theme.ini is read again, and checked against the SHA-256 recorded at its install, each time the list of themes
is read (at the start, and after an install or a removal); one that changed or no longer reads is left out of the
Style list. Removing the theme in use puts Gothic back.

## theme.ini

`key = value` lines; `#` starts a comment. Every key is optional: one left out is Classic's. A key Grimoire does not
know, or a value out of its range, refuses the file at the install, with the key in the message. Sizes are px at
the Kindle Scribe's 300 dpi, scaled on another screen as Grimoire's own sizes are.

| Key | Values | Classic's | What it changes |
|---|---|---|---|
| `frame` | `round`, `cut` | `round` | the corners of every box: rounded, or cut straight as Gothic's are |
| `fonts` | `classic`, `gothic` | `classic` | Classic's type (Terminess, or the font Settings > Font picks) or Gothic's blackletter and pixel faces |
| `card` | 0 to 60 | 22 | a button's and a Settings card's corner |
| `surface` | 0 to 60 | 24 | the kit's surfaces: a popup, an app tile, an icon button, a Store card |
| `pill` | `round`, 0 to 60 | `round` | a chip's, a text field's and a select's ends |
| `tag` | `round`, 0 to 60 | `round` | a Store card's tag |
| `popup_row` | 0 to 60 | 18 | a popup's row |
| `sheet_row` | 0 to 60 | 20 | a slide-up sheet's row |
| `tooltip` | 0 to 60 | 16 | a tooltip |
| `title_scale` | 0.5 to 2.5 | 1.1 | a screen's title, times the title size |
| `sheet_title_scale` | 0.5 to 2.0 | 0.72 | a slide-up sheet's title, times the title size |
| `field_rule` | 0 to 255 | 140 | the gray of a text field's outline (0 black) |
| `caps` | `yes`, `no` | `no` | a button's label in capitals (Cancel, Create, a sheet's actions) |
| `name_face` | `regular`, `bold`, `title`, `caps` | `bold` | a notebook's or folder's name on its card |
| `heading_face` | `regular`, `bold`, `title`, `caps` | `bold` | a heading at the left of a panel |
| `label_lift` | 0 to 20 | 0 | how far a button's label under its icon is raised |
| `knob` | 12 to 40 | 26 | a slider's knob, its radius |

```ini
# Paper Cut: Classic's type, its corners cut straight, smaller and squarer.
frame = cut
card = 10
surface = 12
pill = 12
title_scale = 1.3
field_rule = 0
caps = yes
```

`tests/fixtures/themepkg/` holds this theme as a package, and `tests/scenarios/z_theme_package.qs` installs it from
the import folder, picks it and removes it.

## Apps and widgets

An app or a widget drawn with the package kit (KIT.md) draws its elements in the theme in use, a theme package's
too. `theme()` (APPS.md) and the kit's THEME metric are 0 for Gothic, 1 for Classic, and 2 and up for the others in
the Style list's order; a package that only knows the first two should draw as Classic for any other.

## In the Store's catalog

A theme's catalog entry is an app's (the package-store-contents README) with `"type": "theme"`. Its `icon` may be
the built-in `style` (Settings' Style glyph) or an image; its screenshots show Grimoire's own screens in the theme.
