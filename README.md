# Grimoire — tibidibi's development fork

This is a personal development fork of **[Grimoire](https://github.com/ganten1998/grimoire-scribe)**,
the handwriting notebook for the jailbroken Kindle Scribe by ganten1998. It is not the official
project and publishes no builds.

**Looking for the app?** Get it from the official project: the
[releases](https://github.com/ganten1998/grimoire-scribe/releases), the project's KPM feeds, or
[Codeberg](https://codeberg.org/ganten1998/grimoire-scribe). Report bugs and request features
[upstream](https://github.com/ganten1998/grimoire-scribe/issues), not here.

## What this fork is for

Work on changes meant to go back to Grimoire as pull requests. Each change lives on its own
branch cut from upstream `main` and is opened as a PR against
[ganten1998/grimoire-scribe](https://github.com/ganten1998/grimoire-scribe/pulls).

- `main` follows upstream `main` and carries no changes of its own.
- Other branches are proposed changes. Until upstream merges them, treat them as unfinished
  and untested.

## What Grimoire is

A handwriting notebook for the jailbroken Kindle Scribe: lined, dotted, graph and Cornell paper,
sheet music and tab, pens tuned to the Scribe's pressure and tilt, folders, and export to PDF or
PNG. Everything stays on the Kindle in open files: no account, no cloud, no tracking.

For screenshots, supported Kindles and the full manual, see the upstream
[README](https://github.com/ganten1998/grimoire-scribe#readme) and
[wiki](https://github.com/ganten1998/grimoire-scribe/wiki/Home). The wiki's source pages are also
in [`wiki/`](wiki) here; the translated READMEs ([日本語](README-ja.md), [Polski](README-pl.md))
are upstream's and describe the official app.

## Building

Same as upstream: see
[Building from Source](https://github.com/ganten1998/grimoire-scribe/wiki/Building-from-Source).
Run `make check` before opening a PR, as the
[Contributing](https://github.com/ganten1998/grimoire-scribe/wiki/Contributing) page asks.

## License, name and logo

Grimoire is free software under GPL-3.0-or-later (see `LICENSE`); this fork keeps that license.
It uses [FBInk](https://github.com/NiLuJe/FBInk) by NiLuJe, and much of what it knows about the
Kindle comes from [KOReader](https://koreader.rocks) and [kindlemodding.org](https://kindlemodding.org).
Bundled fonts and components are listed with their licenses in [`assets/README.md`](assets/README.md).

The name "Grimoire" and the bat logo identify the official builds and are not covered by the GPL.
This fork distributes no builds and does not present itself as the official app or as endorsed by
the Grimoire project. If you want to support Grimoire, support the
[original project](https://github.com/ganten1998/grimoire-scribe#support-grimoire).

Kindle and Kindle Scribe are trademarks of Amazon.com, Inc. or its affiliates. Neither Grimoire
nor this fork is affiliated with, endorsed by or supported by Amazon.
