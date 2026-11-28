//! widgets.c: the editor's widget windows and their layout.
//!
//! Every widget comes from an installed package (widgetpkg.rs). A widget is a small window on the
//! editor canvas: it can be raised, dragged by its title bar, minimized, resized from its
//! bottom-right grip, and closed. Its place and size are kept in widgets.ini at the store root, so
//! they come back on the next launch.
//!
//! The module is geometry, frame state and the runtime handle. editor.rs draws the frames and
//! routes input; widgetwasm.rs runs the package's WebAssembly.

use crate::prelude::*;

/// The most widgets that can be open at once.
pub const WIDGETS_MAX: usize = 8;
/// The square a minimized widget takes, at 300 dpi.
const WIDGET_MINIMIZED: f32 = 112.0;
/// How far from the canvas edge a new widget starts.
const WIDGET_MARGIN: f32 = 28.0;
/// How far each new widget is offset from the one before it.
const WIDGET_CASCADE: f32 = 22.0;
/// A frame's own parts, at 300 dpi: the title bar's height; the margin on each side of the body; the gap
/// between the title bar and the body; and what a frame's height holds beside its title bar and its body
/// (that gap and the margin under the body). `title_rect`, `close_rect` and `body_rect` lay a frame out with
/// these, and the smallest frame is made of them too.
const WIDGET_TITLE_H: f32 = 64.0;
const WIDGET_BODY_SIDE: f32 = 14.0;
const WIDGET_BODY_TOP: f32 = 8.0;
const WIDGET_BODY_ROWS: f32 = 22.0;
/// The smallest body's height: one line of the small text (`ui_small_size`).
const WIDGET_BODY_MIN_H: f32 = 34.0;
/// The smallest frame the editor will make, whatever a widget asks for, at 300 dpi: 120 by 120. Its height is
/// the title bar, the rows around the body and the smallest body, so a frame at its smallest still has a body.
pub const WIDGET_FLOOR_W: f32 = 120.0;
pub const WIDGET_FLOOR_H: f32 = WIDGET_TITLE_H + WIDGET_BODY_ROWS + WIDGET_BODY_MIN_H;

/// The smallest frame in screen pixels: what a manifest's sizes are checked against (widgetpkg.rs) and what
/// `WidgetFrame::limits` and `WidgetHost::spawn` hold a frame to. The height is the sum of its parts as the
/// layout rounds them, so at any scale the body of the smallest frame is `WIDGET_BODY_MIN_H` high exactly.
pub fn widget_floor() -> (i32, i32) {
    (ui_px(WIDGET_FLOOR_W), ui_px(WIDGET_TITLE_H) + ui_px(WIDGET_BODY_ROWS) + ui_px(WIDGET_BODY_MIN_H))
}
/// A frame never takes more than this share of the canvas either way.
const WIDGET_MAX_SHARE: i32 = 70;

/// One entry in the widget catalog. Sizes are at 300 dpi, as the rest of the UI's constants.
#[derive(Clone, Copy, Debug)]
pub struct WidgetDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: IconKind,
    pub default_w: f32,
    pub default_h: f32,
    pub min_w: f32,
    pub min_h: f32,
    pub max_w: f32,
    pub max_h: f32,
    pub resizable: bool,
    /// the installed package's version, author and description
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
}

/// The catalog starts empty: every widget is an installed package (widgetpkg.rs).
pub const WIDGET_CATALOG: [WidgetDef; 0] = [];

/// The icon a package names in its manifest. Unknown names get the generic grid.
pub fn widget_icon(name: &str) -> IconKind {
    let name = name.trim().to_ascii_lowercase();
    let name = name.strip_prefix("md-").unwrap_or(&name);
    match name {
        "calculator" => IconKind::Calculator,
        "package" => IconKind::Package,
        "clock" => IconKind::Clock,
        "timer" | "hourglass" => IconKind::Hourglass,
        "music" => IconKind::Music,
        "notebook" => IconKind::Notebook,
        "pdf" => IconKind::Pdf,
        "image" | "photo" => IconKind::Image,
        "files" | "folder" => IconKind::Files,
        "search" => IconKind::Search,
        "download" => IconKind::Download,
        "language" => IconKind::Language,
        "gear" | "settings" => IconKind::Gear,
        "update" => IconKind::Update,
        "paper" | "page" => IconKind::Paper,
        "library" => IconKind::Library,
        "link" => IconKind::Link,
        "live" => IconKind::Live,
        "gamepad" => IconKind::Gamepad,
        "eye" => IconKind::Eye,
        "style" | "theme" => IconKind::Castle,
        _ => IconKind::Grid,
    }
}

