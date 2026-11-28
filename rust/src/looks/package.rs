//! A theme from a package (`type: theme`, themepkg.rs): Classic's drawing with the measures and choices its
//! `theme.ini` sets. A theme package carries no code: only the numbers the shared drawing already asks a Look for
//! (docs/THEMES.md lists the keys).

use super::{Corner, Look};
use crate::font::FaceMap;
use crate::prelude::*;
use std::cell::{Cell, RefCell};

/// The largest theme.ini a package may carry.
pub const THEME_FILE_MAX: usize = 16 * 1024;

/// What a theme.ini says. Px are at the Kindle Scribe's 300 dpi (scaled as Grimoire's own sizes are); `None`: Classic's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeSpec {
    /// `frame = cut`: corners cut straight, as Gothic's (else rounded)
    pub cut: bool,
    /// `fonts = gothic`: Gothic's type (else Classic's: Terminess, or the font Settings > Font picks)
    pub gothic_type: bool,
    /// card, surface, popup_row, sheet_row, tooltip (CORNER_KEYS' order)
    pub corners: [Option<f32>; 5],
    /// `pill`, `tag`: px, or `None` for round ends (half the height)
    pub pill: Option<f32>,
    pub tag: Option<f32>,
    pub title_scale: Option<f32>,
    pub sheet_title_scale: Option<f32>,
    pub field_rule: Option<u8>,
    pub caps: bool,
    pub name_face: Option<FontFace>,
    pub heading_face: Option<FontFace>,
    pub label_lift: Option<f32>,
    pub knob: Option<f32>,
}

const CORNER_KEYS: [&str; 5] = ["card", "surface", "popup_row", "sheet_row", "tooltip"];

fn num(key: &str, v: &str, lo: f32, hi: f32) -> Result<f32, String> {
    match v.parse::<f32>() {
        Ok(x) if x.is_finite() && (lo..=hi).contains(&x) => Ok(x),
        _ => Err(format!("theme.ini: `{key}` must be a number from {lo} to {hi}")),
    }
}

fn face(key: &str, v: &str) -> Result<FontFace, String> {
    match v {
        "regular" => Ok(FontFace::Regular),
        "bold" => Ok(FontFace::Bold),
        "title" => Ok(FontFace::Title),
        "caps" => Ok(FontFace::Caps),
        _ => Err(format!("theme.ini: `{key}` must be regular, bold, title or caps")),
    }
}

fn yes_no(key: &str, v: &str) -> Result<bool, String> {
    match v {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(format!("theme.ini: `{key}` must be yes or no")),
    }
}

/// Reads a theme.ini: `key = value` lines, `#` comments and blank lines. A key it does not know, or a value out of
/// its range, refuses the whole file (the package's author sees why at the install).
pub fn theme_spec_parse(text: &[u8]) -> Result<ThemeSpec, String> {
    if text.len() > THEME_FILE_MAX {
        return Err(format!("theme.ini is larger than {THEME_FILE_MAX} bytes"));
    }
    let text = std::str::from_utf8(text).map_err(|_| "theme.ini is not UTF-8 text".to_string())?;
    let mut s = ThemeSpec::default();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("theme.ini line {}: not `key = value`", n + 1));
        };
        let (k, v) = (k.trim(), v.trim().to_ascii_lowercase());
        let v = v.as_str();
        match k {
            "frame" => {
                s.cut = match v {
                    "cut" => true,
                    "round" => false,
                    _ => return Err("theme.ini: `frame` must be round or cut".into()),
                }
            }
            "fonts" => {
                s.gothic_type = match v {
                    "gothic" => true,
                    "classic" => false,
                    _ => return Err("theme.ini: `fonts` must be classic or gothic".into()),
                }
            }
            "pill" | "tag" => {
                let r = if v == "round" { None } else { Some(num(k, v, 0.0, 60.0)?) };
                if k == "pill" { s.pill = r } else { s.tag = r }
            }
            "title_scale" => s.title_scale = Some(num(k, v, 0.5, 2.5)?),
            "sheet_title_scale" => s.sheet_title_scale = Some(num(k, v, 0.5, 2.0)?),
            "field_rule" => s.field_rule = Some(num(k, v, 0.0, 255.0)? as u8),
            "caps" => s.caps = yes_no(k, v)?,
            "name_face" => s.name_face = Some(face(k, v)?),
            "heading_face" => s.heading_face = Some(face(k, v)?),
            "label_lift" => s.label_lift = Some(num(k, v, 0.0, 20.0)?),
            "knob" => s.knob = Some(num(k, v, 12.0, 40.0)?),
            _ => match CORNER_KEYS.iter().position(|&c| c == k) {
                Some(i) => s.corners[i] = Some(num(k, v, 0.0, 60.0)?),
                None => return Err(format!("theme.ini: unknown key `{k}`")),
            },
        }
    }
    Ok(s)
}