/// The installed packages in name order. The package entries are cached by widgetpkg.rs and
/// refreshed after an install, update or remove.
pub fn widget_catalog() -> Vec<&'static WidgetDef> {
    let mut v: Vec<&'static WidgetDef> = WIDGET_CATALOG.iter().collect();
    v.extend(crate::widgetpkg::widget_package_defs());
    v
}

pub fn widget_def(id: &str) -> Option<&'static WidgetDef> {
    WIDGET_CATALOG.iter().find(|d| d.id == id).or_else(|| crate::widgetpkg::widget_package_defs().into_iter().find(|d| d.id == id))
}

/// A widget window's place on the canvas. x, y, w and h are in screen pixels; w and h are the
/// restored size while minimized too, so a restore comes back the size it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetLayout {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub minimized: bool,
    pub z: i32,
}

/// An open widget window and the generic runtime its package supplies.
#[derive(Debug)]
pub struct WidgetFrame {
    pub def: &'static WidgetDef,
    pub layout: WidgetLayout,
    pub runtime: Option<WidgetRuntime>,
    pub runtime_error: Option<String>,
    /// the body's contact recognised (the kit's events) and the drawing pass input owes (kithost.rs)
    pub input: crate::kithost::KitInput,
}

/// Where a contact came down in a frame. The editor routes input by these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WidgetPart {
    #[default]
    None,
    Title,
    Close,
    Minimize,
    Resize,
    Body,
}

/// The state of one contact on a widget: which frame and part, where it came down, and the frame's
/// place then.
#[derive(Clone, Copy, Debug)]
pub struct WidgetDrag {
    pub index: usize,
    pub part: WidgetPart,
    pub down_x: i32,
    pub down_y: i32,
    pub start: Rect,
    pub moved: bool,
}

impl WidgetFrame {
    pub fn new(def: &'static WidgetDef) -> Self {
        WidgetFrame {
            def,
            layout: WidgetLayout {
                x: 0,
                y: 0,
                w: ui_px(def.default_w),
                h: ui_px(def.default_h),
                minimized: false,
                z: 0,
            },
            runtime: None,
            runtime_error: None,
            input: Default::default(),
        }
    }

    /// Loads the package's WASM instance on the first draw (or the first touch of its body), or keeps the error for
    /// the body. The module is given the frame's body from its start on.
    pub fn ensure_runtime(&mut self) {
        if self.runtime.is_some() || self.runtime_error.is_some() {
            return;
        }
        let body = self.body_rect();
        match WidgetRuntime::new(self.def.id, (body.w, body.h)) {
            Ok(r) => self.runtime = Some(r),
            Err(e) => self.runtime_error = Some(e),
        }
    }

    /// The last runtime error, if its package or module could not start or was stopped: the first
    /// MESSAGE_SHOWN_MAX bytes of it at most, cut between two characters, wherever it came from.
    pub fn error(&self) -> Option<&str> {
        self.runtime_error.as_deref().or_else(|| self.runtime.as_ref().and_then(WidgetRuntime::error)).map(message_shown)
    }

    /// What a frame whose module failed says in place of its body, in the order it is drawn: the reason (the
    /// frame's error), then whose widget it is (the package's name, its version and its author). None while the
    /// module runs. The reason is first so that a body with room for little shows it. Each is MESSAGE_SHOWN_MAX
    /// bytes at most.
    pub fn says(&self) -> Option<(String, String)> {
        let reason = self.error()?.to_string();
        let mut whose = self.def.name.to_string();
        if !self.def.version.is_empty() {
            whose.push_str(&format!(" v{}", self.def.version));
        }
        if !self.def.author.is_empty() {
            whose.push_str(&format!(" · {}", self.def.author));
        }
        Some((reason, message_shown(&whose).to_string()))
    }

    /// The frame as drawn and hit: a minimized widget is a small square at its corner.
    pub fn rect(&self) -> Rect {
        if self.layout.minimized {
            let s = ui_px(WIDGET_MINIMIZED).min(self.layout.w).min(self.layout.h).max(ui_px(64.0));
            Rect { x: self.layout.x, y: self.layout.y, w: s, h: s }
        } else {
            Rect { x: self.layout.x, y: self.layout.y, w: self.layout.w, h: self.layout.h }
        }
    }

    /// The sizes this frame may take on `area`: the widget's own bounds, never past the screen's
    /// floor and share.
    pub fn limits(&self, area: Rect) -> (i32, i32, i32, i32) {
        let (floor_w, floor_h) = widget_floor();
        let share_w = (area.w * WIDGET_MAX_SHARE / 100).max(floor_w);
        let share_h = (area.h * WIDGET_MAX_SHARE / 100).max(floor_h);
        let min_w = ui_px(self.def.min_w).clamp(floor_w, share_w);
        let min_h = ui_px(self.def.min_h).clamp(floor_h, share_h);
        let max_w = ui_px(self.def.max_w).clamp(min_w, share_w);
        let max_h = ui_px(self.def.max_h).clamp(min_h, share_h);
        (min_w, min_h, max_w, max_h)
    }