/// An installed theme package as a Look.
pub struct PackageLook {
    pub id: &'static str,
    pub name: &'static str,
    pub spec: ThemeSpec,
}

fn px(v: Option<f32>, classic: f32) -> i32 {
    ui_px(v.unwrap_or(classic))
}

impl Look for PackageLook {
    fn id(&self) -> &'static str {
        self.id
    }
    fn style_label(&self) -> &'static str {
        self.name
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn faces(&self) -> Option<[FaceMap; FontFace::COUNT]> {
        if self.spec.gothic_type { super::gothic::Gothic.faces() } else { None }
    }
    fn share_pages_gothic(&self) -> bool {
        self.spec.gothic_type
    }
    fn name_face(&self) -> FontFace {
        self.spec.name_face.unwrap_or(FontFace::Bold)
    }
    fn heading_face(&self) -> FontFace {
        self.spec.heading_face.unwrap_or(FontFace::Bold)
    }
    fn label_lift(&self) -> i32 {
        px(self.spec.label_lift, 0.0)
    }
    fn caps_labels(&self) -> bool {
        self.spec.caps
    }
    fn corner(&self, k: Corner) -> i32 {
        let c = &self.spec.corners;
        match k {
            Corner::Card => px(c[0], 22.0),
            Corner::Surface => px(c[1], 24.0),
            Corner::PopupRow => px(c[2], 18.0),
            Corner::SheetRow => px(c[3], 20.0),
            Corner::Tooltip => px(c[4], 16.0),
            Corner::Pill(h) => self.spec.pill.map_or(h / 2, |v| ui_px(v).min(h / 2)),
            Corner::Tag(h) => self.spec.tag.map_or(h / 2, |v| ui_px(v).min(h / 2)),
            Corner::Battery(k) => (10 * k).max(1),
        }
    }
    fn page_title_scale(&self) -> f32 {
        self.spec.title_scale.unwrap_or(1.1)
    }
    fn slide_up_title_scale(&self) -> f32 {
        self.spec.sheet_title_scale.unwrap_or(0.72)
    }
    fn field_rule_gray(&self) -> u8 {
        self.spec.field_rule.unwrap_or(crate::globalui::GUI_GRAY_RULE)
    }
    fn slider_knob_radius(&self) -> i32 {
        px(self.spec.knob, 26.0)
    }
    // (a cut chip's ends are points as deep as half the chip: its text keeps off them, as Gothic's does)
    fn chip_pad(&self, h: i32) -> i32 {
        if self.spec.cut { h / 2 - ui_px(7.0) } else { ui_px(24.0) }
    }
    fn frame(&self, c: &mut Canvas, r: Rect, radius: i32, gray: u8, filled: bool, t: i32) {
        if self.spec.cut {
            canvas_chamfer_rect(c, r, radius + radius / 4, gray, filled, t);
        } else {
            canvas_round_rect(c, r, radius, gray, filled, t);
        }
    }
}

thread_local! {
    // (per thread, as the store's root and the theme in use are: each test thread has its own)
    static LOOKS_OF_PACKAGES: RefCell<Vec<&'static PackageLook>> = const { RefCell::new(Vec::new()) };
    static LOADED: Cell<bool> = const { Cell::new(false) };
}

/// Reads the installed theme packages again (after an install or a removal). A look whose id, name and theme.ini are
/// unchanged is kept, so a reload that changes nothing leaks nothing.
pub fn package_looks_reload() {
    let root_known = !crate::store::store_root().is_empty();
    let found = if root_known { crate::themepkg::theme_package_specs() } else { Vec::new() };
    LOOKS_OF_PACKAGES.with(|cell| {
        let old = cell.borrow().clone();
        let next = found
            .into_iter()
            .map(|(id, name, spec)| {
                // shortcut: a changed theme leaks its few bytes until the app restarts; a reload is an install's, rare
                old.iter().copied().find(|l| l.id == id && l.name == name && l.spec == spec).unwrap_or_else(|| {
                    Box::leak(Box::new(PackageLook { id: Box::leak(id.into_boxed_str()), name: Box::leak(name.into_boxed_str()), spec }))
                })
            })
            .collect();
        *cell.borrow_mut() = next;
    });
    LOADED.set(root_known);
}

fn loaded() {
    if !LOADED.get() {
        package_looks_reload();
    }
}

/// How many theme packages are installed (the Style list shows them after the built-in themes).
pub fn package_look_count() -> usize {
    loaded();
    LOOKS_OF_PACKAGES.with(|cell| cell.borrow().len())
}

/// The i-th installed theme package, in the Style list's order.
pub fn package_look(i: usize) -> Option<&'static PackageLook> {
    loaded();
    LOOKS_OF_PACKAGES.with(|cell| cell.borrow().get(i).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_ini_reads_its_keys_and_refuses_what_it_does_not_know() {
        let s = theme_spec_parse(
            b"# a test theme\nframe = cut\nfonts = Gothic\ncard = 8\npill = round\ntag = 6 # small\ntitle_scale = 1.4\n\
              field_rule = 0\ncaps = yes\nname_face = regular\nknob = 30\n",
        )
        .unwrap();
        assert!(s.cut && s.gothic_type && s.caps);
        assert_eq!(s.corners[0], Some(8.0));
        assert_eq!((s.pill, s.tag), (None, Some(6.0)));
        assert_eq!(s.title_scale, Some(1.4));
        assert_eq!(s.field_rule, Some(0));
        assert_eq!(s.name_face, Some(FontFace::Regular));
        assert_eq!(theme_spec_parse(b"").unwrap(), ThemeSpec::default());
        for bad in ["colour = red", "card = 99", "title_scale = x", "frame = wavy", "caps = maybe", "just words", "knob = NaN"] {
            assert!(theme_spec_parse(bad.as_bytes()).is_err(), "{bad}");
        }
        assert!(theme_spec_parse(&vec![b'#'; THEME_FILE_MAX + 1]).is_err());
        assert!(theme_spec_parse(b"\xff = 1").is_err());
    }

    #[test]
    fn an_empty_theme_ini_is_classic_in_every_measure() {
        let l = PackageLook { id: "t", name: "T", spec: ThemeSpec::default() };
        let c = &super::super::classic::Classic;
        for k in [Corner::Card, Corner::Surface, Corner::Pill(40), Corner::Tag(30), Corner::PopupRow, Corner::SheetRow, Corner::Tooltip, Corner::Battery(3)] {
            assert_eq!(l.corner(k), c.corner(k), "{k:?}");
        }
        assert_eq!(l.page_title_scale(), c.page_title_scale());
        assert_eq!(l.slide_up_title_scale(), c.slide_up_title_scale());
        assert_eq!(l.field_rule_gray(), c.field_rule_gray());
        assert_eq!(l.slider_knob_radius(), c.slider_knob_radius());
        assert_eq!(l.chip_pad(50), c.chip_pad(50));
        assert_eq!((l.name_face(), l.heading_face(), l.caps_labels(), l.label_lift()), (c.name_face(), c.heading_face(), c.caps_labels(), c.label_lift()));
        assert!(l.faces().is_none() && !l.share_pages_gothic());
    }
}