    /// Keeps the frame's size in its bounds and its whole rectangle on `area`.
    pub fn clamp(&mut self, area: Rect) -> bool {
        let before = self.layout;
        let (min_w, min_h, max_w, max_h) = self.limits(area);
        self.layout.w = self.layout.w.clamp(min_w, max_w);
        self.layout.h = self.layout.h.clamp(min_h, max_h);
        let r = self.rect();
        self.layout.x = clampi(self.layout.x, area.x, area.x + area.w - r.w);
        self.layout.y = clampi(self.layout.y, area.y, area.y + area.h - r.h);
        self.layout != before
    }

    pub fn move_to(&mut self, x: i32, y: i32, area: Rect) -> bool {
        let r = self.rect();
        let nx = clampi(x, area.x, area.x + area.w - r.w);
        let ny = clampi(y, area.y, area.y + area.h - r.h);
        let changed = nx != self.layout.x || ny != self.layout.y;
        self.layout.x = nx;
        self.layout.y = ny;
        changed
    }

    pub fn resize_to(&mut self, w: i32, h: i32, area: Rect) -> bool {
        let before = self.layout;
        let (min_w, min_h, max_w, max_h) = self.limits(area);
        self.layout.w = w.clamp(min_w, max_w);
        self.layout.h = h.clamp(min_h, max_h);
        let r = self.rect();
        self.layout.x = clampi(self.layout.x, area.x, area.x + area.w - r.w);
        self.layout.y = clampi(self.layout.y, area.y, area.y + area.h - r.h);
        self.layout != before
    }

    pub fn title_rect(&self) -> Rect {
        let r = self.rect();
        let h = (ui_px(WIDGET_TITLE_H)).min(r.h);
        Rect { x: r.x, y: r.y, w: r.w, h }
    }

    /// The body: what the package draws on, under the title bar and inside the frame's margins. A frame no
    /// smaller than `widget_floor` has one at least `WIDGET_BODY_MIN_H` high.
    pub fn body_rect(&self) -> Rect {
        let r = self.rect();
        let title = self.title_rect();
        Rect {
            x: r.x + ui_px(WIDGET_BODY_SIDE),
            y: title.y + title.h + ui_px(WIDGET_BODY_TOP),
            w: r.w - ui_px(2.0 * WIDGET_BODY_SIDE),
            h: r.h - title.h - ui_px(WIDGET_BODY_ROWS),
        }
    }

    pub fn close_rect(&self) -> Rect {
        let r = self.rect();
        let side = ui_px(WIDGET_TITLE_H).min(r.h).min(r.w / 3);
        Rect { x: r.x + r.w - side, y: r.y, w: side, h: side }
    }

    pub fn minimize_rect(&self) -> Rect {
        let r = self.close_rect();
        Rect { x: r.x - r.w, y: r.y, w: r.w, h: r.h }
    }

    pub fn resize_rect(&self) -> Rect {
        let r = self.rect();
        let side = ui_px(72.0).min(r.w / 3).min(r.h / 3);
        Rect { x: r.x + r.w - side, y: r.y + r.h - side, w: side, h: side }
    }
}

/// The editor's open widgets. Draw order is z order; the topmost wins a hit.
#[derive(Debug)]
pub struct WidgetHost {
    pub frames: Vec<WidgetFrame>,
    next_z: i32,
    /// Whether widgets.ini has been read (the editor reads it when it first opens).
    pub loaded: bool,
}

impl Default for WidgetHost {
    fn default() -> Self {
        WidgetHost::new()
    }
}

impl WidgetHost {
    pub fn new() -> Self {
        WidgetHost { frames: Vec::new(), next_z: 0, loaded: false }
    }

    pub fn is_open(&self, id: &str) -> bool {
        self.frames.iter().any(|f| f.def.id == id)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.frames.iter().position(|f| f.def.id == id)
    }

    /// Lets an open frame of this package read the settings that were just saved, and tells its module so (a
    /// SETTINGS event), which owes a drawing pass when it says its picture changed.
    pub fn reload_settings(&mut self, id: &str) {
        if let Some(i) = self.index_of(id) {
            let f = &mut self.frames[i];
            if let Some(rt) = f.runtime.as_mut() {
                rt.reload_settings(id);
                if rt.event(&crate::kithost::KitEvent::new(crate::kithost::EV_SETTINGS, 0, 0, 0, 0)) == Some(true) {
                    f.input.owe();
                }
            }
        }
    }

    /// The next z number, for the frame that goes on top. The numbers only order the frames, so when the next
    /// one would be the largest an i32 holds (widgets.ini is a file a hand can edit: a line may say any number)
    /// the open frames are counted again from 0 in the order they have, and no sum passes that number.
    fn top_z(&mut self) -> i32 {
        if self.next_z == i32::MAX {
            for (rank, i) in self.draw_order().into_iter().enumerate() {
                self.frames[i].layout.z = rank as i32;
            }
            self.next_z = self.frames.len() as i32;
        }
        let z = self.next_z.max(0);
        self.next_z = z + 1;
        z
    }

    /// Raises a frame to the top, returning whether its z changed.
    pub fn raise(&mut self, index: usize) -> bool {
        let Some(f) = self.frames.get(index) else { return false };
        if f.layout.z == self.next_z.max(0) {
            return false;
        }
        let z = self.top_z();
        self.frames[index].layout.z = z;
        true
    }

    /// Opens a catalog widget, or raises it when it is already open.
    pub fn spawn(&mut self, def: &'static WidgetDef, area: Rect, panel: Rect) -> bool {
        if let Some(i) = self.index_of(def.id) {
            self.raise(i);
            return false;
        }
        if self.frames.len() >= WIDGETS_MAX {
            return false;
        }
        let mut f = WidgetFrame::new(def);
        let (_, _, max_w, max_h) = f.limits(area);
        let (floor_w, floor_h) = widget_floor();
        f.layout.w = f.layout.w.clamp(floor_w, max_w);
        f.layout.h = f.layout.h.clamp(floor_h, max_h);
        // Top right of the canvas, left of the panel when the panel is there: the first thing a
        // new widget shows should not be hidden under the toolbar.
        let mut x = area.x + area.w - f.layout.w - ui_px(WIDGET_MARGIN);
        if panel.x > area.x + area.w / 2 {
            x = panel.x - f.layout.w - ui_px(WIDGET_MARGIN);
        }
        let step = ui_px(WIDGET_CASCADE) * self.frames.len() as i32;
        let y = area.y + ui_px(WIDGET_MARGIN) + step;
        f.layout.x = clampi(x - step, area.x, (area.x + area.w - f.layout.w).max(area.x));
        f.layout.y = clampi(y, area.y, (area.y + area.h - f.layout.h).max(area.y));
        f.layout.z = self.top_z();
        self.frames.push(f);
        true
    }

    /// Opens a frame from widgets.ini, with its saved place and size, clamped to this screen.
    pub fn restore(&mut self, def: &'static WidgetDef, layout: WidgetLayout, area: Rect) {
        if self.is_open(def.id) || self.frames.len() >= WIDGETS_MAX {
            return;
        }
        let mut f = WidgetFrame::new(def);
        f.layout = layout;
        f.clamp(area);
        // (the saved z may be any number: the sum stops at the largest, and `top_z` counts again from there)
        self.next_z = self.next_z.max(f.layout.z.saturating_add(1));
        self.frames.push(f);
    }

    pub fn close(&mut self, index: usize) -> bool {
        if index < self.frames.len() {
            self.frames.remove(index);
            true
        } else {
            false
        }
    }

    /// The topmost frame under a point, and which part of it.
    pub fn hit(&self, x: i32, y: i32) -> Option<(usize, WidgetPart)> {
        let mut order: Vec<usize> = (0..self.frames.len()).collect();
        order.sort_by_key(|&i| self.frames[i].layout.z);
        for &i in order.iter().rev() {
            let f = &self.frames[i];
            let r = f.rect();
            if !rect_contains(r, x, y) {
                continue;
            }
            if f.layout.minimized {
                return Some((i, WidgetPart::Title));
            }
            if rect_contains(f.close_rect(), x, y) {
                return Some((i, WidgetPart::Close));
            }
            if rect_contains(f.minimize_rect(), x, y) {
                return Some((i, WidgetPart::Minimize));
            }
            if f.def.resizable && rect_contains(f.resize_rect(), x, y) {
                return Some((i, WidgetPart::Resize));
            }
            if rect_contains(f.title_rect(), x, y) {
                return Some((i, WidgetPart::Title));
            }
            return Some((i, WidgetPart::Body));
        }
        None
    }

    pub fn draw_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.frames.len()).collect();
        order.sort_by_key(|&i| self.frames[i].layout.z);
        order
    }

    /// Keeps every frame on this screen after a turn or a resize.
    pub fn reclamp(&mut self, area: Rect) -> bool {
        let mut changed = false;
        for f in &mut self.frames {
            changed |= f.clamp(area);
        }
        changed
    }

    pub fn load(&mut self, area: Rect) {
        self.frames.clear();
        self.next_z = 0;
        for e in widgets_load() {
            if let Some(def) = widget_def(&e.id) {
                self.restore(def, e.layout, area);
            }
        }
        self.loaded = true;
    }

    pub fn save(&self) -> i32 {
        let entries: Vec<WidgetEntry> = self
            .frames
            .iter()
            .map(|f| WidgetEntry { id: f.def.id.to_string(), layout: f.layout })
            .collect();
        widgets_save(&entries)
    }
}

/// One line of widgets.ini.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetEntry {
    pub id: String,
    pub layout: WidgetLayout,
}

fn widgets_path() -> String {
    format!("{}/widgets.ini", store_root())
}

/// Reads widgets.ini. A missing or unreadable file is an empty layout. Lines that are not a package's
/// id (`widget_id_ok`, the manifest check's own rule) with seven comma-separated fields are skipped;
/// the file is small and human-editable.
pub fn widgets_load() -> Vec<WidgetEntry> {
    let root = store_root();
    if root.is_empty() {
        return Vec::new();
    }
    let Some(data) = read_file_max(&widgets_path(), 64 * 1024) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&data);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((id, rest)) = line.split_once('=') else { continue };
        let id = id.trim();
        if !crate::widgetpkg::widget_id_ok(id) {
            continue;
        }
        let v: Vec<&str> = rest.split(',').map(str::trim).collect();
        if v.len() != 7 {
            continue;
        }
        let Ok(x) = v[0].parse::<i32>() else { continue };
        let Ok(y) = v[1].parse::<i32>() else { continue };
        let Ok(w) = v[2].parse::<i32>() else { continue };
        let Ok(h) = v[3].parse::<i32>() else { continue };
        let Ok(minimized) = v[4].parse::<i32>() else { continue };
        let Ok(z) = v[5].parse::<i32>() else { continue };
        // Seven fields are required, but the last is reserved for a future flag; accept it only as
        // a number so a malformed line cannot make the rest look valid.
        if v[6].parse::<i32>().is_err() {
            continue;
        }
        if w <= 0 || h <= 0 {
            continue;
        }
        out.push(WidgetEntry {
            id: id.to_string(),
            layout: WidgetLayout { x, y, w, h, minimized: minimized != 0, z },
        });
    }
    out
}

/// Writes widgets.ini atomically. The open frames are the file: a closed widget has no line. A line is
/// written for an id the reader takes (every installed package's is one), so the file holds no line that
/// would be skipped when it is read.
pub fn widgets_save(entries: &[WidgetEntry]) -> i32 {
    let root = store_root();
    if root.is_empty() {
        return 0;
    }
    let _ = mkdir_p(&root);
    let mut s = String::from("# Grimoire widget layout. One line per open widget:\n");
    s.push_str("# id=x,y,w,h,minimized,z\n");
    for e in entries.iter().filter(|e| crate::widgetpkg::widget_id_ok(&e.id)) {
        s.push_str(&format!(
            "{}={},{},{},{},{},{},0\n",
            e.id, e.layout.x, e.layout.y, e.layout.w, e.layout.h, e.layout.minimized as i32, e.layout.z
        ));
    }
    write_file_atomic(&widgets_path(), s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect { x: 0, y: 0, w: 1860, h: 2480 }
    }

    fn panel() -> Rect {
        Rect { x: 1700, y: 220, w: 128, h: 1700 }
    }

    static TEST_WIDGET: WidgetDef = WidgetDef {
        id: "test.widget",
        name: "Test",
        icon: IconKind::Grid,
        default_w: 300.0,
        default_h: 420.0,
        min_w: 240.0,
        min_h: 360.0,
        max_w: 460.0,
        max_h: 620.0,
        resizable: true,
        version: "",
        author: "",
        description: "",
    };

    #[test]
    fn a_package_icon_names_a_grimoire_icon() {
        assert_eq!(widget_icon("calculator"), IconKind::Calculator);
        assert_eq!(widget_icon("package"), IconKind::Package);
        assert_eq!(widget_icon("md-timer"), IconKind::Hourglass);
        assert_eq!(widget_icon("not one"), IconKind::Grid);
    }

    #[test]
    fn spawn_stays_on_the_canvas_and_to_the_left_of_the_panel() {
        ui_init(300.0);
        let mut h = WidgetHost::new();
        let notes = &TEST_WIDGET;
        assert!(h.spawn(notes, area(), panel()));
        let r = h.frames[0].rect();
        assert!(r.x >= 0 && r.y >= 0 && r.x + r.w <= area().w && r.y + r.h <= area().h);
        assert!(r.x + r.w <= panel().x);
        // The same widget again raises, does not duplicate.
        let z = h.frames[0].layout.z;
        assert!(!h.spawn(notes, area(), panel()));
        assert_eq!(h.frames.len(), 1);
        assert!(h.frames[0].layout.z >= z);
    }

    #[test]
    fn clamp_and_resize_keep_the_floor_and_the_share() {
        ui_init(300.0);
        let mut f = WidgetFrame::new(&TEST_WIDGET);
        f.resize_to(1, 1, area());
        let (min_w, min_h, _, _) = f.limits(area());
        assert_eq!((f.layout.w, f.layout.h), (min_w, min_h));
        f.resize_to(100_000, 100_000, area());
        let (_, _, max_w, max_h) = f.limits(area());
        assert_eq!((f.layout.w, f.layout.h), (max_w, max_h));
        f.move_to(-500, 9000, area());
        assert!(f.layout.x >= area().x && f.layout.y + f.rect().h <= area().h);
    }

    /// A package that asks for less than any frame may be: its sizes are held to the floor.
    static TINY_WIDGET: WidgetDef = WidgetDef {
        id: "test.tiny",
        name: "Tiny",
        icon: IconKind::Grid,
        default_w: 1.0,
        default_h: 1.0,
        min_w: 1.0,
        min_h: 1.0,
        max_w: 500.0,
        max_h: 500.0,
        resizable: true,
        version: "",
        author: "",
        description: "",
    };

    #[test]
    fn the_smallest_frame_still_has_a_body() {
        assert_eq!((WIDGET_FLOOR_W, WIDGET_FLOOR_H), (120.0, 120.0));
        // (the Scribe's 300 dpi, and other scales, where each part rounds on its own)
        for dpi in [300.0, 226.0, 167.0, 212.0, 265.0, 359.0, 600.0] {
            ui_init(dpi);
            let (floor_w, floor_h) = widget_floor();
            let body_min = ui_px(WIDGET_BODY_MIN_H);
            assert!(body_min >= 1);
            // Opened from the popup, resized to nothing, restored from a layout with no size, and clamped again:
            // each is the floor, and the body there is as high as the smallest body and inside the frame.
            let mut h = WidgetHost::new();
            assert!(h.spawn(&TINY_WIDGET, area(), panel()));
            let mut resized = WidgetFrame::new(&TINY_WIDGET);
            resized.resize_to(1, 1, area());
            let mut restored = WidgetHost::new();
            restored.restore(&TINY_WIDGET, WidgetLayout { x: 40, y: 40, w: 1, h: 1, minimized: false, z: 0 }, area());
            let mut clamped = WidgetFrame::new(&TINY_WIDGET);
            clamped.layout.h = 0;
            clamped.clamp(area());
            for f in [&h.frames[0], &resized, &restored.frames[0], &clamped] {
                assert_eq!((f.layout.w, f.layout.h), (floor_w, floor_h), "at {dpi} dpi");
                let (r, title, body) = (f.rect(), f.title_rect(), f.body_rect());
                assert_eq!(body.h, body_min, "at {dpi} dpi");
                assert!(body.w > 0 && body.x > r.x && body.x + body.w < r.x + r.w, "at {dpi} dpi");
                assert!(body.y > title.y + title.h && body.y + body.h < r.y + r.h, "at {dpi} dpi");
            }
            // The floor is a frame's least height at every size the limits give, whatever the canvas.
            let small = Rect { x: 0, y: 0, w: 100, h: 100 };
            let (min_w, min_h, max_w, max_h) = resized.limits(small);
            assert!(min_w >= floor_w && min_h >= floor_h && max_w >= min_w && max_h >= min_h);
        }
        ui_init(300.0);
    }

    #[test]
    fn a_z_order_of_any_number_in_widgets_ini_keeps_its_order() {
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-widgets-z-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        // A line whose z is the largest number an i32 holds, between two ordinary ones, and one with the least.
        std::fs::write(
            root.join("widgets.ini"),
            "test.a=10,10,300,300,0,5,0\ntest.ini=10,10,300,300,0,2147483647,0\ntest.b=40,40,300,300,0,7,0\ntest.c=70,70,300,300,0,-2147483648,0\n",
        )
        .unwrap();
        let lines = widgets_load();
        assert_eq!(lines.iter().map(|e| e.layout.z).collect::<Vec<_>>(), [5, i32::MAX, 7, i32::MIN]);
        static DEFS: [WidgetDef; 5] = [
            WidgetDef { id: "test.a", ..TEST_WIDGET },
            WidgetDef { id: "test.ini", ..TEST_WIDGET },
            WidgetDef { id: "test.b", ..TEST_WIDGET },
            WidgetDef { id: "test.c", ..TEST_WIDGET },
            WidgetDef { id: "test.new", ..TEST_WIDGET },
        ];
        let mut h = WidgetHost::new();
        for (e, def) in lines.iter().zip(&DEFS) {
            h.restore(def, e.layout, area());
        }
        let ids = |h: &WidgetHost| h.draw_order().into_iter().map(|i| h.frames[i].def.id).collect::<Vec<_>>();
        assert_eq!(ids(&h), ["test.c", "test.a", "test.b", "test.ini"], "drawn in the order of their numbers");
        // What the editor does next with the frames: raise one, open another, raise each in turn. Each goes on top
        // of the others, which keep their order, and every number stays one a sum can be made with.
        assert!(h.raise(0));
        assert_eq!(ids(&h), ["test.c", "test.b", "test.ini", "test.a"]);
        assert!(h.spawn(&DEFS[4], area(), panel()));
        assert_eq!(ids(&h), ["test.c", "test.b", "test.ini", "test.a", "test.new"]);
        for i in [1, 3, 2, 0, 4, 1] {
            h.raise(i);
            assert_eq!(*ids(&h).last().unwrap(), h.frames[i].def.id);
            assert!(h.frames.iter().all(|f| (0..1000).contains(&f.layout.z)), "{:?}", h.frames.iter().map(|f| f.layout.z).collect::<Vec<_>>());
        }
        assert_eq!(h.hit(100, 100).map(|(i, _)| h.frames[i].def.id), Some("test.ini"), "the frame raised last is the one a tap finds");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_frame_says_the_reason_first() {
        static NAMED: WidgetDef = WidgetDef { name: "A Widget With A Long Name", version: "1.2.3", author: "Its Author", ..TEST_WIDGET };
        let mut f = WidgetFrame::new(&NAMED);
        assert_eq!(f.says(), None, "a frame with no error says nothing of one");
        f.runtime_error = Some("The widget's artifact could not be read.".to_string());
        let (reason, whose) = f.says().unwrap();
        assert_eq!(reason, "The widget's artifact could not be read.");
        assert_eq!(whose, "A Widget With A Long Name v1.2.3 · Its Author");
        // (a package with no version or author in its catalog entry: its name alone)
        let mut f = WidgetFrame::new(&TEST_WIDGET);
        f.runtime_error = Some("The widget package is not installed.".to_string());
        assert_eq!(f.says(), Some(("The widget package is not installed.".to_string(), "Test".to_string())));
    }

    #[test]
    fn a_frame_shows_at_most_256_bytes_of_each_text() {
        assert_eq!(MESSAGE_SHOWN_MAX, 256);
        let mut f = WidgetFrame::new(&TEST_WIDGET);
        // A reason of 256 bytes is shown whole; one byte more, and the first 256 are shown.
        for (n, shown) in [(255, 255), (256, 256), (257, 256), (100_000, 256)] {
            f.runtime_error = Some("a".repeat(n));
            assert_eq!(f.error(), Some("a".repeat(shown).as_str()), "a reason of {n} bytes");
            assert_eq!(f.says().unwrap().0, "a".repeat(shown), "a reason of {n} bytes");
        }
        // Of characters of three bytes, 256 bytes end inside one: the text is cut before it, at 255.
        f.runtime_error = Some("\u{20ac}".repeat(1000));
        assert_eq!(f.error(), Some("\u{20ac}".repeat(85).as_str()));
        // Whose widget it is, the same way: a version of 200,000 bytes, and a name and an author of 1,023 bytes each
        // (the most a manifest's name and author have).
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let reason = "The widget's artifact could not be read.".to_string();
        let versioned: &'static WidgetDef = Box::leak(Box::new(WidgetDef { name: "Test", version: leak(format!("1.0.0-{}", "a".repeat(200_000))), author: "Its Author", ..TEST_WIDGET }));
        let mut f = WidgetFrame::new(versioned);
        f.runtime_error = Some(reason.clone());
        assert_eq!(f.says(), Some((reason.clone(), format!("Test v1.0.0-{}", "a".repeat(256 - 12)))));
        let name = leak("\u{20ac}".repeat(341));
        let named: &'static WidgetDef = Box::leak(Box::new(WidgetDef { name, version: "1.0.0", author: name, ..TEST_WIDGET }));
        let mut f = WidgetFrame::new(named);
        f.runtime_error = Some(reason.clone());
        assert_eq!(f.says(), Some((reason, "\u{20ac}".repeat(85))));
    }

    #[test]
    fn minimized_rect_is_a_square_and_restore_keeps_the_size() {
        ui_init(300.0);
        let mut f = WidgetFrame::new(&TEST_WIDGET);
        let (w, h) = (f.layout.w, f.layout.h);
        f.layout.minimized = true;
        let r = f.rect();
        assert_eq!(r.w, r.h);
        f.layout.minimized = false;
        assert_eq!((f.layout.w, f.layout.h), (w, h));
    }

    #[test]
    fn layout_round_trips_through_widgets_ini() {
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-widgets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        let entries = vec![WidgetEntry {
            id: "test.widget".to_string(),
            layout: WidgetLayout { x: 10, y: 20, w: 260, h: 220, minimized: true, z: 3 },
        }];
        assert_eq!(widgets_save(&entries), 0);
        assert_eq!(widgets_load(), entries);
        // A bad line is skipped, not a reason to lose the good ones.
        std::fs::write(
            root.join("widgets.ini"),
            "# c\ntest.widget=1,2,3,4,0,5,0\nbad=1,2\nother=1,2,3,4,0,6,0\n",
        )
        .unwrap();
        let got = widgets_load();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "test.widget");
        assert_eq!(got[1].layout.z, 6);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A manifest with this id and nothing a frame needs beyond it.
    fn manifest_of(id: &str) -> String {
        format!(
            "name: Long\nid: {id}\ntype: widget\nversion: 1.0.0\ncapabilities: [editor:overlay]\nartifact:\n  path: w.bin\n  sha256: {}\n  size: 4\nspec:\n  runtime: wasm\n",
            crate::sha256::sha256_of(b"test")
        )
    }

    #[test]
    fn a_frame_of_a_package_with_the_longest_id_is_there_after_a_restart() {
        use crate::widgetpkg::{widget_catalog_invalidate, widget_id_ok, widget_package_install, WIDGET_ID_MAX};
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-widgets-long-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        widget_catalog_invalidate();
        // The longest id the manifest check takes: 128 bytes of every kind of character an id has.
        assert_eq!(WIDGET_ID_MAX, 128);
        let id = format!("com.example.{}", "long-id-0123456789.".repeat(7));
        let id = &id[..WIDGET_ID_MAX - 1];
        let id = format!("{id}z");
        assert!(id.len() == WIDGET_ID_MAX && widget_id_ok(&id));
        widget_package_install(&manifest_of(&id), b"test", "", 1860, 2480).unwrap();
        // (one byte more is no id: not to the manifest check, and not in widgets.ini)
        let longer = format!("{id}z");
        assert!(!widget_id_ok(&longer));
        assert!(widget_package_install(&manifest_of(&longer), b"test", "", 1860, 2480).is_err());

        // Open its frame and move it; the layout is saved as the editor saves it.
        let mut h = WidgetHost::new();
        assert!(h.spawn(widget_def(&id).unwrap(), area(), panel()));
        h.frames[0].move_to(300, 400, area());
        let layout = h.frames[0].layout;
        assert_eq!(h.save(), 0);
        assert_eq!(widgets_load(), vec![WidgetEntry { id: id.clone(), layout }]);

        // A restart: nothing is kept but the files. The catalog is read again, then the layout.
        drop(h);
        widget_catalog_invalidate();
        let mut again = WidgetHost::new();
        again.load(area());
        assert_eq!(again.frames.len(), 1, "the frame is open again");
        assert_eq!((again.frames[0].def.id, again.frames[0].layout), (id.as_str(), layout));

        // widgets.ini's reader takes the ids the manifest check takes, and no others.
        std::fs::write(
            root.join("widgets.ini"),
            format!("{longer}=1,2,300,300,0,1,0\nCom.Example=1,2,300,300,0,2,0\ncom_example=1,2,300,300,0,3,0\n.com.example=1,2,300,300,0,4,0\ncom..example=1,2,300,300,0,5,0\ncom.example.=1,2,300,300,0,6,0\n{id}=5,6,300,300,1,7,0\n"),
        )
        .unwrap();
        let got = widgets_load();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].id.as_str(), got[0].layout.z), (id.as_str(), 7));
        // (and the writer leaves out a line its reader would skip)
        let entry = |id: &str| WidgetEntry { id: id.to_string(), layout };
        assert_eq!(widgets_save(&[entry("Not An Id"), entry(&id), entry(&longer)]), 0);
        assert_eq!(widgets_load(), vec![entry(&id)]);
        // (the file itself, not what the reader makes of it: two lines of comment and that frame's line)
        let written = std::fs::read_to_string(root.join("widgets.ini")).unwrap();
        let lines: Vec<&str> = written.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(lines, [format!("{id}=300,400,{},{},0,{},0", layout.w, layout.h, layout.z)]);
        assert!(!written.contains("Not An Id") && !written.contains(&longer));
        widget_catalog_invalidate();
        let _ = std::fs::remove_dir_all(&root);
    }
}
