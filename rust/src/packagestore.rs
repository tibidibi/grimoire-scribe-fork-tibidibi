//! Package Store: the built-in catalog for Grimoire app and widget packages (2.3).
//!
//! The store is a GitHub-hosted `index.json` plus package folders. The catalog is cached
//! under `package-store/`, so the last downloaded store still opens offline. Screenshots
//! and store icons are cached the same way. Installing reuses the package manager's
//! manifest validation and installers; the manifest and artifact are checked by those
//! before anything is written.
//!
//! The catalog's icons and screenshots come from the network, so nothing here decodes them (issue
//! #236): the safe loader's picture worker does (pkgpics.rs), and the screens draw the gray
//! pictures it left in `.cache/packages/`, read back and checked. A picture is asked for once its
//! file is in `package-store/assets/` and comes a moment later (`asset_picture`, `tick_pictures`);
//! until then, and when the worker refuses it or none can be had, a card or a page shows what it
//! shows for a picture that is missing.

use crate::globalui::{gui_button, gui_button_h, gui_button_row, gui_chip, gui_chip_h, gui_chip_w, gui_gap, gui_header, gui_icon_button, gui_margin, gui_radius};
use crate::pkgpics::{picture_cached, picture_request, Ask, Cached, PicForm, PicSrc};
use crate::prelude::*;
use crate::sha256::sha256_of;
use crate::themepkg::{theme_package_install, theme_package_installed, theme_package_installed_manifest, theme_package_list, theme_package_remove, theme_package_validate};

/// The catalog this build publishes. A test or a mirror can point `GRIMOIRE_PACKAGE_STORE_URL`
/// at another `index.json`.
pub const DEFAULT_CATALOG_URL: &str = "https://raw.githubusercontent.com/grimoire-notes/package-store-contents/main/index.json";

const H_CLOSE: i32 = 801;
const H_BACK: i32 = 802;
const H_REFRESH: i32 = 803;
const H_FILTER: i32 = 804;
const H_CARD: i32 = 805;
const H_PREV: i32 = 806;
const H_NEXT: i32 = 807;
const H_INSTALL: i32 = 808;
const H_TRUST: i32 = 809;
const H_QR: i32 = 810;
const H_SHOT_PREV: i32 = 811;
const H_SHOT_NEXT: i32 = 812;
const H_RETRY: i32 = 813;
const H_SEARCH: i32 = 814;
const H_REPO: i32 = 815;
const H_UNINSTALL: i32 = 816;
const H_UNINSTALL_YES: i32 = 817;
const H_QUERY_CLEAR: i32 = 818;
const H_DESC_MORE: i32 = 819;
const H_DESC_PREV: i32 = 820;
const H_DESC_NEXT: i32 = 821;
const H_LICENSE: i32 = 822;
const H_LICENSE_PREV: i32 = 823;
const H_LICENSE_NEXT: i32 = 824;
const H_AGREE: i32 = 825;
/// A card of the Installed tab in a hit: `H_CARD`'s argument from this number up is an index into
/// `PackageStoreUi::installed` rather than the catalog.
const INSTALLED_CARD: i32 = 10_000;

const INDEX_MAX: usize = 512 * 1024;
const INDEX_TOKENS: usize = 32 * 1024;
const PACKAGE_MAX: usize = 200;
const SCREENSHOT_MAX: usize = crate::pkgpics::SHOT_FILE_MAX;
const INSTALL_ICON_MAX: usize = crate::apppkg::APP_ICON_MAX;
const LICENSE_MAX: usize = 256 * 1024;

/// Where a package being worked on is tried before it is published: a repository cloned into
/// `packages/import/` (its `package.yaml` and the artifact beside it). Each package there is installed
/// when the Store opens or is refreshed, so it is in the Launcher, the Widgets popup and the Installed
/// tab without a catalog entry.
const DEV_IMPORT_DIR: &str = "packages/import";
const DEV_IMPORT_MAX: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StoreView {
    #[default]
    List,
    Detail,
    Trust,
    Busy,
    Error,
    Qr,
    /// a GitHub repository or .yaml address is typed on the keyboard
    InstallRepo,
    /// asking before a package is uninstalled
    Remove,
    /// reading a package's license, installed or offered
    License,
    /// accepting a package's license before it is installed
    Agree,
}

/// Which field the Store's keyboard edits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum KbFor {
    #[default]
    Search,
    Repo,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StoreAsset {
    url: String,
    /// Safe path under `package-store/assets/`, or a digest for an absolute URL.
    key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StoreIconRef {
    Glyph(IconKind),
    Image(StoreAsset),
}

impl Default for StoreIconRef {
    fn default() -> Self {
        Self::Glyph(IconKind::Package)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorePackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub author: String,
    pub category: String,
    pub summary: String,
    pub description: String,
    pub license: String,
    pub repository: String,
    pub homepage: String,
    pub manifest_url: String,
    icon: StoreIconRef,
    screenshots: Vec<StoreAsset>,
    pub tags: Vec<String>,
    pub added: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoreIndex {
    pub name: String,
    pub tagline: String,
    pub updated: String,
    pub featured: String,
    pub packages: Vec<StorePackage>,
}

/// One installed package the catalog has a newer version of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackageUpdate {
    pub package: StorePackage,
    pub installed_version: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FetchStage {
    #[default]
    Catalog,
    /// GitHub's repository answer, for its default branch (a repository install)
    Branch,
    Manifest,
    Artifact,
    Icon,
    Screenshot,
    InstallIcon,
    ListIcon,
    /// a repository's license file, for the License page
    License,
}

struct FetchJob {
    stage: FetchStage,
    fetch: WidgetFetch,
    url: String,
    key: String,
}

/// A picture asked of the worker's job (pkgpics.rs) and not there yet: the slot it is for (`Icon`,
/// `Screenshot` or `ListIcon`, with the asset's key), its file as it was asked for, and its job's
/// path once the job is queued.
struct PicWait {
    stage: FetchStage,
    key: String,
    src: PicSrc,
    job: Option<String>,
}

/// What the Store has of a cached asset's picture now.
enum PicNow {
    /// the worker's picture, from its cache
    There(Canvas),
    /// asked for: it comes with a later tick (`tick_pictures`)
    Coming,
    /// none to be had: the worker refused the file
    None,
    /// the asset is not in `package-store/assets/` (it is still to download)
    NoFile,
}

#[derive(Default)]
struct StoreIcon {
    glyph: IconKind,
    image: Canvas,
}

impl StoreIcon {
    fn reset(&mut self) {
        canvas_free(&mut self.image);
        self.glyph = IconKind::Package;
    }

    fn draw(&self, c: &mut Canvas, r: Rect, ink: u8) {
        if !self.image.px.is_empty() {
            canvas_blit_fit(c, &self.image, r);
        } else {
            ui_icon(c, self.glyph, r, ink);
        }
    }
}

/// The screen's state, kept on `App` so the Library tick can poll its downloads.
#[derive(Default)]
pub struct PackageStoreUi {
    pub open: bool,
    pub view: StoreView,
    pub index: StoreIndex,
    pub filter: i32,
    page: i32,
    pub selected: Option<String>,
    pub hits: HitList,
    press: Press,
    pressed: Option<Rect>,
    pressed_hit: Option<Hit>,
    pub status: String,
    pub error: String,
    /// the text a Search typed, which filters the catalog or the Installed tab
    pub query: String,
    /// every installed package, gathered when the Store opens or a package is installed or removed
    pub installed: Vec<StorePackage>,
    /// what importing the developers' packages left in `packages/import/` said, for the list's note
    pub dev_note: String,
    /// the one keyboard: a Search's text, or a repository to install from
    pub kb: Keyboard,
    kb_for: KbFor,
    /// the manifest being installed from a repository rather than the catalog
    repo_install: bool,
    repo_input: String,
    repo_pkg: Option<StorePackage>,
    index_url: String,
    catalog_job: Option<FetchJob>,
    install_job: Option<FetchJob>,
    asset_job: Option<FetchJob>,
    manifest: Option<WidgetPackage>,
    manifest_url: String,
    manifest_yaml: String,
    artifact_url: String,
    icon_url: Option<String>,
    install_icon_key: String,
    pending_artifact: Vec<u8>,
    icon: StoreIcon,
    icon_for: String,
    icon_tried: bool,
    list_icons: Vec<Canvas>,
    list_tried: Vec<bool>,
    shot: Canvas,
    shot_for: String,
    shot_tried: bool,
    shot_index: i32,
    /// the package page's description is shown whole (its Read more was tapped)
    desc_more: bool,
    /// the description reader's page (0-based)
    desc_page: i32,
    /// the license text the repository answered with, and what is said while there is none
    license_text: String,
    license_note: String,
    license_urls: Vec<String>,
    license_try: usize,
    /// the license page (0-based), for a license longer than one page
    license_page: i32,
    /// the pictures asked of the worker and not there yet
    pic_waits: Vec<PicWait>,
    pub refreshing: bool,
    /// The General > Updates page uses this catalog, its icons and its install machinery without
    /// opening the Store screen.
    pub updates_center: bool,
    pub update_installing: bool,
    pub update_status: String,
    pub update_error: String,
    pub update_target: String,
}

impl PackageStoreUi {
    fn selected_package(&self) -> Option<&StorePackage> {
        let id = self.selected.as_deref()?;
        if self.repo_install {
            if let Some(p) = self.repo_pkg.as_ref().filter(|p| p.id == id) {
                return Some(p);
            }
        }
        self.index
            .packages
            .iter()
            .find(|p| p.id == id)
            .or_else(|| self.installed.iter().find(|p| p.id == id))
            .or_else(|| self.repo_pkg.as_ref().filter(|p| p.id == id))
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.desc_more = false;
        self.desc_page = 0;
        self.license_text.clear();
        self.license_note.clear();
        self.license_urls.clear();
        self.license_try = 0;
        self.license_page = 0;
        self.manifest = None;
        self.manifest_url.clear();
        self.manifest_yaml.clear();
        self.artifact_url.clear();
        self.icon_url = None;
        self.install_icon_key.clear();
        self.pending_artifact.clear();
        self.icon.reset();
        self.icon_for.clear();
        self.icon_tried = false;
        canvas_free(&mut self.shot);
        self.shot_for.clear();
        self.shot_tried = false;
        self.shot_index = 0;
        self.drop_waits(&|stage| matches!(stage, FetchStage::Icon | FetchStage::Screenshot));
    }

    fn cancel_jobs(&mut self) {
        self.catalog_job = None;
        self.install_job = None;
        self.asset_job = None;
        self.refreshing = false;
        self.repo_install = false;
        self.repo_input.clear();
        self.repo_pkg = None;
        self.drop_waits(&|_| true);
    }

    fn reset_list_icons(&mut self) {
        for icon in &mut self.list_icons {
            canvas_free(icon);
        }
        self.list_icons = vec![Canvas::default(); self.index.packages.len()];
        self.list_tried = vec![false; self.index.packages.len()];
        self.drop_waits(&|stage| stage == FetchStage::ListIcon);
    }

    /// The pictures of these slots are no longer waited for (another package, another screenshot,
    /// the Store closed): a job of theirs that is still queued leaves the queue, unless another
    /// slot waits on that very job (a running one runs on: its picture is kept for the next look).
    /// (One function, never a copy in each of its callers: the Kindle's binary has no room for them.)
    #[inline(never)]
    fn drop_waits(&mut self, which: &dyn Fn(FetchStage) -> bool) {
        let (gone, kept): (Vec<PicWait>, Vec<PicWait>) = std::mem::take(&mut self.pic_waits).into_iter().partition(|w| which(w.stage));
        self.pic_waits = kept;
        for job in gone.iter().filter_map(|w| w.job.as_deref()) {
            if !self.pic_waits.iter().any(|w| w.job.as_deref() == Some(job)) {
                bg_unqueue(BgKind::PackagePictures, "", job);
            }
        }
    }
}

/// The list's picture jobs that are still queued leave the queue (a running one runs on), so that a
/// package's page is not behind them with its own pictures, as `open_detail` gives up a card icon's
/// download for the page. Their waits stay, with no job, and are asked for again as places free up
/// (`tick_pictures`).
fn unqueue_list_pictures(ui: &mut PackageStoreUi) {
    for i in 0..ui.pic_waits.len() {
        let w = &ui.pic_waits[i];
        if w.stage != FetchStage::ListIcon {
            continue;
        }
        let Some(job) = w.job.clone() else { continue };
        if bg_unqueue(BgKind::PackagePictures, "", &job) {
            // (every wait on that very job: a page's icon may be the file of a card's)
            for w in ui.pic_waits.iter_mut().filter(|w| w.job.as_deref() == Some(job.as_str())) {
                w.job = None;
            }
        }
    }
}

/// Whether the Store can be drawn again by itself now (a picture that came from the worker): not
/// under a press, whose lift draws.
pub(crate) fn store_at_rest(ui: &PackageStoreUi) -> bool {
    ui.pressed.is_none() && !ui.kb.open
}

fn catalog_url() -> String {
    std::env::var("GRIMOIRE_PACKAGE_STORE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| s.starts_with("https://") && s.len() <= WIDGET_PKG_URL_MAX)
        .unwrap_or_else(|| DEFAULT_CATALOG_URL.to_string())
}

fn offline() -> bool {
    std::env::var_os("GRIMOIRE_PACKAGE_STORE_OFFLINE").is_some_and(|v| !v.is_empty())
}

/// The address a fetch asks when its answer must not be one a cache kept: the address with a query
/// that is new every time. GitHub's raw host keeps a file it served for five minutes
/// (`cache-control: max-age=300`) and its copies are keyed by the whole address, query included, so
/// a catalog or a manifest pushed a moment ago is read only from an address it has not seen. An
/// address with no room left for the query is fetched as it is (`fetch_address_ok` takes at most
/// WIDGET_PKG_URL_MAX bytes).
fn fresh_url(url: &str) -> String {
    let mark = if url.contains('?') { '&' } else { '?' };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let fresh = format!("{url}{mark}v={now}");
    if fresh.len() <= WIDGET_PKG_URL_MAX && !fresh.chars().any(|c| c.is_control() || c == ' ') {
        fresh
    } else {
        url.to_string()
    }
}

fn store_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !id.starts_with('.')
        && !id.ends_with('.')
        && !id.contains("..")
}

fn image_name(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    s.ends_with(".ico") || s.ends_with(".png") || s.ends_with(".jpg") || s.ends_with(".jpeg")
}

fn need_asset(s: &str) -> bool {
    s.contains('/') || image_name(s)
}

fn safe_key(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty()
        || s.len() > 512
        || s.starts_with('/')
        || s.contains('\\')
        || s.contains('?')
        || s.contains('#')
        || s.split('/').any(|p| p.is_empty() || p == "." || p == "..")
    {
        return None;
    }
    Some(s.to_string())
}

fn cache_key(value: &str) -> Option<String> {
    if let Some(key) = safe_key(value) {
        return Some(key);
    }
    if value.starts_with("https://") && value.len() <= WIDGET_PKG_URL_MAX {
        return Some(format!("url-{}", sha256_of(value.as_bytes())));
    }
    None
}

fn resolve_url(base: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > WIDGET_PKG_URL_MAX {
        return None;
    }
    if value.starts_with("https://") {
        return Some(value.to_string());
    }
    if value.starts_with("http://") || value.starts_with('/') || value.starts_with("//") {
        return None;
    }
    let (dir, _) = base.rsplit_once('/')?;
    Some(format!("{dir}/{value}"))
}

fn asset(base: &str, value: &str) -> Option<StoreAsset> {
    let url = resolve_url(base, value)?;
    let key = cache_key(value)?;
    Some(StoreAsset { url, key })
}

fn cache_root() -> String {
    format!("{}/package-store", store_root())
}

fn catalog_cache_path() -> String {
    format!("{}/index.json", cache_root())
}

fn asset_cache_path(key: &str) -> String {
    format!("{}/assets/{key}", cache_root())
}

fn read_cached_asset(key: &str, cap: usize) -> Option<Vec<u8>> {
    if safe_key(key).as_deref() != Some(key) {
        return None;
    }
    read_file_max(&asset_cache_path(key), cap)
}

fn write_cached_asset(key: &str, data: &[u8]) {
    let path = asset_cache_path(key);
    if let Some(parent) = path.rsplit_once('/').map(|(p, _)| p) {
        let _ = mkdir_p(parent);
    }
    let _ = write_file_atomic(&path, data);
}

fn write_cached_index(data: &[u8]) {
    let path = catalog_cache_path();
    if let Some(parent) = path.rsplit_once('/').map(|(p, _)| p) {
        let _ = mkdir_p(parent);
    }
    let _ = write_file_atomic(&path, data);
}

// ---- catalog parsing ----

fn jstr(text: &[u8], toks: &[JsonTok], obj: i32, key: &str, cap: usize) -> String {
    let v = json_get(text, toks, obj, key);
    if v < 0 {
        return String::new();
    }
    let mut b = Vec::new();
    if json_string(text, &toks[v as usize], &mut b, cap) {
        String::from_utf8_lossy(&b).trim().to_string()
    } else {
        String::new()
    }
}

fn jlist(text: &[u8], toks: &[JsonTok], obj: i32, key: &str, max: usize) -> Vec<String> {
    let arr = json_get(text, toks, obj, key);
    if arr < 0 || toks[arr as usize].type_ != JsonType::Array {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..toks[arr as usize].size.min(max as i32) {
        let it = json_item(toks, arr, i);
        if it < 0 {
            continue;
        }
        let mut b = Vec::new();
        if json_string(text, &toks[it as usize], &mut b, 1024) {
            let s = String::from_utf8_lossy(&b).trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    out
}

fn icon_ref(base: &str, value: &str) -> StoreIconRef {
    let value = value.trim();
    if need_asset(value) {
        if let Some(a) = asset(base, value) {
            return StoreIconRef::Image(a);
        }
    }
    StoreIconRef::Glyph(if value.is_empty() { IconKind::Package } else { widget_icon(value) })
}

/// Parses a catalog from bytes, with every relative path resolved against `base` (the
/// catalog's own URL).
pub fn store_index_parse(data: &[u8], base: &str) -> Result<StoreIndex, String> {
    if data.len() > INDEX_MAX {
        return Err("The Store catalog is too large.".into());
    }
    let mut toks = vec![JsonTok::default(); INDEX_TOKENS];
    let n = json_parse(data, &mut toks);
    if n < 0 {
        return Err("The Store catalog is not valid JSON.".into());
    }
    let toks = &toks[..n as usize];
    if toks[0].type_ != JsonType::Object {
        return Err("The Store catalog is not an object.".into());
    }
    let version = json_get(data, toks, 0, "version");
    let mut v = 0i32;
    if version < 0 || !json_int(data, &toks[version as usize], &mut v) || v != 1 {
        return Err("This Store catalog needs a newer Grimoire.".into());
    }
    let arr = json_get(data, toks, 0, "packages");
    if arr < 0 || toks[arr as usize].type_ != JsonType::Array {
        return Err("The Store catalog has no packages list.".into());
    }
    let mut index = StoreIndex {
        name: jstr(data, toks, 0, "name", 160),
        tagline: jstr(data, toks, 0, "tagline", 512),
        updated: jstr(data, toks, 0, "updated", 64),
        featured: jstr(data, toks, 0, "featured", 128),
        ..Default::default()
    };
    if index.name.is_empty() {
        index.name = "Package Store".into();
    }
    let count = toks[arr as usize].size.min(PACKAGE_MAX as i32);
    for i in 0..count {
        let it = json_item(toks, arr, i);
        if it < 0 || toks[it as usize].type_ != JsonType::Object {
            continue;
        }
        let id = jstr(data, toks, it, "id", 128);
        let name = jstr(data, toks, it, "name", 160);
        let version = jstr(data, toks, it, "version", 64);
        let kind = jstr(data, toks, it, "type", 16);
        if !store_id_ok(&id) || name.is_empty() || version.is_empty() || !matches!(kind.as_str(), "app" | "widget" | "theme") {
            continue;
        }
        if index.packages.iter().any(|p| p.id == id) {
            continue;
        }
        let manifest = jstr(data, toks, it, "manifest", 1024);
        let Some(manifest_url) = resolve_url(base, &manifest) else { continue };
        let icon = icon_ref(base, &jstr(data, toks, it, "icon", 1024));
        let mut screenshots = Vec::new();
        for s in jlist(data, toks, it, "screenshots", 12) {
            if let Some(a) = asset(base, &s) {
                screenshots.push(a);
            }
        }
        index.packages.push(StorePackage {
            id,
            name,
            version,
            kind,
            author: jstr(data, toks, it, "author", 160),
            category: jstr(data, toks, it, "category", 80),
            summary: jstr(data, toks, it, "summary", 512),
            description: jstr(data, toks, it, "description", 8192),
            license: jstr(data, toks, it, "license", 160),
            repository: jstr(data, toks, it, "repository", 1024),
            homepage: jstr(data, toks, it, "homepage", 1024),
            manifest_url,
            icon,
            screenshots,
            tags: jlist(data, toks, it, "tags", 12),
            added: jstr(data, toks, it, "added", 64),
        });
    }
    if index.packages.is_empty() {
        return Err("The Store catalog has no packages that this Grimoire can show.".into());
    }
    Ok(index)
}

// ---- opening and fetching ----

fn start_catalog(ui: &mut PackageStoreUi) {
    if ui.catalog_job.is_some() {
        return;
    }
    if offline() {
        if ui.index.packages.is_empty() {
            ui.error = tr("No saved catalog. Connect to the Internet, then tap Refresh.").to_string();
            ui.view = StoreView::Error;
        } else {
            ui.status = tr("Showing the saved catalog.").to_string();
        }
        return;
    }
    match WidgetFetch::start(&fresh_url(&ui.index_url), INDEX_MAX) {
        Ok(fetch) => {
            ui.catalog_job = Some(FetchJob {
                stage: FetchStage::Catalog,
                fetch,
                url: ui.index_url.clone(),
                key: String::new(),
            });
            ui.refreshing = true;
            if ui.index.packages.is_empty() {
                ui.view = StoreView::Busy;
                ui.status = tr("Reading the Store...").to_string();
            } else {
                ui.status = tr("Refreshing the Store...").to_string();
            }
        }
        Err(e) => {
            ui.error = e;
            if ui.index.packages.is_empty() {
                ui.view = StoreView::Error;
            } else {
                ui.status = tr("Couldn't refresh the Store.").to_string();
            }
        }
    }
}

fn load_cached_index(ui: &mut PackageStoreUi) -> bool {
    let Some(data) = read_file_max(&catalog_cache_path(), INDEX_MAX) else {
        return false;
    };
    match store_index_parse(&data, &ui.index_url) {
        Ok(index) => {
            ui.index = index;
            true
        }
        Err(e) => {
            qlog!("package store: saved catalog refused: %s", &e);
            false
        }
    }
}

pub fn store_open(a: &mut App) {
    import_development(a);
    let ui = &mut a.store;
    ui.open = true;
    ui.view = StoreView::List;
    ui.filter = 0;
    ui.page = 0;
    ui.query.clear();
    ui.status.clear();
    ui.error.clear();
    ui.index = StoreIndex::default();
    ui.selected = None;
    ui.cancel_jobs();
    ui.clear_selection();
    ui.reset_list_icons();
    keyboard_close(&mut ui.kb);
    refresh_installed(ui);
    ui.index_url = catalog_url();
    let cached = load_cached_index(ui);
    if cached {
        ui.reset_list_icons();
        ui.status = tr("Showing the saved catalog.").to_string();
    }
    start_catalog(ui);
    if !cached && ui.index.packages.is_empty() && ui.view != StoreView::Error {
        ui.view = StoreView::Busy;
        ui.status = tr("Reading the Store...").to_string();
    }
}

/// Opens the package half of Settings > Updates. It uses the Store's cached catalog, icons and
/// install path, but never puts the Store screen in front of the Library.
pub fn updates_open(a: &mut App) {
    let ui = &mut a.store;
    ui.updates_center = true;
    ui.update_installing = false;
    ui.update_target.clear();
    ui.update_status.clear();
    ui.update_error.clear();
    ui.selected = None;
    ui.cancel_jobs();
    ui.clear_selection();
    ui.reset_list_icons();
    ui.index = StoreIndex::default();
    ui.status.clear();
    ui.error.clear();
    ui.index_url = catalog_url();
    let cached = load_cached_index(ui);
    if cached {
        ui.reset_list_icons();
        ui.status = tr("Showing the saved catalog.").to_string();
    }
    start_catalog(ui);
    if !cached && ui.index.packages.is_empty() && ui.view != StoreView::Error {
        ui.view = StoreView::Busy;
        ui.status = tr("Reading the Store...").to_string();
    }
}

/// Settings > Updates is leaving: nothing keeps downloading behind it. The catalog and its icons
/// stay cached for the next visit.
pub fn updates_close(a: &mut App) {
    let ui = &mut a.store;
    if !ui.updates_center {
        return;
    }
    ui.updates_center = false;
    ui.cancel_jobs();
    ui.selected = None;
    ui.clear_selection();
    ui.update_installing = false;
    ui.update_target.clear();
    ui.update_status.clear();
    ui.update_error.clear();
}

pub fn updates_tick(a: &mut App, _now: u32) {
    if !a.store.updates_center {
        return;
    }
    let mut changed = tick_catalog(a);
    changed |= tick_install(a);
    // (a picture's download that ended: store_tick has why it is no whole redraw)
    let mut came = tick_asset(a);
    changed |= start_list_icons(a);
    came |= tick_pictures(&mut a.store);
    if changed {
        crate::library::store_redraw(a);
    } else if came {
        crate::library::picture_came(a);
    }
}

pub fn updates_refresh(a: &mut App) {
    if !a.store.updates_center {
        return;
    }
    a.store.error.clear();
    start_catalog(&mut a.store);
    crate::library::store_redraw(a);
}

/// Every installed package whose catalog listing is newer, by name.
pub fn package_updates(ui: &PackageStoreUi) -> Vec<PackageUpdate> {
    let mut out = Vec::new();
    for p in &ui.index.packages {
        let Some(it) = installed_for(p) else { continue };
        if update_version_cmp(&p.version, &it.version) <= 0 {
            continue;
        }
        out.push(PackageUpdate {
            package: p.clone(),
            installed_version: it.version,
        });
    }
    out.sort_by(|a, b| {
        a.package
            .name
            .to_lowercase()
            .cmp(&b.package.name.to_lowercase())
            .then_with(|| a.package.id.cmp(&b.package.id))
    });
    out
}

/// A package's catalog icon, once its cached image has been decoded (start_list_icons fills these).
pub fn update_icon<'a>(ui: &'a PackageStoreUi, id: &str) -> Option<&'a Canvas> {
    let i = ui.index.packages.iter().position(|p| p.id == id)?;
    ui.list_icons.get(i).filter(|icon| !icon.px.is_empty())
}

/// Starts installing the catalog's newer version of `id`, through the same manifest, artifact and
/// hash checks as a Store install. The Updates page stays in front while it downloads.
pub fn updates_install(a: &mut App, id: &str) {
    let ui = &mut a.store;
    if !ui.updates_center || ui.update_installing {
        return;
    }
    let Some(pkg) = ui.index.packages.iter().find(|p| p.id == id).cloned() else { return };
    let newer = installed_for(&pkg).is_some_and(|it| update_version_cmp(&pkg.version, &it.version) > 0);
    if !newer {
        return;
    }
    ui.selected = None;
    ui.clear_selection();
    ui.selected = Some(pkg.id.clone());
    ui.update_target = pkg.id.clone();
    ui.update_installing = true;
    ui.update_status = tr("Downloading update...").to_string();
    ui.update_error.clear();
    ui.status.clear();
    ui.error.clear();
    ui.manifest_url = pkg.manifest_url.clone();
    if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Manifest, &pkg.manifest_url, WIDGET_PKG_MANIFEST_MAX, "") {
        update_failed(a, e);
    }
    crate::library::store_redraw(a);
}

fn close(a: &mut App) {
    a.store.cancel_jobs();
    a.store.open = false;
    a.store.clear_selection();
    crate::library::store_closed(a);
}

/// The Library is leaving (a notebook opened, a turn): keep nothing downloading behind it.
pub fn store_leave(a: &mut App) {
    if a.store.open {
        a.store.cancel_jobs();
        a.store.open = false;
        a.store.clear_selection();
    }
}

fn redraw(a: &mut App) {
    crate::library::store_redraw(a);
}

/// One of the list's packages: a catalog entry (the All, Apps and Widgets tabs) or an installed
/// one (the Installed tab).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PkgRef {
    Catalog(usize),
    Installed(usize),
}

fn pkg_matches(p: &StorePackage, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    p.name.to_lowercase().contains(&q)
        || p.id.to_lowercase().contains(&q)
        || p.author.to_lowercase().contains(&q)
        || p.category.to_lowercase().contains(&q)
        || p.summary.to_lowercase().contains(&q)
        || p.description.to_lowercase().contains(&q)
        || p.tags.iter().any(|t| t.to_lowercase().contains(&q))
}

/// The list's chips: All, Apps, Widgets, Themes, then the installed packages.
const FILTER_INSTALLED: i32 = 4;

fn filtered(ui: &PackageStoreUi) -> Vec<PkgRef> {
    let mut out = Vec::new();
    if ui.filter == FILTER_INSTALLED {
        for (i, p) in ui.installed.iter().enumerate() {
            if pkg_matches(p, &ui.query) {
                out.push(PkgRef::Installed(i));
            }
        }
        return out;
    }
    for (i, p) in ui.index.packages.iter().enumerate() {
        let kind = match ui.filter {
            1 => p.kind == "app",
            2 => p.kind == "widget",
            3 => p.kind == "theme",
            _ => true,
        };
        if kind && pkg_matches(p, &ui.query) {
            out.push(PkgRef::Catalog(i));
        }
    }
    out
}

fn pkg_at(ui: &PackageStoreUi, item: PkgRef) -> Option<&StorePackage> {
    match item {
        PkgRef::Catalog(i) => ui.index.packages.get(i),
        PkgRef::Installed(i) => ui.installed.get(i),
    }
}

fn card_arg(item: PkgRef) -> i32 {
    match item {
        PkgRef::Catalog(i) => i as i32,
        PkgRef::Installed(i) => INSTALLED_CARD + i as i32,
    }
}

fn card_ref(n: i32) -> Option<PkgRef> {
    if n >= INSTALLED_CARD {
        Some(PkgRef::Installed((n - INSTALLED_CARD) as usize))
    } else if n >= 0 {
        Some(PkgRef::Catalog(n as usize))
    } else {
        None
    }
}

/// An installed package as a catalog entry looks, so the list and the detail page show both alike.
fn installed_package(it: &WidgetInstall, kind: &str) -> StorePackage {
    let manifest = installed_manifest(kind, &it.id);
    let icon = if kind == "widget" {
        if it.icon.trim().is_empty() { IconKind::Grid } else { widget_icon(&it.icon) }
    } else {
        kind_glyph(kind)
    };
    StorePackage {
        id: it.id.clone(),
        name: it.name.clone(),
        version: it.version.clone(),
        kind: kind.to_string(),
        author: it.author.clone(),
        summary: it.description.clone(),
        description: it.description.clone(),
        license: manifest.as_ref().map_or(String::new(), |m| m.license.clone()),
        repository: widget_github_repo_url(&it.source_url).unwrap_or_else(|| it.source_url.clone()),
        manifest_url: it.source_url.clone(),
        icon: StoreIconRef::Glyph(icon),
        ..Default::default()
    }
}

fn refresh_installed(ui: &mut PackageStoreUi) {
    let mut out = Vec::new();
    for it in widget_package_list() {
        out.push(installed_package(&it, "widget"));
    }
    for it in app_package_list() {
        out.push(installed_package(&it, "app"));
    }
    for it in theme_package_list() {
        out.push(installed_package(&it, "theme"));
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
    ui.installed = out;
}

/// The package folders under `packages/import/`: the folder itself (a repository cloned into it) and
/// one level of folders inside it (a repository cloned there), each only when it holds a
/// `package.yaml`. A link is not a folder of the user's own and is left out.
fn dev_import_folders() -> Vec<String> {
    let base = format!("{}/{DEV_IMPORT_DIR}", store_root());
    let mut out = Vec::new();
    let holds_manifest = |dir: &std::path::Path| dir.join("package.yaml").is_file();
    if holds_manifest(std::path::Path::new(&base)) {
        out.push(base.clone());
    }
    let Ok(entries) = std::fs::read_dir(&base) else { return out };
    let mut dirs: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|d| holds_manifest(d))
        .map(|d| d.to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    out.append(&mut dirs);
    out.truncate(DEV_IMPORT_MAX);
    out
}

/// A path inside a package's folder, as its manifest names it: relative, with no `.` or `..` part and
/// no backslash, the rules an installed package's `artifact.path` meets too.
fn dev_rel_ok(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 240
        && !p.starts_with('/')
        && !p.contains('\\')
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Installs one folder left in `packages/import/`: its manifest, its artifact and (for an app) the
/// `.ico` its spec names, all read from beside it, then the package manager's own checks on the way in.
/// A package already installed at that version and artifact is left alone; a newer one, and a rebuilt
/// one at the same version, replaces it with its state kept. Returns the package's id and what the
/// note should say, empty when there is nothing to say.
fn import_dev_folder(dir: &str, w: i32, h: i32) -> Result<(String, String), String> {
    let label = dir.rsplit('/').next().unwrap_or(dir).to_string();
    let text = read_file_max(&format!("{dir}/package.yaml"), WIDGET_PKG_MANIFEST_MAX)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .ok_or_else(|| format!("{label}: package.yaml could not be read"))?;
    let p = widget_package_parse(&text).map_err(|e| format!("{label}: {e}"))?;
    if !matches!(p.type_.as_str(), "app" | "widget" | "theme") {
        return Err(format!("{label}: package.yaml names no package type"));
    }
    let (want_sha, _) = widget_package_artifact_digest(&p);
    let installed = installed_of(&p.type_, &p.id);
    if let Some(it) = installed.as_ref() {
        let rebuilt = it.version == p.version && !it.sha.eq_ignore_ascii_case(want_sha);
        let newer = update_version_cmp(&p.version, &it.version) > 0;
        if !newer && !rebuilt {
            // (already installed, or at a version newer than the folder was left at)
            return Ok((p.id.clone(), String::new()));
        }
    }
    if !dev_rel_ok(&p.artifact.path) {
        return Err(format!("{label}: artifact.path is not a safe relative path"));
    }
    let Some(artifact) = read_file_max(&format!("{dir}/{}", p.artifact.path), WIDGET_PKG_ARTIFACT_MAX as usize) else {
        return Err(format!("{label}: {} could not be read beside package.yaml", p.artifact.path));
    };
    let icon = if p.type_ == "app" && app_icon_is_file(&p.spec.icon) {
        let icon = p.spec.icon.trim();
        if !dev_rel_ok(icon) {
            return Err(format!("{label}: spec.icon is not a safe relative path"));
        }
        match read_file_max(&format!("{dir}/{icon}"), INSTALL_ICON_MAX) {
            Some(bytes) => Some(bytes),
            None => return Err(format!("{label}: {icon} could not be read beside package.yaml")),
        }
    } else {
        None
    };
    // (the source an install records: the repository the package names, when it names one)
    let repository = p.repository.trim();
    let source = if repository.starts_with("https://") { repository.to_string() } else { String::new() };
    let it = match p.type_.as_str() {
        "app" => app_package_install(&text, &artifact, icon.as_deref(), &source, w, h),
        "theme" => theme_package_install(&text, &artifact, &source),
        _ => widget_package_install(&text, &artifact, &source, w, h),
    }
    .map_err(|e| format!("{label}: {e}"))?;
    qlog!("packages: %s %s imported from %s", &it.name, &it.version, dir);
    Ok((it.id.clone(), format!("{} {}", tr("Imported"), it.name)))
}

/// Imports every package left in `packages/import/` and writes what happened on the note the Store's
/// list shows: what was imported, and what could not be (so a developer sees a bad manifest or a
/// missing artifact without the log).
fn import_development_for(ui: &mut PackageStoreUi, w: i32, h: i32) {
    let mut imported = Vec::new();
    let mut failed = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for dir in dev_import_folders() {
        match import_dev_folder(&dir, w, h) {
            // (one package per id: two folders of one id would replace each other on every open)
            Ok((id, _)) if seen.contains(&id) => {}
            Ok((id, line)) => {
                seen.push(id);
                if !line.is_empty() {
                    imported.push(line);
                }
            }
            Err(e) => {
                qlog!("packages: %s", &e);
                failed.push(e);
            }
        }
    }
    let mut note = String::new();
    if !imported.is_empty() {
        note = format!("{}: {}", tr("Imported from packages/import"), imported.join(", "));
    }
    if !failed.is_empty() {
        if !note.is_empty() {
            note.push_str(" · ");
        }
        note.push_str(&format!("{}: {}", tr("Couldn't import"), failed.join("; ")));
    }
    ui.dev_note = note;
}

/// What the Store does as it opens: a developer's packages are put in before anything is listed.
fn import_development(a: &mut App) {
    let (w, h) = (a.w, a.h - status_reserved(a));
    import_development_for(&mut a.store, w, h);
    if !a.store.dev_note.is_empty() {
        refresh_installed(&mut a.store);
        themes_changed(a);
    }
}

/// The installed themes are read again after one is installed, updated or removed; the theme in use stays the one
/// it is by its id (Gothic when it was the one removed), and is drawn again from what its package says now.
fn themes_changed(a: &mut App) {
    let id = ui_theme().id();
    crate::looks::package::package_looks_reload();
    let t = UiTheme::from_id(id.as_bytes()).unwrap_or(UiTheme::Gothic);
    a.settings.theme = t.index();
    ui_set_theme(t);
    crate::userfonts::userfonts_apply(a);
    httpd_restyle(look().share_pages_gothic());
    if t.id() != id {
        settings_save(&a.settings);
    }
}

/// An installed package's index entry, by its kind.
fn installed_of(kind: &str, id: &str) -> Option<WidgetInstall> {
    match kind {
        "app" => app_package_installed(id),
        "theme" => theme_package_installed(id),
        _ => widget_package_installed(id),
    }
}

/// An installed package's manifest, by its kind.
fn installed_manifest(kind: &str, id: &str) -> Option<WidgetPackage> {
    match kind {
        "app" => app_package_installed_manifest(id),
        "theme" => theme_package_installed_manifest(id),
        _ => widget_package_installed_manifest(id),
    }
}

/// The glyph a package of a kind with no icon of its own is shown with (a theme: Settings' Style row's).
fn kind_glyph(kind: &str) -> IconKind {
    if kind == "theme" { IconKind::Castle } else { IconKind::Package }
}

/// Which package index `id` is in, whether it is installed at all.
fn installed_kind(id: &str) -> Option<&'static str> {
    if app_package_installed(id).is_some() {
        Some("app")
    } else if widget_package_installed(id).is_some() {
        Some("widget")
    } else if theme_package_installed(id).is_some() {
        Some("theme")
    } else {
        None
    }
}

fn installed_for(p: &StorePackage) -> Option<WidgetInstall> {
    match p.kind.as_str() {
        "app" => app_package_list().into_iter().find(|x| x.id == p.id),
        "theme" => theme_package_list().into_iter().find(|x| x.id == p.id),
        _ => widget_package_list().into_iter().find(|x| x.id == p.id),
    }
}

fn install_label(p: &StorePackage) -> (String, ButtonStyle, bool) {
    match installed_for(p) {
        Some(it) if update_version_cmp(&p.version, &it.version) <= 0 => (tr("Installed").to_string(), ButtonStyle::Disabled, false),
        Some(_) => (format!("{} {}", tr("Update"), p.version), ButtonStyle::Primary, true),
        None => (tr("Install").to_string(), ButtonStyle::Primary, true),
    }
}

enum FetchSlot {
    Install,
    Asset,
}

fn begin_fetch(ui: &mut PackageStoreUi, slot: FetchSlot, stage: FetchStage, url: &str, cap: usize, key: &str) -> Result<(), String> {
    // A manifest, an artifact and a license are what an update reads, so they are asked for by an
    // address no cache has seen: a copy kept a few minutes longer would name the version that was
    // replaced. `job.url` stays the address as the catalog or the manifest wrote it.
    let asked = if matches!(stage, FetchStage::Manifest | FetchStage::Artifact | FetchStage::License) { fresh_url(url) } else { url.to_string() };
    let fetch = WidgetFetch::start(&asked, cap)?;
    let job = FetchJob {
        stage,
        fetch,
        url: url.to_string(),
        key: key.to_string(),
    };
    match slot {
        FetchSlot::Install => ui.install_job = Some(job),
        FetchSlot::Asset => ui.asset_job = Some(job),
    }
    Ok(())
}

fn begin_install(a: &mut App) {
    let Some(pkg) = a.store.selected_package().cloned() else { return };
    let newer = installed_for(&pkg).is_some_and(|it| update_version_cmp(&pkg.version, &it.version) > 0);
    if installed_kind(&pkg.id).is_some() && !newer {
        return;
    }
    // A catalog install is not a repository one, whatever the last try left behind.
    a.store.repo_install = false;
    a.store.repo_pkg = None;
    let (_, style, enabled) = install_label(&pkg);
    if !enabled && style == ButtonStyle::Disabled {
        return;
    }
    let ui = &mut a.store;
    ui.error.clear();
    ui.status = tr("Reading package.yaml...").to_string();
    ui.manifest = None;
    ui.manifest_url = pkg.manifest_url.clone();
    ui.manifest_yaml.clear();
    ui.artifact_url.clear();
    ui.icon_url = None;
    ui.install_icon_key.clear();
    ui.pending_artifact.clear();
    ui.view = StoreView::Busy;
    if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Manifest, &pkg.manifest_url, WIDGET_PKG_MANIFEST_MAX, "") {
        ui.error = e;
        ui.view = StoreView::Error;
    }
    redraw(a);
}

/// Starts installing a package that is not in the catalog: `user/repo`, a GitHub address, or an
/// https `.yaml`. Its manifest, artifact and hashes are checked as a catalog install's are.
fn begin_repo_install(a: &mut App, input: &str) {
    let mut input = input.trim().to_string();
    // A pasted GitHub page address (https://github.com/user/repo) is the same repository as its
    // user/repo shorthand.
    if input.starts_with("https://github.com/") || input.starts_with("http://github.com/") {
        if let Some(repo) = widget_github_repo(&input) {
            input = repo;
        }
    }
    let ui = &mut a.store;
    ui.repo_input = input.clone();
    ui.repo_pkg = None;
    ui.selected = None;
    ui.clear_selection();
    ui.repo_install = false;
    ui.error.clear();
    ui.status.clear();
    if input.is_empty() {
        ui.error = tr("Enter a GitHub repository or a .yaml address.").to_string();
        ui.view = StoreView::Error;
        return;
    }
    ui.repo_install = true;
    ui.view = StoreView::Busy;
    ui.status = tr("Looking up the repository...").to_string();
    if let Some(api) = widget_github_api(&input) {
        if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Branch, &api, 512 * 1024, "") {
            ui.error = e;
            ui.view = StoreView::Error;
        }
        return;
    }
    match widget_manifest_url(&input) {
        Ok((url, _)) => {
            ui.manifest_url = url.clone();
            ui.status = tr("Reading package.yaml...").to_string();
            if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Manifest, &url, WIDGET_PKG_MANIFEST_MAX, "") {
                ui.error = e;
                ui.view = StoreView::Error;
            }
        }
        Err(e) => {
            ui.error = e;
            ui.view = StoreView::Error;
        }
    }
}

/// The manifest of a repository install: validated by its own kind, then shown on the Trust page.
fn after_repo_manifest(ui: &mut PackageStoreUi, data: &[u8], w: i32, h: i32) {
    let text = String::from_utf8_lossy(data).into_owned();
    let parsed = widget_package_parse(&text).and_then(|p| {
        let checked = match p.type_.as_str() {
            "app" => app_package_validate(&p, w, h),
            "widget" => widget_package_validate(&p, w, h),
            "theme" => theme_package_validate(&p),
            _ => Err("The manifest is not an app, a widget or a theme package.".to_string()),
        };
        checked.map(|_| p)
    });
    let p = match parsed {
        Ok(p) => p,
        Err(e) => {
            ui.error = e;
            ui.view = StoreView::Error;
            return;
        }
    };
    ui.manifest_yaml = text;
    ui.artifact_url = widget_package_artifact_url(&ui.manifest_url, &p).unwrap_or_default();
    ui.icon_url = if p.type_ == "app" { app_package_icon_url(&ui.manifest_url, &p) } else { None };
    ui.install_icon_key.clear();
    if ui.icon_url.is_some() {
        if let Some(key) = safe_key(&format!("{}/{}", p.id, p.spec.icon)) {
            ui.install_icon_key = key;
        }
    }
    let icon = if p.type_ == "widget" {
        if p.spec.icon.trim().is_empty() { IconKind::Grid } else { widget_icon(&p.spec.icon) }
    } else {
        kind_glyph(&p.type_)
    };
    let pkg = StorePackage {
        id: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone(),
        kind: p.type_.clone(),
        author: p.author.clone(),
        summary: p.description.clone(),
        description: p.description.clone(),
        license: p.license.clone(),
        repository: p.repository.clone(),
        homepage: p.repository.clone(),
        manifest_url: ui.manifest_url.clone(),
        icon: StoreIconRef::Glyph(icon),
        ..Default::default()
    };
    ui.selected = Some(pkg.id.clone());
    ui.repo_pkg = Some(pkg);
    ui.manifest = Some(p);
    ui.status.clear();
    ui.error.clear();
    ui.view = StoreView::Trust;
}

fn start_artifact(a: &mut App) {
    let Some(m) = a.store.manifest.as_ref() else { return };
    let Some(url) = widget_package_artifact_url(&a.store.manifest_url, m) else {
        a.store.error = tr("The manifest names no artifact that can be downloaded.").to_string();
        a.store.view = StoreView::Error;
        redraw(a);
        return;
    };
    let ui = &mut a.store;
    ui.artifact_url = url.clone();
    ui.error.clear();
    ui.status = tr("Downloading the package...").to_string();
    ui.view = StoreView::Busy;
    if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Artifact, &url, WIDGET_PKG_ARTIFACT_MAX as usize, "") {
        ui.error = e;
        ui.view = StoreView::Error;
    }
    redraw(a);
}

fn start_install_icon(a: &mut App) {
    let icon_url = a.store.icon_url.clone();
    let Some(url) = icon_url else {
        install_pending(a, None);
        return;
    };
    let key = a.store.install_icon_key.clone();
    if let Some(data) = read_cached_asset(&key, INSTALL_ICON_MAX) {
        install_pending(a, Some(&data));
        return;
    }
    a.store.status = tr("Downloading the package icon...").to_string();
    a.store.view = StoreView::Busy;
    if let Err(e) = begin_fetch(&mut a.store, FetchSlot::Install, FetchStage::InstallIcon, &url, INSTALL_ICON_MAX, &key) {
        a.store.error = e;
        a.store.view = StoreView::Error;
    }
    redraw(a);
}

fn validate_for_kind(p: &WidgetPackage, kind: &str, w: i32, h: i32) -> Result<(), String> {
    match kind {
        "app" => app_package_validate(p, w, h),
        "widget" => widget_package_validate(p, w, h),
        "theme" => theme_package_validate(p),
        _ => Err("The Store does not know this package kind.".into()),
    }
}

/// Installs what the last download left ready. The caller has validated the manifest; the
/// installers check the artifact's size and hash again before writing anything.
fn install_from_pending(a: &mut App, icon: Option<&[u8]>) -> Result<(String, String), String> {
    let Some(m) = a.store.manifest.clone() else {
        return Err(tr("The package's manifest was lost.").to_string());
    };
    let manifest = std::mem::take(&mut a.store.manifest_yaml);
    let artifact = std::mem::take(&mut a.store.pending_artifact);
    let source = a.store.manifest_url.clone();
    let kind = if a.store.repo_install {
        m.type_.clone()
    } else {
        a.store
            .index
            .packages
            .iter()
            .find(|p| p.id == m.id)
            .map(|p| p.kind.clone())
            .ok_or_else(|| tr("That package is no longer in the Store.").to_string())?
    };
    let it = if kind == "app" {
        app_package_install(&manifest, &artifact, icon, &source, a.w, a.h - status_reserved(a))?
    } else if kind == "widget" {
        widget_package_install(&manifest, &artifact, &source, a.w, a.h - status_reserved(a))?
    } else if kind == "theme" {
        let it = theme_package_install(&manifest, &artifact, &source)?;
        themes_changed(a);
        it
    } else {
        return Err(tr("The Store does not know this package kind.").to_string());
    };
    Ok((it.name, it.version))
}

fn install_pending(a: &mut App, icon: Option<&[u8]>) {
    match install_from_pending(a, icon) {
        Ok((name, _)) => {
            let repo = a.store.repo_install;
            a.store.status = format!("{} {}", tr("Installed"), name);
            a.store.error.clear();
            a.store.repo_install = false;
            a.store.repo_input.clear();
            a.store.repo_pkg = None;
            refresh_installed(&mut a.store);
            if repo {
                a.store.selected = None;
                a.store.filter = FILTER_INSTALLED;
                a.store.query.clear();
                a.store.page = 0;
                a.store.view = StoreView::List;
            } else {
                a.store.view = StoreView::Detail;
            }
        }
        Err(e) => {
            a.store.error = e;
            a.store.view = StoreView::Error;
        }
    }
}

fn install_update_pending(a: &mut App, icon: Option<&[u8]>) {
    match install_from_pending(a, icon) {
        Ok((name, version)) => {
            let ui = &mut a.store;
            ui.update_installing = false;
            ui.update_target.clear();
            ui.update_error.clear();
            ui.manifest = None;
            ui.manifest_url.clear();
            ui.manifest_yaml.clear();
            ui.artifact_url.clear();
            ui.icon_url = None;
            ui.install_icon_key.clear();
            ui.pending_artifact.clear();
            let mut msg = String::new();
            csnprintf!(&mut msg, 300, tr("Updated %s to %s."), &name, &version);
            ui.update_status = msg;
        }
        Err(e) => update_failed(a, e),
    }
}

fn start_selected_assets(a: &mut App) {
    let Some(pkg) = a.store.selected_package().cloned() else { return };
    if a.store.icon_for != pkg.id {
        a.store.icon.reset();
        a.store.icon_for = pkg.id.clone();
        a.store.icon_tried = false;
        a.store.drop_waits(&|stage| stage == FetchStage::Icon);
        if let StoreIconRef::Glyph(g) = pkg.icon.clone() {
            a.store.icon.glyph = g;
        }
    }
    if a.store.shot_for != pkg.id {
        canvas_free(&mut a.store.shot);
        a.store.shot_for = pkg.id.clone();
        a.store.shot_tried = false;
        a.store.shot_index = 0;
        a.store.drop_waits(&|stage| stage == FetchStage::Screenshot);
    }
    if a.store.asset_job.is_some() {
        return;
    }
    if !a.store.icon_tried {
        if let StoreIconRef::Image(asset) = pkg.icon.clone() {
            match asset_picture(&mut a.store, FetchStage::Icon, &asset.key) {
                PicNow::NoFile => {
                    start_asset_job(a, FetchStage::Icon, &asset.url, INSTALL_ICON_MAX, &asset.key);
                    return;
                }
                now => {
                    place_picture(&mut a.store, FetchStage::Icon, &asset.key, now);
                    a.store.icon_tried = true;
                }
            }
        } else {
            a.store.icon_tried = true;
        }
    }
    if !a.store.shot_tried {
        if let Some(asset) = pkg.screenshots.get(a.store.shot_index as usize).cloned() {
            match asset_picture(&mut a.store, FetchStage::Screenshot, &asset.key) {
                PicNow::NoFile => {
                    start_asset_job(a, FetchStage::Screenshot, &asset.url, SCREENSHOT_MAX, &asset.key);
                    return;
                }
                now => {
                    place_picture(&mut a.store, FetchStage::Screenshot, &asset.key, now);
                    a.store.shot_tried = true;
                }
            }
        } else {
            a.store.shot_tried = true;
        }
    }
}

fn start_asset_job(a: &mut App, stage: FetchStage, url: &str, cap: usize, key: &str) {
    if let Err(e) = begin_fetch(&mut a.store, FetchSlot::Asset, stage, url, cap, key) {
        qlog!("package store: asset fetch failed to start: %s", &e);
        match stage {
            FetchStage::Icon => a.store.icon_tried = true,
            FetchStage::Screenshot => a.store.shot_tried = true,
            FetchStage::ListIcon => mark_list_icon_tried(&mut a.store, key),
            _ => {}
        }
    }
}

/// Load the catalog cards' own icons, one cached image at a time and one download at a time. An
/// icon whose file is cached shows the worker's picture of it: at once when that is in its cache,
/// else once its job has made it (`tick_pictures`).
/// Returns whether an image was loaded without waiting on the network or the worker.
fn start_list_icons(a: &mut App) -> bool {
    let showing = a.store.updates_center || (a.store.open && a.store.view == StoreView::List);
    if !showing || a.store.asset_job.is_some() {
        return false;
    }
    if a.store.list_icons.len() != a.store.index.packages.len() || a.store.list_tried.len() != a.store.index.packages.len() {
        a.store.reset_list_icons();
    }
    let mut loaded = false;
    for i in 0..a.store.index.packages.len() {
        let has_image = a.store.list_icons.get(i).is_some_and(|icon| !icon.px.is_empty());
        if has_image || a.store.list_tried.get(i).copied().unwrap_or(false) {
            continue;
        }
        let Some((url, key)) = (match &a.store.index.packages[i].icon {
            StoreIconRef::Image(asset) => Some((asset.url.clone(), asset.key.clone())),
            StoreIconRef::Glyph(_) => None,
        }) else {
            continue;
        };
        match asset_picture(&mut a.store, FetchStage::ListIcon, &key) {
            PicNow::NoFile => {
                start_asset_job(a, FetchStage::ListIcon, &url, INSTALL_ICON_MAX, &key);
                return loaded;
            }
            now => {
                // (this card's own slot: two packages may name one icon file)
                if let (PicNow::There(image), Some(slot)) = (now, a.store.list_icons.get_mut(i)) {
                    *slot = image;
                    loaded = true;
                }
                a.store.list_tried[i] = true;
            }
        }
    }
    loaded
}

/// The picture of the cached asset `key` for `stage`'s slot (`Icon` and `ListIcon`: a catalog icon;
/// `Screenshot`). The asset's file is looked at, never read (one stat); its picture is the one the
/// sandboxed worker made, read from the worker's cache and checked (`pkgpics::picture_cached`).
/// With nothing there the picture is asked for (`tick_pictures` queues its job and takes it in).
fn asset_picture(ui: &mut PackageStoreUi, stage: FetchStage, key: &str) -> PicNow {
    if safe_key(key).as_deref() != Some(key) {
        return PicNow::NoFile;
    }
    // (a key with a control character names no file a picture's job can be asked for: no picture, and nothing
    // to download for one)
    if key.chars().any(|c| c.is_control()) {
        return PicNow::None;
    }
    let form = if stage == FetchStage::Screenshot { PicForm::Screenshot } else { PicForm::StoreIcon };
    let Some(src) = PicSrc::of(&asset_cache_path(key), form) else { return PicNow::NoFile };
    match picture_cached(&src) {
        Cached::Picture(c) => PicNow::There(c),
        Cached::None => PicNow::None,
        Cached::Nothing => {
            if !ui.pic_waits.iter().any(|w| w.stage == stage && w.key == key) {
                ui.pic_waits.push(PicWait { stage, key: key.to_string(), src, job: None });
            }
            PicNow::Coming
        }
    }
}

/// A picture that is there goes to the slot it was asked for, if that slot still shows the asset
/// `key`: the icon of every card whose package names it and has none yet, the detail page's icon,
/// or its screenshot. True when it was placed.
fn place_picture(ui: &mut PackageStoreUi, stage: FetchStage, key: &str, now: PicNow) -> bool {
    let PicNow::There(image) = now else { return false };
    match stage {
        FetchStage::ListIcon => {
            let mut placed = false;
            for (i, p) in ui.index.packages.iter().enumerate() {
                let names = matches!(&p.icon, StoreIconRef::Image(a) if a.key == key);
                if let Some(slot) = ui.list_icons.get_mut(i).filter(|slot| names && slot.px.is_empty()) {
                    *slot = image.clone();
                    placed = true;
                }
            }
            placed
        }
        FetchStage::Icon => {
            let shows = ui.selected_package().is_some_and(|p| p.id == ui.icon_for && matches!(&p.icon, StoreIconRef::Image(a) if a.key == key));
            if shows {
                ui.icon.image = image;
            }
            shows
        }
        FetchStage::Screenshot => {
            let shows = ui.selected_package().is_some_and(|p| p.id == ui.shot_for && p.screenshots.get(ui.shot_index as usize).is_some_and(|a| a.key == key));
            if shows {
                ui.shot = image;
            }
            shows
        }
        _ => false,
    }
}

/// Whether the screen in front shows the slot of `stage` that the asset `key` fills: a card of the
/// list's page as it was last drawn (its hits), the detail page's icon or screenshot while that page
/// is up; on Settings > Updates any row (they are the Library's to lay out). A picture that comes
/// for a slot that is not shown (a card of another page, the list's while a page is up) is kept in
/// its slot and nothing is drawn or compared for it: the page that shows it later draws it.
fn slot_in_front(ui: &PackageStoreUi, stage: FetchStage, key: &str) -> bool {
    if !ui.open {
        return ui.updates_center;
    }
    match stage {
        FetchStage::ListIcon => {
            let names = |n: i32| matches!(ui.index.packages.get(n as usize).map(|p| &p.icon), Some(StoreIconRef::Image(a)) if a.key == key);
            ui.view == StoreView::List && ui.hits.h.iter().any(|h| h.id == H_CARD && names(h.arg))
        }
        _ => ui.view == StoreView::Detail,
    }
}

/// One round, at each tick: the pictures still to ask for are asked of the worker's job (a few
/// jobs at a time: `picture_request`), and those whose job has ended are looked up again and go to
/// their slots. A job that ended with nothing (a refused file, no worker) leaves the slot as it is.
/// The pictures of what is in front are asked for first: a package's page's icon and screenshot
/// before the list's icons while a page is up, and for a page's picture still to ask for, the
/// list's queued jobs leave the queue (`unqueue_list_pictures`), so it runs next and not behind
/// them. True when a picture came for a slot the screen in front shows (`slot_in_front`), or the
/// page's screenshot's job ended without one (its frame is drawn again).
fn tick_pictures(ui: &mut PackageStoreUi) -> bool {
    let of_page = |stage: FetchStage| matches!(stage, FetchStage::Icon | FetchStage::Screenshot);
    let page = ui.open && ui.view != StoreView::List;
    if page && ui.pic_waits.iter().any(|w| of_page(w.stage) && w.job.is_none()) {
        unqueue_list_pictures(ui);
    }
    let mut came = false;
    for first in [true, false] {
        let mut i = 0;
        while i < ui.pic_waits.len() {
            // (this round's: the pictures of what is in front in the first, the others in the second)
            if (of_page(ui.pic_waits[i].stage) == page) != first {
                i += 1;
                continue;
            }
            match ui.pic_waits[i].job.as_deref() {
                None => match picture_request(&ui.pic_waits[i].src) {
                    Ask::Job(job) => ui.pic_waits[i].job = Some(job),
                    Ask::Later => {}
                    Ask::No => {
                        let w = ui.pic_waits.remove(i);
                        came |= w.stage == FetchStage::Screenshot && slot_in_front(ui, w.stage, &w.key);
                        continue;
                    }
                },
                Some(job) if !bg_job_pending(BgKind::PackagePictures, "", job) => {
                    let w = ui.pic_waits.remove(i);
                    let placed = match picture_cached(&w.src) {
                        Cached::Picture(c) => place_picture(ui, w.stage, &w.key, PicNow::There(c)),
                        // (no screenshot comes: its frame no longer says one is loading)
                        _ => w.stage == FetchStage::Screenshot,
                    };
                    came |= placed && slot_in_front(ui, w.stage, &w.key);
                    continue;
                }
                Some(_) => {}
            }
            i += 1;
        }
    }
    came
}

fn mark_list_icon_tried(ui: &mut PackageStoreUi, key: &str) {
    for (i, p) in ui.index.packages.iter().enumerate() {
        if let StoreIconRef::Image(asset) = &p.icon {
            if asset.key == key {
                if let Some(tried) = ui.list_tried.get_mut(i) {
                    *tried = true;
                }
            }
        }
    }
}

fn accept_manifest(ui: &mut PackageStoreUi, data: &[u8], w: i32, h: i32) -> Result<WidgetPackage, String> {
    let text = String::from_utf8_lossy(data);
    let selected = ui
        .selected_package()
        .cloned()
        .ok_or_else(|| tr("The selected package is gone.").to_string())?;
    let p = widget_package_parse(&text).and_then(|p| {
        validate_for_kind(&p, &selected.kind, w, h).map(|_| p)
    })?;
    if p.id != selected.id || p.name != selected.name {
        return Err(tr("The manifest does not match its Store listing.").to_string());
    }
    if update_version_cmp(&p.version, &selected.version) != 0 {
        return Err(tr("The manifest's version does not match its Store listing.").to_string());
    }
    ui.manifest_yaml = text.into_owned();
    ui.artifact_url = widget_package_artifact_url(&ui.manifest_url, &p).unwrap_or_default();
    ui.icon_url = if selected.kind == "app" { app_package_icon_url(&ui.manifest_url, &p) } else { None };
    ui.install_icon_key.clear();
    if ui.icon_url.is_some() {
        if let Some(key) = safe_key(&format!("{}/{}", selected.id, p.spec.icon)) {
            ui.install_icon_key = key;
        }
    }
    Ok(p)
}

fn after_manifest(ui: &mut PackageStoreUi, data: &[u8], w: i32, h: i32) {
    match accept_manifest(ui, data, w, h) {
        Ok(p) => {
            ui.manifest = Some(p);
            ui.status.clear();
            ui.error.clear();
            ui.view = StoreView::Trust;
        }
        Err(e) => {
            ui.error = e;
            ui.view = StoreView::Error;
        }
    }
}

fn update_failed(a: &mut App, error: String) {
    let ui = &mut a.store;
    qlog!("package update: %s", &error);
    ui.update_installing = false;
    ui.update_target.clear();
    ui.update_status.clear();
    ui.update_error = error;
    ui.pending_artifact.clear();
}

fn after_update_manifest(a: &mut App, data: &[u8]) {
    let w = a.w;
    let h = a.h - status_reserved(a);
    match accept_manifest(&mut a.store, data, w, h) {
        Ok(p) => {
            // The update would install terms the user never agreed to: it waits for an accept in the
            // Store's package page, where the license can be read first.
            let was = a.store.selected_package().map(installed_license).unwrap_or_default();
            if !was.is_empty() && !p.license.is_empty() && !was.eq_ignore_ascii_case(&p.license) {
                let mut msg = String::new();
                csnprintf!(
                    &mut msg,
                    500,
                    tr("The update changes the license from %s to %s. Open the package in the Store and accept it there."),
                    &was,
                    &p.license
                );
                update_failed(a, msg);
                return;
            }
            a.store.manifest = Some(p);
            a.store.update_status = tr("Downloading update...").to_string();
            a.store.update_error.clear();
            start_update_artifact(a);
        }
        Err(e) => update_failed(a, e),
    }
}

fn start_update_artifact(a: &mut App) {
    let Some(m) = a.store.manifest.as_ref() else {
        update_failed(a, tr("The package's manifest was lost.").to_string());
        return;
    };
    let Some(url) = widget_package_artifact_url(&a.store.manifest_url, m) else {
        update_failed(a, tr("The manifest names no artifact that can be downloaded.").to_string());
        return;
    };
    let ui = &mut a.store;
    ui.artifact_url = url.clone();
    ui.update_status = tr("Downloading update...").to_string();
    if let Err(e) = begin_fetch(ui, FetchSlot::Install, FetchStage::Artifact, &url, WIDGET_PKG_ARTIFACT_MAX as usize, "") {
        update_failed(a, e);
    }
}

fn start_update_icon(a: &mut App) {
    let icon_url = a.store.icon_url.clone();
    let Some(url) = icon_url else {
        install_update_pending(a, None);
        return;
    };
    let key = a.store.install_icon_key.clone();
    if let Some(data) = read_cached_asset(&key, INSTALL_ICON_MAX) {
        install_update_pending(a, Some(&data));
        return;
    }
    a.store.update_status = tr("Downloading the package icon...").to_string();
    if let Err(e) = begin_fetch(&mut a.store, FetchSlot::Install, FetchStage::InstallIcon, &url, INSTALL_ICON_MAX, &key) {
        update_failed(a, e);
    }
}

fn tick_catalog(a: &mut App) -> bool {
    let Some(mut job) = a.store.catalog_job.take() else { return false };
    let Some(res) = job.fetch.tick() else {
        a.store.catalog_job = Some(job);
        return false;
    };
    a.store.refreshing = false;
    match res {
        Ok(data) => match store_index_parse(&data, &job.url) {
            Ok(index) => {
                write_cached_index(&data);
                let selected = a.store.selected.clone();
                a.store.index = index;
                a.store.reset_list_icons();
                a.store.selected = selected.filter(|id| a.store.index.packages.iter().any(|p| &p.id == id));
                if a.store.selected.is_none() {
                    a.store.clear_selection();
                }
                if a.store.view == StoreView::Busy || a.store.view == StoreView::Error {
                    a.store.view = StoreView::List;
                }
                a.store.status = format!("{} {}", tr("Updated"), a.store.index.updated);
                a.store.error.clear();
                if !a.store.index.packages.is_empty() && a.store.view == StoreView::List {
                    a.store.page = 0;
                }
                true
            }
            Err(e) => {
                if a.store.index.packages.is_empty() {
                    a.store.error = e;
                    a.store.view = StoreView::Error;
                } else {
                    a.store.status = e;
                }
                true
            }
        },
        Err(e) => {
            if a.store.index.packages.is_empty() {
                a.store.error = e;
                a.store.view = StoreView::Error;
            } else {
                a.store.status = e;
            }
            true
        }
    }
}

fn tick_install(a: &mut App) -> bool {
    let Some(mut job) = a.store.install_job.take() else { return false };
    let Some(res) = job.fetch.tick() else {
        a.store.install_job = Some(job);
        return false;
    };
    match res {
        Ok(data) => match job.stage {
            FetchStage::Branch => {
                let input = a.store.repo_input.clone();
                let fallback = widget_manifest_url(&input).map(|(u, _)| u).unwrap_or_default();
                let url = widget_github_branch(&String::from_utf8_lossy(&data))
                    .and_then(|b| {
                        let parts: Vec<&str> = input.split('/').collect();
                        (parts.len() == 2).then(|| format!("https://raw.githubusercontent.com/{}/{}/{b}/package.yaml", parts[0], parts[1]))
                    })
                    .unwrap_or(fallback);
                if url.is_empty() {
                    a.store.error = tr("That repository has no package.yaml.").to_string();
                    a.store.view = StoreView::Error;
                } else {
                    a.store.manifest_url = url.clone();
                    a.store.status = tr("Reading package.yaml...").to_string();
                    if let Err(e) = begin_fetch(&mut a.store, FetchSlot::Install, FetchStage::Manifest, &url, WIDGET_PKG_MANIFEST_MAX, "") {
                        a.store.error = e;
                        a.store.view = StoreView::Error;
                    }
                }
            }
            FetchStage::Manifest => {
                if a.store.update_installing {
                    after_update_manifest(a, &data);
                } else if a.store.repo_install {
                    let w = a.w;
                    let h = a.h - status_reserved(a);
                    after_repo_manifest(&mut a.store, &data, w, h);
                } else {
                    let w = a.w;
                    let h = a.h - status_reserved(a);
                    after_manifest(&mut a.store, &data, w, h);
                }
            }
            FetchStage::Artifact => {
                a.store.pending_artifact = data;
                if a.store.update_installing {
                    if a.store.icon_url.is_some() {
                        start_update_icon(a);
                    } else {
                        install_update_pending(a, None);
                    }
                } else if a.store.icon_url.is_some() {
                    start_install_icon(a);
                } else {
                    install_pending(a, None);
                }
            }
            FetchStage::InstallIcon => {
                if !job.key.is_empty() {
                    write_cached_asset(&job.key, &data);
                }
                let icon = data.clone();
                if a.store.update_installing {
                    install_update_pending(a, Some(&icon));
                } else {
                    install_pending(a, Some(&icon));
                }
            }
            FetchStage::License => {
                let text = String::from_utf8_lossy(&data).into_owned();
                if text.trim().is_empty() {
                    start_next_license(a);
                } else {
                    a.store.license_text = text;
                    a.store.license_page = 0;
                    a.store.license_note.clear();
                }
            }
            _ => {}
        },
        Err(e) => {
            if a.store.update_installing {
                update_failed(a, e);
            } else if job.stage == FetchStage::License {
                start_next_license(a);
            } else if job.stage == FetchStage::Branch {
                // The GitHub API is an optimization: a failed lookup falls back to main's package.yaml.
                let input = a.store.repo_input.clone();
                match widget_manifest_url(&input) {
                    Ok((url, _)) => {
                        a.store.manifest_url = url.clone();
                        if let Err(e) = begin_fetch(&mut a.store, FetchSlot::Install, FetchStage::Manifest, &url, WIDGET_PKG_MANIFEST_MAX, "") {
                            a.store.error = e;
                            a.store.view = StoreView::Error;
                        }
                    }
                    Err(e) => {
                        a.store.error = e;
                        a.store.view = StoreView::Error;
                    }
                }
            } else {
                a.store.error = e;
                a.store.view = StoreView::Error;
            }
        }
    }
    true
}

/// The asset download's poll (a card's icon, a page's icon or screenshot: the picture stages). True when it
/// ended, well or not: the page may show something else now (the compared redraw finds out what).
fn tick_asset(a: &mut App) -> bool {
    let Some(mut job) = a.store.asset_job.take() else { return false };
    let Some(res) = job.fetch.tick() else {
        a.store.asset_job = Some(job);
        return false;
    };
    match res {
        Ok(data) => {
            if !job.key.is_empty() {
                write_cached_asset(&job.key, &data);
            }
            // (the bytes went to `package-store/assets/`: their picture is asked of the worker, never made here)
            if matches!(job.stage, FetchStage::Icon | FetchStage::Screenshot | FetchStage::ListIcon) {
                let now = asset_picture(&mut a.store, job.stage, &job.key);
                place_picture(&mut a.store, job.stage, &job.key, now);
            }
            match job.stage {
                FetchStage::Icon => a.store.icon_tried = true,
                FetchStage::Screenshot => a.store.shot_tried = true,
                FetchStage::ListIcon => mark_list_icon_tried(&mut a.store, &job.key),
                _ => {}
            }
        }
        Err(e) => {
            qlog!("package store: %s", &e);
            match job.stage {
                FetchStage::Icon => a.store.icon_tried = true,
                FetchStage::Screenshot => a.store.shot_tried = true,
                FetchStage::ListIcon => mark_list_icon_tried(&mut a.store, &job.key),
                _ => {}
            }
        }
    }
    if a.store.open && a.store.view == StoreView::Detail {
        start_selected_assets(a);
    } else {
        start_list_icons(a);
    }
    true
}

pub fn store_tick(a: &mut App, _now: u32) {
    if !a.store.open {
        return;
    }
    if a.store.kb.open {
        // The keyboard draws by itself (app_keyboard_pointer): this is its held delete's repeats and
        // the one grayscale clean-up after typing pauses. The page under it does not move.
        let mut kb = std::mem::take(&mut a.store.kb);
        app_keyboard_tick(a, &mut kb, _now);
        a.store.kb = kb;
        return;
    }
    let mut changed = tick_catalog(a);
    changed |= tick_install(a);
    // A picture's download that ended puts nothing new on the screen by itself: its picture is still to come
    // from the worker. So it asks for no redraw of the whole screen, only for the compared one, which refreshes
    // nothing while the page is as it was, the picture's place when the worker's cache had it already, and the
    // screenshot frame's words when its download failed.
    let mut came = tick_asset(a);
    if a.store.view == StoreView::List {
        changed |= start_list_icons(a);
    }
    // (a picture the worker made: the page is drawn again and only its place refreshed)
    came |= tick_pictures(&mut a.store);
    if changed {
        redraw(a);
    } else if came {
        crate::library::picture_came(a);
    }
}

// ---- pointer ----

fn open_detail(a: &mut App, n: i32) {
    let Some(pkg) = card_ref(n).and_then(|r| pkg_at(&a.store, r)).cloned() else { return };
    // A card icon still downloading must not hold up the page the tap asked for.
    if a.store.asset_job.as_ref().is_some_and(|job| job.stage == FetchStage::ListIcon) {
        a.store.asset_job = None;
    }
    // (nor the cards' pictures still queued for the worker)
    unqueue_list_pictures(&mut a.store);
    a.store.selected = Some(pkg.id.clone());
    a.store.desc_more = false;
    a.store.desc_page = 0;
    a.store.license_text.clear();
    a.store.license_note.clear();
    a.store.license_urls.clear();
    a.store.license_try = 0;
    a.store.license_page = 0;
    a.store.view = StoreView::Detail;
    a.store.status.clear();
    a.store.error.clear();
    a.store.shot_index = 0;
    a.store.icon_for.clear();
    a.store.shot_for.clear();
    a.store.icon_tried = false;
    a.store.shot_tried = false;
    start_selected_assets(a);
    redraw(a);
}

/// The license the installed version was installed under (the manifest saved with it), or nothing
/// when the package is not installed.
fn installed_license(p: &StorePackage) -> String {
    if installed_kind(&p.id).is_none() {
        return String::new();
    }
    let m = installed_manifest(&p.kind, &p.id);
    m.map(|m| m.license.trim().to_string()).filter(|l| !l.is_empty()).unwrap_or_default()
}

/// The Trust page's continue: the license is put in front of the user when the package has one
/// (always for a first install, and for an update only when the license differs from the one the
/// installed version was accepted under). With nothing to accept the download starts at once.
fn after_trust(a: &mut App) {
    let Some(p) = a.store.selected_package().cloned() else { return };
    let mut new = a.store.manifest.as_ref().map_or(String::new(), |m| m.license.trim().to_string());
    if new.is_empty() {
        new = p.license.trim().to_string();
    }
    let was = installed_license(&p);
    if new.is_empty() || (!was.is_empty() && was.eq_ignore_ascii_case(&new)) {
        start_artifact(a);
    } else {
        open_agree(a);
    }
}

/// The license of the version on offer, to read and accept before anything is downloaded.
fn open_agree(a: &mut App) {
    let Some(p) = a.store.selected_package().cloned() else { return };
    let urls = match a.store.manifest.as_ref() {
        Some(m) => crate::widgetpkgui::installed_license_candidates(&a.store.manifest_url, m),
        None => {
            let mut out = Vec::new();
            if let Some((base, _)) = p.manifest_url.rsplit_once('/') {
                out.extend(crate::widgetpkgui::license_urls(base));
            }
            if let Some(repo) = widget_github_repo(&p.repository) {
                for branch in ["main", "master"] {
                    out.extend(crate::widgetpkgui::license_urls(&format!("https://raw.githubusercontent.com/{repo}/{branch}")));
                }
            }
            out
        }
    };
    let ui = &mut a.store;
    ui.view = StoreView::Agree;
    ui.install_job = None;
    ui.license_page = 0;
    ui.license_text.clear();
    ui.license_urls = urls;
    ui.license_try = 0;
    ui.license_note.clear();
    if offline() {
        ui.license_note = tr("Connect to the Internet to read the license's text.").to_string();
        redraw(a);
    } else if ui.license_urls.is_empty() {
        ui.license_note = tr("The package names no license text the Store could read.").to_string();
        redraw(a);
    } else {
        start_next_license(a);
    }
}

/// Opens the selected package's license: the installed version's when it is installed (the one the
/// install agreed to), else the one the Store offers. The text comes from the repository (its
/// `license_url`, or the LICENSE file beside the manifest); the license's name shows while it is read.
fn open_license(a: &mut App) {
    let Some(p) = a.store.selected_package().cloned() else { return };
    let installed = installed_manifest(&p.kind, &p.id);
    let source = installed_for(&p).map(|it| it.source_url).unwrap_or_default();
    let mut urls = Vec::new();
    match installed.as_ref() {
        Some(m) => urls = crate::widgetpkgui::installed_license_candidates(&source, m),
        None => {
            if let Some((base, _)) = p.manifest_url.rsplit_once('/') {
                urls.extend(crate::widgetpkgui::license_urls(base));
            }
            if let Some(repo) = widget_github_repo(&p.repository) {
                for branch in ["main", "master"] {
                    urls.extend(crate::widgetpkgui::license_urls(&format!("https://raw.githubusercontent.com/{repo}/{branch}")));
                }
            }
        }
    }
    let ui = &mut a.store;
    ui.view = StoreView::License;
    ui.install_job = None;
    ui.license_page = 0;
    ui.license_text.clear();
    ui.license_urls = urls;
    ui.license_try = 0;
    ui.license_note.clear();
    if offline() {
        ui.license_note = tr("Connect to the Internet to read the license's text.").to_string();
        redraw(a);
    } else if ui.license_urls.is_empty() {
        ui.license_note = tr("The repository has no license text the Store could read.").to_string();
        redraw(a);
    } else {
        start_next_license(a);
    }
}

/// Asks for the next license file the repository may keep. With none left the page says so (the
/// license's name still shows).
fn start_next_license(a: &mut App) {
    let mut started = false;
    {
        let ui = &mut a.store;
        while ui.license_try < ui.license_urls.len() {
            let url = ui.license_urls[ui.license_try].clone();
            ui.license_try += 1;
            ui.license_note = tr("Reading the license...").to_string();
            if begin_fetch(ui, FetchSlot::Install, FetchStage::License, &url, LICENSE_MAX, "").is_ok() {
                started = true;
                break;
            }
        }
        if !started {
            ui.license_text.clear();
            ui.license_note = tr("The repository has no license text the Store could read.").to_string();
        }
    }
    redraw(a);
}

/// Removes the selected package from the Kindle (its files, not the user's layout) and returns to
/// the Installed tab.
fn uninstall_selected(a: &mut App) {
    let Some(pkg) = a.store.selected_package().cloned() else { return };
    let kind = installed_kind(&pkg.id).unwrap_or(pkg.kind.as_str());
    let res = match kind {
        "app" => app_package_remove(&pkg.id, true),
        "theme" => theme_package_remove(&pkg.id),
        _ => widget_package_remove(&pkg.id, true),
    };
    match res {
        Ok(()) => {
            // An open frame or app goes with its package.
            if kind == "widget" {
                if let Some(ed) = a.ed.as_deref_mut() {
                    while let Some(i) = ed.widgets.index_of(&pkg.id) {
                        ed.widgets.close(i);
                    }
                    ed.widgets.save();
                }
            } else if kind == "theme" {
                themes_changed(a);
            } else if a.app_view.open && a.app_view.id == pkg.id {
                crate::apphost::apphost_close(a);
            }
            a.store.selected = None;
            a.store.clear_selection();
            refresh_installed(&mut a.store);
            a.store.filter = FILTER_INSTALLED;
            a.store.query.clear();
            a.store.page = 0;
            a.store.view = StoreView::List;
            a.store.error.clear();
            a.store.status = format!("{} {}", tr("Uninstalled"), pkg.name);
            redraw(a);
        }
        Err(e) => {
            a.store.error = e;
            a.store.view = StoreView::Error;
            redraw(a);
        }
    }
}

fn shot_move(a: &mut App, step: i32) {
    let Some(pkg) = a.store.selected_package().cloned() else { return };
    let n = pkg.screenshots.len() as i32;
    if n == 0 {
        return;
    }
    let next = (a.store.shot_index + step).clamp(0, n - 1);
    if next == a.store.shot_index {
        return;
    }
    a.store.shot_index = next;
    a.store.shot_tried = false;
    canvas_free(&mut a.store.shot);
    a.store.asset_job = None;
    a.store.drop_waits(&|stage| stage == FetchStage::Screenshot);
    start_selected_assets(a);
}

fn tapped(a: &mut App, h: Hit) {
    match h.id {
        H_CLOSE => close(a),
        H_BACK => {
            match a.store.view {
                StoreView::List => close(a),
                StoreView::Agree => {
                    a.store.view = StoreView::Trust;
                    a.store.install_job = None;
                    a.store.drop_waits(&|stage| stage == FetchStage::License);
                    redraw(a);
                }
                StoreView::Trust | StoreView::Error if a.store.repo_install => {
                    a.store.view = StoreView::List;
                    a.store.error.clear();
                    a.store.install_job = None;
                    a.store.repo_install = false;
                    a.store.repo_input.clear();
                    a.store.repo_pkg = None;
                    a.store.selected = None;
                    redraw(a);
                }
                StoreView::Trust | StoreView::Error | StoreView::Qr | StoreView::Remove | StoreView::License => {
                    a.store.view = StoreView::Detail;
                    a.store.error.clear();
                    a.store.install_job = None;
                    redraw(a);
                }
                _ => {
                    a.store.view = StoreView::List;
                    a.store.error.clear();
                    a.store.install_job = None;
                    a.store.asset_job = None;
                    redraw(a);
                }
            }
        }
        H_REFRESH => {
            a.store.error.clear();
            import_development(a);
            start_catalog(&mut a.store);
            redraw(a);
        }
        H_FILTER => {
            a.store.filter = clampi(h.arg, 0, FILTER_INSTALLED);
            a.store.page = 0;
            redraw(a);
        }
        H_CARD => open_detail(a, h.arg),
        H_SEARCH => {
            a.store.kb_for = KbFor::Search;
            keyboard_open(&mut a.store.kb, tr("Search packages"), a.store.query.as_bytes());
            redraw(a);
        }
        H_REPO => {
            a.store.view = StoreView::InstallRepo;
            a.store.kb_for = KbFor::Repo;
            a.store.error.clear();
            keyboard_open(&mut a.store.kb, tr("GitHub repository"), b"");
            redraw(a);
        }
        H_QUERY_CLEAR => {
            a.store.query.clear();
            a.store.page = 0;
            redraw(a);
        }
        H_DESC_MORE => {
            a.store.desc_more = !a.store.desc_more;
            a.store.desc_page = 0;
            redraw(a);
        }
        H_DESC_PREV => {
            a.store.desc_page = (a.store.desc_page - 1).max(0);
            redraw(a);
        }
        H_DESC_NEXT => {
            a.store.desc_page += 1;
            redraw(a);
        }
        H_LICENSE => open_license(a),
        H_LICENSE_PREV => {
            a.store.license_page = (a.store.license_page - 1).max(0);
            redraw(a);
        }
        H_LICENSE_NEXT => {
            a.store.license_page += 1;
            redraw(a);
        }
        H_UNINSTALL => {
            a.store.view = StoreView::Remove;
            redraw(a);
        }
        H_UNINSTALL_YES => uninstall_selected(a),
        H_PREV => {
            a.store.page = (a.store.page - 1).max(0);
            redraw(a);
        }
        H_NEXT => {
            a.store.page += 1;
            redraw(a);
        }
        H_SHOT_PREV => {
            shot_move(a, -1);
            redraw(a);
        }
        H_SHOT_NEXT => {
            shot_move(a, 1);
            redraw(a);
        }
        H_INSTALL => begin_install(a),
        H_TRUST => after_trust(a),
        H_AGREE => {
            a.store.install_job = None;
            a.store.drop_waits(&|stage| stage == FetchStage::License);
            start_artifact(a);
        }
        H_QR => {
            a.store.view = StoreView::Qr;
            redraw(a);
        }
        H_RETRY => {
            a.store.repo_install = false;
            a.store.repo_input.clear();
            a.store.repo_pkg = None;
            if a.store.index.packages.is_empty() {
                start_catalog(&mut a.store);
            } else {
                a.store.view = StoreView::List;
                a.store.error.clear();
            }
            redraw(a);
        }
        _ => {}
    }
}

pub fn store_pointer(a: &mut App, ph: i32, ev: &InEvent) {
    if !a.store.open {
        return;
    }
    if a.store.kb.open {
        let mut ui = std::mem::take(&mut a.store);
        let r = app_keyboard_pointer(a, &mut ui.kb, ph, ev);
        let finished = r == KbResult::Done || r == KbResult::Cancel;
        if finished {
            keyboard_close(&mut ui.kb);
            let text = ui.kb.text.as_str().to_string();
            let to_repo = ui.kb_for == KbFor::Repo;
            a.store = ui;
            if r == KbResult::Done {
                if to_repo {
                    begin_repo_install(a, &text);
                } else {
                    a.store.query = text;
                    a.store.page = 0;
                    a.store.view = StoreView::List;
                    redraw(a);
                }
            } else {
                if a.store.view == StoreView::InstallRepo {
                    a.store.view = StoreView::List;
                }
                redraw(a);
            }
        } else {
            a.store = ui;
        }
        return;
    }
    if ph == 0 {
        press_begin(&mut a.store.press, ev.x, ev.y, ev.t_ms);
        a.store.pressed = None;
        a.store.pressed_hit = None;
        if let Some(h) = hits_find(&a.store.hits, ev.x, ev.y).copied() {
            app_press_feedback(a, h.r);
            a.store.pressed = Some(h.r);
            a.store.pressed_hit = Some(h);
        }
        return;
    }
    press_move(&mut a.store.press, ev.x, ev.y);
    if ph != 2 {
        return;
    }
    let mut pressed = a.store.pressed.take();
    let back = app_press_release(a, &mut pressed);
    let cancelled = touch_cancelled(ev);
    let hit = a.store.pressed_hit.take().filter(|h| !cancelled && press_is_tap(&a.store.press, h.r));
    if let Some(h) = hit {
        tapped(a, h);
        return;
    }
    if !rect_is_empty(back) {
        app_invalidate(a, back, RefreshMode::Ui);
    }
}

// ---- layout and drawing ----

const PAGER_H: i32 = 110;

/// How many words of a package's description the page shows before it offers Read more.
const DESC_WORDS: usize = 60;

struct StorePage {
    content: Rect,
    back: Rect,
    close: Rect,
    refresh: Rect,
    search: Rect,
    repo: Rect,
}

fn store_page(c: &mut Canvas, aw: i32, ah: i32, title: &str, back: bool, tools: bool) -> StorePage {
    canvas_fill(c, Rect::new(0, 0, aw, ah), 255);
    let mut actions: Vec<(IconKind, i32)> = Vec::with_capacity(4);
    if tools {
        actions.push((IconKind::Update, 0));
        actions.push((IconKind::Search, 0));
        actions.push((IconKind::Download, 0));
    }
    actions.push((IconKind::Close, 0));
    let (head, rects) = gui_header(c, aw, title, back, &actions);
    let pad = gui_margin();
    let close = rects.last().map_or(RECT_EMPTY, |r| r.0);
    let tool = |i: usize| if tools { rects.get(i).map_or(RECT_EMPTY, |r| r.0) } else { RECT_EMPTY };
    StorePage {
        content: Rect::new(pad, head.foot, aw - 2 * pad, ah - head.foot - pad),
        back: head.back,
        close,
        refresh: tool(0),
        search: tool(1),
        repo: tool(2),
    }
}

fn draw_header_hits(ui: &mut PackageStoreUi, page: &StorePage) {
    if !rect_is_empty(page.back) {
        hits_add(&mut ui.hits, page.back, H_BACK, 0);
    }
    if !rect_is_empty(page.close) {
        hits_add(&mut ui.hits, page.close, H_CLOSE, 0);
    }
    if !rect_is_empty(page.refresh) {
        hits_add(&mut ui.hits, page.refresh, H_REFRESH, 0);
    }
    if !rect_is_empty(page.search) {
        hits_add(&mut ui.hits, page.search, H_SEARCH, 0);
    }
    if !rect_is_empty(page.repo) {
        hits_add(&mut ui.hits, page.repo, H_REPO, 0);
    }
}

fn draw_pager(c: &mut Canvas, ui: &mut PackageStoreUi, area: Rect, page: i32, pages: i32) {
    if pages <= 1 {
        return;
    }
    let bw = ui_px(110.0);
    let bh = ui_px(90.0);
    let y = area.y + area.h - bh;
    let prev = Rect::new(area.x, y, bw, bh);
    let next = Rect::new(area.x + area.w - bw, y, bw, bh);
    gui_icon_button(c, prev, IconKind::Prev, if page > 0 { ButtonStyle::Plain } else { ButtonStyle::Disabled });
    gui_icon_button(c, next, IconKind::Next, if page + 1 < pages { ButtonStyle::Plain } else { ButtonStyle::Disabled });
    if page > 0 {
        hits_add(&mut ui.hits, prev, H_PREV, 0);
    }
    if page + 1 < pages {
        hits_add(&mut ui.hits, next, H_NEXT, 0);
    }
    font_draw_centered(
        c,
        FontFace::Fixed,
        ui_small_size(),
        Rect::new(prev.x + prev.w, y, (next.x - prev.x - prev.w).max(0), bh),
        &format!("{}/{}", page + 1, pages),
        0,
    );
}

fn kind_word(p: &StorePackage) -> &'static str {
    match p.kind.as_str() {
        "app" => tr("App"),
        // TRANSLATORS: the Package Store's tag on a theme's card (a package that changes how Grimoire looks).
        "theme" => tr("Theme"),
        _ => tr("Widget"),
    }
}

fn draw_tag(c: &mut Canvas, r: Rect, text: &str, selected: bool) {
    let rad = look().corner(Corner::Tag(r.h));
    ui_frame(c, r, rad, if selected { 0 } else { 255 }, true, 0);
    ui_frame(c, r, rad, if selected { 0 } else { 150 }, false, ui_px(2.0));
    font_draw_centered(c, FontFace::Fixed, ui_small_size() * 0.82, r, text, if selected { 255 } else { 80 });
}

fn draw_package_card(c: &mut Canvas, r: Rect, p: &StorePackage, featured: bool, icon_image: Option<&Canvas>) {
    ui_frame(c, r, gui_radius(), 255, true, 0);
    ui_frame(c, r, gui_radius(), 0, false, ui_px(3.0));
    let pad = ui_px(24.0);
    let icon_box = Rect::new(r.x + pad, r.y + pad, ui_px(120.0), ui_px(120.0));
    ui_frame(c, icon_box, gui_radius(), 255, true, 0);
    ui_frame(c, icon_box, gui_radius(), 0, false, ui_px(2.0));
    if let Some(image) = icon_image {
        canvas_blit_fit(c, image, rect_inflate(icon_box, -ui_px(12.0)));
    } else {
        let glyph = match &p.icon {
            StoreIconRef::Glyph(g) => *g,
            StoreIconRef::Image(_) => IconKind::Package,
        };
        ui_icon(c, glyph, rect_inflate(icon_box, -ui_px(22.0)), 0);
    }
    let tx = icon_box.x + icon_box.w + ui_px(18.0);
    let tw = (r.x + r.w - pad - tx).max(0);
    let title_h = ui_px(58.0);
    ui_text(c, FontFace::Bold, ui_text_size(), ui_small_size(), Rect::new(tx, r.y + pad, tw, title_h), &p.name, 1, false, 0);
    let sub = if p.category.is_empty() {
        format!("{} · {}", kind_word(p), p.version)
    } else {
        format!("{} · {} · {}", kind_word(p), p.category, p.version)
    };
    font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(tx, r.y + pad + title_h, tw, ui_px(34.0)), &sub, 90);
    let body_y = icon_box.y + icon_box.h + ui_px(18.0);
    let body_h = (r.y + r.h - pad - body_y).max(ui_px(40.0));
    let summary = if p.summary.is_empty() { p.description.as_str() } else { p.summary.as_str() };
    let lh = (font_ascent(FontFace::Regular, ui_small_size()) + font_descent(FontFace::Regular, ui_small_size())).max(1);
    let lines = ((body_h - ui_px(52.0)) / lh).clamp(1, 3);
    font_draw_wrapped(c, FontFace::Regular, ui_small_size(), Rect::new(r.x + pad, body_y, r.w - 2 * pad, body_h - ui_px(50.0)), summary, lines, lh + ui_px(4.0), false, 0);
    let mut tag_x = r.x + pad;
    let tag_y = r.y + r.h - pad - ui_px(38.0);
    for tag in p.tags.iter().take(2) {
        let w = (font_measure(FontFace::Fixed, ui_small_size() * 0.82, tag) + ui_px(28.0)).min(ui_px(280.0));
        if tag_x + w > r.x + r.w - pad {
            break;
        }
        draw_tag(c, Rect::new(tag_x, tag_y, w, ui_px(38.0)), tag, false);
        tag_x += w + ui_px(10.0);
    }
    if featured {
        let label = tr("Featured");
        let w = font_measure(FontFace::Fixed, ui_small_size() * 0.82, label) + ui_px(28.0);
        draw_tag(c, Rect::new(r.x + r.w - pad - w, tag_y, w, ui_px(38.0)), label, true);
    }
}

fn draw_list(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Package Store"), false, true);
    draw_header_hits(ui, &page);
    let area = page.content;
    let gap = gui_gap();
    let mut top = area.y;
    if !ui.query.is_empty() {
        let qh = ui_px(54.0);
        let text = format!("{} {}", tr("Search:"), ui.query);
        font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(area.x, top, (area.w - ui_px(80.0)).max(0), qh), &text, 80);
        let xr = Rect::new(area.x + area.w - ui_px(64.0), top, ui_px(64.0), qh);
        gui_icon_button(c, xr, IconKind::Close, ButtonStyle::Plain);
        hits_add(&mut ui.hits, xr, H_QUERY_CLEAR, 0);
        top += qh + gap;
    }
    let chips_h = gui_chip_h();
    // TRANSLATORS: the Package Store's chip that lists the themes (Settings > Display > Style lists those installed).
    let labels = [tr("All"), tr("Apps"), tr("Widgets"), tr("Themes"), tr("Installed")];
    let n = labels.len() as i32;
    let mut widths = Vec::with_capacity(labels.len());
    for label in &labels {
        widths.push(gui_chip_w(label));
    }
    let total: i32 = widths.iter().sum::<i32>() + gap * (n - 1);
    let max_w = (area.w - ui_px(12.0)).max(ui_px(240.0));
    if total > max_w {
        let each = ((max_w - gap * (n - 1)) / n).max(ui_px(110.0));
        for w in &mut widths {
            *w = each;
        }
    }
    let mut x = area.x;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::new(x, top, widths[i], chips_h);
        gui_chip(c, r, label, ui.filter == i as i32);
        hits_add(&mut ui.hits, r, H_FILTER, i as i32);
        x += widths[i] + gap;
    }
    let grid_top = top + chips_h + gap;
    let bottom = area.y + area.h;
    let vis = filtered(ui);
    let pager = if vis.is_empty() { 0 } else { PAGER_H + gap };
    let grid_h = (bottom - grid_top - pager).max(ui_px(200.0));
    let cols = if area.w >= ui_px(2550.0) {
        4
    } else if area.w >= ui_px(1700.0) {
        3
    } else {
        2
    };
    let card_w = (area.w - gap * (cols - 1)) / cols;
    let rows = ((grid_h + gap) / (ui_px(330.0) + gap)).max(1);
    let card_h = ((grid_h - gap * (rows - 1)) / rows).clamp(ui_px(260.0), ui_px(430.0));
    let per = (cols * rows).max(1) as usize;
    let pages = ((vis.len() + per - 1) / per).max(1) as i32;
    ui.page = ui.page.clamp(0, pages - 1);
    let first = ui.page as usize * per;
    if vis.is_empty() {
        let msg = if ui.refreshing {
            tr("Reading the Store...").to_string()
        } else if ui.filter == FILTER_INSTALLED {
            tr("No packages are installed.").to_string()
        } else if !ui.query.is_empty() {
            tr("No packages match your search.").to_string()
        } else {
            tr("Nothing in the Store yet.").to_string()
        };
        font_draw_centered(c, FontFace::Bold, ui_title_size(), Rect::new(area.x, grid_top + ui_px(120.0), area.w, ui_px(90.0)), &msg, 0);
        let hint = if ui.error.is_empty() { tr("Tap Refresh to check again.") } else { message_shown(&ui.error) };
        ui_text(c, FontFace::Regular, ui_text_size(), ui_small_size(), Rect::new(area.x, grid_top + ui_px(230.0), area.w, ui_px(180.0)), hint, 2, true, 70);
        if !ui.error.is_empty() {
            let r = Rect::new(area.x + area.w / 2 - ui_px(170.0), grid_top + ui_px(430.0), ui_px(340.0), ui_px(100.0));
            gui_button(c, r, IconKind::Update, tr("Refresh"), ButtonStyle::Plain);
            hits_add(&mut ui.hits, r, H_RETRY, 0);
        }
    } else {
        let shown = vis.len().saturating_sub(first).min(per);
        for (k, item) in vis.iter().skip(first).take(shown).enumerate() {
            let row = (k / cols as usize) as i32;
            let in_row = (shown - row as usize * cols as usize).min(cols as usize) as i32;
            let row_w = in_row * card_w + (in_row - 1).max(0) * gap;
            let x0 = area.x + (area.w - row_w) / 2;
            let col = (k % cols as usize) as i32;
            let r = Rect::new(x0 + col * (card_w + gap), grid_top + row * (card_h + gap), card_w, card_h);
            let Some(p) = pkg_at(ui, *item) else { continue };
            let featured = p.id == ui.index.featured && ui.filter != 3;
            let icon_image = match item {
                PkgRef::Catalog(n) => ui.list_icons.get(*n).filter(|icon| !icon.px.is_empty()),
                PkgRef::Installed(_) => None,
            };
            draw_package_card(c, r, p, featured, icon_image);
            hits_add(&mut ui.hits, r, H_CARD, card_arg(*item));
        }
        draw_pager(c, ui, Rect::new(area.x, bottom - PAGER_H, area.w, PAGER_H), ui.page, pages);
    }
    if !ui.dev_note.is_empty() {
        font_draw_centered(c, FontFace::Regular, ui_small_size(), Rect::new(area.x, bottom - PAGER_H - ui_px(76.0), area.w, ui_px(34.0)), &ui.dev_note, 110);
    }
    if !ui.status.is_empty() {
        font_draw_centered(c, FontFace::Regular, ui_small_size(), Rect::new(area.x, bottom - PAGER_H - ui_px(38.0), area.w, ui_px(34.0)), message_shown(&ui.status), 100);
    }
}

/// Whether the detail page's screenshot is still to come: not asked for yet, downloading, or asked of the
/// worker's job.
fn shot_coming(ui: &PackageStoreUi) -> bool {
    !ui.shot_tried
        || ui.asset_job.as_ref().is_some_and(|job| job.stage == FetchStage::Screenshot)
        || ui.pic_waits.iter().any(|w| w.stage == FetchStage::Screenshot)
}

fn draw_shot(c: &mut Canvas, ui: &mut PackageStoreUi, r: Rect, p: &StorePackage) {
    ui_frame(c, r, gui_radius(), 255, true, 0);
    ui_frame(c, r, gui_radius(), 0, false, ui_px(3.0));
    let inner = rect_inflate(r, -ui_px(16.0));
    let n = p.screenshots.len() as i32;
    if n == 0 {
        ui_icon(c, IconKind::Image, rect_inflate(inner, -ui_px(120.0)), 150);
        font_draw_centered(c, FontFace::Regular, ui_text_size(), inner, tr("No screenshots yet."), 100);
        return;
    }
    let bar = ui_px(64.0);
    let image_r = Rect::new(inner.x, inner.y, inner.w, (inner.h - bar - ui_px(10.0)).max(ui_px(80.0)));
    if !ui.shot.px.is_empty() {
        canvas_blit_fit(c, &ui.shot, image_r);
    } else {
        // (the frame with its glyph; it says a screenshot is loading only while one is still to come: not once
        // its download failed, the worker refused it, or no worker could be had)
        ui_icon(c, IconKind::Image, rect_inflate(image_r, -ui_px(100.0)), 155);
        if shot_coming(ui) {
            font_draw_centered(c, FontFace::Regular, ui_text_size(), image_r, tr("Loading screenshot..."), 100);
        }
    }
    let y = image_r.y + image_r.h + ui_px(10.0);
    let label = format!("{} {}/{}", tr("Screenshot"), ui.shot_index + 1, n);
    font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(inner.x, y, (inner.w - ui_px(230.0)).max(0), bar), &label, 90);
    let bw = ui_px(100.0);
    let prev = Rect::new(inner.x + inner.w - 2 * bw - ui_px(12.0), y, bw, bar);
    let next = Rect::new(inner.x + inner.w - bw, y, bw, bar);
    gui_icon_button(c, prev, IconKind::Prev, if ui.shot_index > 0 { ButtonStyle::Plain } else { ButtonStyle::Disabled });
    gui_icon_button(c, next, IconKind::Next, if ui.shot_index + 1 < n { ButtonStyle::Plain } else { ButtonStyle::Disabled });
    if ui.shot_index > 0 {
        hits_add(&mut ui.hits, prev, H_SHOT_PREV, 0);
    }
    if ui.shot_index + 1 < n {
        hits_add(&mut ui.hits, next, H_SHOT_NEXT, 0);
    }
}

fn draw_meta(c: &mut Canvas, r: Rect, p: &StorePackage) {
    let mut y = r.y;
    let lh = ui_px(38.0);
    let mut row = |text: &str, gray: u8| {
        if y + lh <= r.y + r.h {
            font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(r.x, y, r.w, lh), text, gray);
            y += lh;
        }
    };
    if !p.author.is_empty() {
        row(&format!("{} {}", tr("By"), p.author), 0);
    }
    if !p.category.is_empty() {
        row(&format!("{} {}", tr("Category:"), p.category), 70);
    }
    if !p.license.is_empty() {
        row(&format!("{} {}", tr("License:"), p.license), 70);
    }
    if !p.repository.is_empty() {
        row(&format!("{} {}", tr("GitHub:"), p.repository), 70);
    }
    if !p.homepage.is_empty() {
        row(&format!("{} {}", tr("Homepage:"), p.homepage), 70);
    }
    if !p.tags.is_empty() {
        row(&format!("{} {}", tr("Tags:"), p.tags.join(" · ")), 70);
    }
    if !p.added.is_empty() {
        row(&format!("{} {}", tr("Added:"), p.added), 70);
    }
}

fn draw_description(c: &mut Canvas, r: Rect, p: &StorePackage) {
    let px = ui_text_size();
    let lh = (font_ascent(FontFace::Regular, px) + font_descent(FontFace::Regular, px)).max(1) + ui_px(4.0);
    let lines = (r.h / lh).max(1);
    font_draw_wrapped(c, FontFace::Regular, px, r, &p.description, lines, lh, false, 0);
}

fn draw_detail(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let title = tr("Package");
    let page = store_page(c, aw, ah, title, true, false);
    draw_header_hits(ui, &page);
    let Some(p) = ui.selected_package().cloned() else {
        ui.view = StoreView::List;
        return;
    };
    let area = page.content;
    // Reading the whole description: the page is the text, a page at a time, and the specs, the
    // buttons and the screenshot are out of its way.
    if ui.desc_more {
        draw_description_page(c, ui, area, &p);
        return;
    }
    let gap = gui_gap();
    let wide = area.w > area.h;
    let icon_sz = if wide { ui_px(150.0) } else { ui_px(140.0) };
    let header_h = icon_sz.max(ui_px(150.0));
    // Install (or Update), Uninstall and the repository link stand side by side, each with its
    // icon and its name beside it. They wrap onto another row only when the width cannot hold them.
    let installed_now = installed_kind(&p.id).is_some();
    let has_update = installed_for(&p).is_some_and(|it| update_version_cmp(&p.version, &it.version) > 0);
    let mut actions: Vec<(String, IconKind, i32, ButtonStyle)> = Vec::new();
    if installed_now {
        if has_update {
            actions.push((format!("{} {}", tr("Update to"), p.version), IconKind::Download, H_INSTALL, ButtonStyle::Primary));
        }
        actions.push((tr("Uninstall").to_string(), IconKind::Close, H_UNINSTALL, ButtonStyle::Primary));
    } else {
        actions.push((tr("Install").to_string(), IconKind::Download, H_INSTALL, ButtonStyle::Primary));
    }
    if !p.repository.is_empty() {
        actions.push((tr("Repository URL").to_string(), IconKind::Link, H_QR, ButtonStyle::Plain));
    }
    if !installed_license(&p).is_empty() || !p.license.is_empty() {
        actions.push((tr("License").to_string(), IconKind::Paper, H_LICENSE, ButtonStyle::Plain));
    }
    let action_h = gui_button_h();
    let action_gap = ui_px(12.0);
    let action_w = if wide { area.w * 43 / 100 } else { area.w };
    let mut widths: Vec<i32> = actions
        .iter()
        .map(|(label, _, _, _)| (font_measure(FontFace::Bold, ui_text_size(), label) + ui_px(150.0)).min(action_w))
        .collect();
    let mut action_rows: Vec<Vec<usize>> = Vec::new();
    let mut row: Vec<usize> = Vec::new();
    let mut row_w = 0;
    for (i, w) in widths.iter().enumerate() {
        if !row.is_empty() && row_w + action_gap + w > action_w {
            action_rows.push(std::mem::take(&mut row));
            row_w = 0;
        }
        row_w += if row.is_empty() { *w } else { *w + action_gap };
        row.push(i);
    }
    if !row.is_empty() {
        action_rows.push(row);
    }
    // Every row fills the width it has: the buttons share what their labels leave, so the row ends
    // flush with the page's right edge instead of stopping short of it.
    for row in &action_rows {
        let n = row.len() as i32;
        if n == 0 {
            continue;
        }
        let used: i32 = row.iter().map(|&i| widths[i]).sum::<i32>() + action_gap * (n - 1);
        let extra = (action_w - used).max(0);
        for (k, &i) in row.iter().enumerate() {
            widths[i] += if (k as i32) + 1 == n { extra - (extra / n) * (n - 1) } else { extra / n };
        }
    }
    let rows_n = action_rows.len().max(1) as i32;
    let actions_h = rows_n * action_h + (rows_n - 1) * action_gap;
    let meta_h = ui_px(228.0);
    let status_h = if ui.status.is_empty() { 0 } else { ui_px(50.0) };
    let (info, shot) = if wide {
        let info = Rect::new(area.x, area.y, area.w * 43 / 100, area.h);
        let shot = Rect::new(area.x + info.w + gap * 2, area.y, area.w - info.w - gap * 2, area.h);
        (info, shot)
    } else {
        let fixed = header_h + ui_px(18.0) + ui_px(460.0) + ui_px(8.0) + meta_h + status_h + ui_px(16.0) + actions_h + ui_px(14.0);
        let max_info = (area.h - gap - ui_px(520.0)).max(ui_px(760.0));
        let info_h = fixed.min(max_info).max(ui_px(760.0));
        let info = Rect::new(area.x, area.y, area.w, info_h);
        let shot = Rect::new(area.x, info.y + info.h + gap, area.w, (area.y + area.h - info.y - info.h - gap).max(ui_px(240.0)));
        (info, shot)
    };
    let icon_r = Rect::new(info.x, info.y, icon_sz, icon_sz);
    ui_frame(c, icon_r, gui_radius(), 255, true, 0);
    ui_frame(c, icon_r, gui_radius(), 0, false, ui_px(3.0));
    if ui.icon_for == p.id {
        ui.icon.draw(c, rect_inflate(icon_r, -ui_px(20.0)), 0);
    } else {
        let glyph = match &p.icon {
            StoreIconRef::Glyph(g) => *g,
            StoreIconRef::Image(_) => IconKind::Package,
        };
        ui_icon(c, glyph, rect_inflate(icon_r, -ui_px(28.0)), 0);
    }
    let tx = icon_r.x + icon_r.w + ui_px(22.0);
    let tw = (info.x + info.w - tx).max(0);
    ui_text(c, FontFace::Bold, ui_title_size() * 0.82, ui_text_size(), Rect::new(tx, info.y, tw, ui_px(76.0)), &p.name, 2, false, 0);
    let sub = format!("{} · {} · {}", kind_word(&p), p.version, p.category);
    font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(tx, info.y + ui_px(76.0), tw, ui_px(36.0)), &sub, 90);
    let y = info.y + header_h + ui_px(18.0);
    // The description stops short of the spec rows and of the line above the actions, so a long one
    // pushes neither off the page: past DESC_WORDS words it is cut and offers Read more.
    let ay = info.y + info.h - actions_h;
    let below = (ay - status_h - ui_px(8.0)).max(y);
    let desc_h = (below - meta_h - ui_px(8.0) - y).clamp(0, ui_px(760.0));
    let words: Vec<&str> = p.description.split_whitespace().collect();
    let cut = words.len() > DESC_WORDS;
    let btn_h = if cut && desc_h >= ui_px(120.0) { ui_px(46.0) } else { 0 };
    let body = if cut && !ui.desc_more {
        let mut t = words[..DESC_WORDS].join(" ");
        t.push_str("...");
        t
    } else {
        p.description.clone()
    };
    let text_h = (desc_h - btn_h - if btn_h > 0 { ui_px(6.0) } else { 0 }).max(0);
    draw_description(c, Rect::new(info.x, y, info.w, text_h), &StorePackage { description: body, ..p.clone() });
    if btn_h > 0 {
        let label = if ui.desc_more { tr("Show less") } else { tr("Read more") };
        let w = (font_measure(FontFace::Bold, ui_small_size(), label) + ui_px(56.0)).max(ui_px(170.0)).min(info.w);
        let b = Rect::new(info.x, y + desc_h - btn_h, w, btn_h);
        ui_frame(c, b, btn_h / 2, 255, true, 0);
        ui_frame(c, b, btn_h / 2, 0, false, ui_px(3.0));
        ui_text(c, FontFace::Bold, ui_small_size(), ui_small_size(), b, label, 1, true, 0);
        hits_add(&mut ui.hits, b, H_DESC_MORE, 0);
    }
    let meta_y = y + desc_h + ui_px(8.0);
    draw_meta(c, Rect::new(info.x, meta_y, info.w, (below - meta_y).clamp(0, meta_h)), &p);
    if !ui.status.is_empty() {
        font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(info.x, ay - status_h, info.w, ui_px(44.0)), message_shown(&ui.status), 90);
    }
    let mut row_y = ay;
    for row in &action_rows {
        let mut bx = info.x;
        for &i in row {
            let (label, icon, id, style) = &actions[i];
            let r = Rect::new(bx, row_y, widths[i], action_h);
            gui_button_row(c, r, *icon, label, *style);
            hits_add(&mut ui.hits, r, *id, 0);
            bx += widths[i] + action_gap;
        }
        row_y += action_h + action_gap;
    }
    draw_shot(c, ui, shot, &p);
}

/// The whole description, a page at a time: the package page's other parts are not drawn, so a long
/// text is never cut off, and Show less goes back to the package page.
fn draw_description_page(c: &mut Canvas, ui: &mut PackageStoreUi, area: Rect, p: &StorePackage) {
    let px = ui_text_size();
    let line_h = (font_ascent(FontFace::Regular, px) + font_descent(FontFace::Regular, px)).max(1) + ui_px(6.0);
    let bar_h = ui_px(76.0);
    let row1 = Rect::new(area.x, area.y + area.h - 2 * bar_h - ui_px(10.0), area.w, bar_h);
    let row2 = Rect::new(area.x, area.y + area.h - bar_h, area.w, bar_h);
    let body = Rect::new(area.x, area.y, area.w, (row1.y - ui_px(14.0) - area.y).max(line_h));
    let lines = crate::widgetpkgui::license_lines(&p.description, px, body.w);
    let per = (body.h / line_h).max(1);
    let pages = (((lines.len() as i32) + per - 1) / per).max(1);
    let page = ui.desc_page.clamp(0, pages - 1);
    ui.desc_page = page;
    let from = page as usize * per as usize;
    let to = (from + per as usize).min(lines.len());
    for (i, line) in lines[from..to].iter().enumerate() {
        font_draw_left(c, FontFace::Regular, px, Rect::new(body.x, body.y + i as i32 * line_h, body.w, line_h), line, 0);
    }
    if pages > 1 {
        let bw = ui_px(110.0);
        let prev = Rect::new(row1.x, row1.y, bw, row1.h);
        let next = Rect::new(row1.x + row1.w - bw, row1.y, bw, row1.h);
        gui_icon_button(c, prev, IconKind::Prev, if page > 0 { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        gui_icon_button(c, next, IconKind::Next, if page + 1 < pages { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        if page > 0 {
            hits_add(&mut ui.hits, prev, H_DESC_PREV, 0);
        }
        if page + 1 < pages {
            hits_add(&mut ui.hits, next, H_DESC_NEXT, 0);
        }
        font_draw_centered(
            c,
            FontFace::Fixed,
            ui_small_size(),
            Rect::new(prev.x + prev.w, row1.y, (next.x - prev.x - prev.w).max(0), row1.h),
            &format!("{}/{}", page + 1, pages),
            0,
        );
    }
    gui_button_row(c, row2, IconKind::Back, tr("Show less"), ButtonStyle::Plain);
    hits_add(&mut ui.hits, row2, H_DESC_MORE, 0);
}

fn draw_trust(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Trust this package?"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    let Some(p) = ui.selected_package().cloned() else { ui.view = StoreView::List; return };
    let m = ui.manifest.clone().unwrap_or_default();
    let icon_sz = ui_px(160.0);
    let icon = Rect::new(area.x, area.y, icon_sz, icon_sz);
    ui_frame(c, icon, gui_radius(), 255, true, 0);
    ui_frame(c, icon, gui_radius(), 0, false, ui_px(3.0));
    let glyph = match &p.icon {
        StoreIconRef::Glyph(g) => *g,
        StoreIconRef::Image(_) => IconKind::Package,
    };
    ui_icon(c, glyph, rect_inflate(icon, -ui_px(30.0)), 0);
    let tx = icon.x + icon.w + ui_px(22.0);
    let tw = (area.x + area.w - tx).max(0);
    font_draw_left(c, FontFace::Bold, ui_title_size() * 0.8, Rect::new(tx, area.y, tw, ui_px(76.0)), &p.name, 0);
    let who = if m.author.is_empty() { p.author.as_str() } else { m.author.as_str() };
    let ver = if m.version.is_empty() { p.version.as_str() } else { m.version.as_str() };
    let sub = format!("{} {} · {}", tr("by"), who, ver);
    font_draw_left(c, FontFace::Regular, ui_small_size(), Rect::new(tx, area.y + ui_px(78.0), tw, ui_px(38.0)), &sub, 90);
    let mut warn = String::new();
    csnprintf!(
        &mut warn,
        1400,
        // TRANSLATORS: the Package Store's trust page. %s is the package kind (app or widget).
        tr("You MUST trust the author of this package. Grimoire does not review, vet, or verify %s code and is not responsible for any damage, data loss, or malfunction caused by installing or running it."),
        match p.kind.as_str() {
            "app" => "app",
            "theme" => "theme",
            _ => "widget",
        }
    );
    let top = area.y + icon_sz + ui_px(34.0);
    let button = Rect::new(area.x, area.y + area.h - ui_px(110.0), area.w, ui_px(100.0));
    let cancel = Rect::new(area.x, button.y - ui_px(72.0), area.w, ui_px(58.0));
    // An update whose license differs from the one the package was installed under says so here: the
    // accept asks for the new terms, and the License page holds the text to read first.
    let was = installed_license(&p);
    let now = if m.license.is_empty() { p.license.clone() } else { m.license.clone() };
    let mut notice = String::new();
    if !was.is_empty() && !now.is_empty() && !was.eq_ignore_ascii_case(&now) {
        csnprintf!(
            &mut notice,
            500,
            tr("This update changes the license from %s to %s. Read it before you accept."),
            &was,
            &now
        );
    }
    let notice_h = if notice.is_empty() { 0 } else { ui_px(110.0) };
    let notice_r = Rect::new(area.x, cancel.y - ui_px(14.0) - notice_h, area.w, notice_h);
    // Laid out from the buttons up, so the description takes what is left and never their room.
    let desc_h = (notice_r.y - ui_px(14.0) - top).max(0);
    draw_description(c, Rect::new(area.x, top, area.w, desc_h), &StorePackage { description: warn, ..p.clone() });
    if !notice.is_empty() {
        ui_text(c, FontFace::Regular, ui_small_size(), ui_small_size(), notice_r, &notice, 3, true, 0);
    }
    gui_button_row(c, button, IconKind::Download, tr("I trust the author, continue"), ButtonStyle::Primary);
    hits_add(&mut ui.hits, button, H_TRUST, 0);
    ui_text(c, FontFace::Regular, ui_small_size(), ui_small_size(), cancel, tr("Back to the package"), 1, true, 120);
    hits_add(&mut ui.hits, cancel, H_BACK, 0);
}

/// Reading and accepting the license a package is installed under. The page is the text, and the
/// install only starts with the accept at its bottom: an update whose license differs passes here
/// again, so new terms are never installed under an old acceptance.
fn draw_agree(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Accept the license"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    let Some(p) = ui.selected_package().cloned() else {
        ui.view = StoreView::List;
        return;
    };
    let mut name = ui.manifest.as_ref().map_or(String::new(), |m| m.license.trim().to_string());
    if name.is_empty() {
        name = p.license.trim().to_string();
    }
    let head = if name.is_empty() { tr("License") } else { name.as_str() };
    font_draw_left(c, FontFace::Bold, ui_title_size() * 0.8, Rect::new(area.x, area.y, area.w, ui_px(76.0)), head, 0);
    let was = installed_license(&p);
    let mut note = String::new();
    if !was.is_empty() && !was.eq_ignore_ascii_case(&name) {
        csnprintf!(
            &mut note,
            400,
            tr("The update changes the license from %s to %s. Read it and accept it to update %s."),
            &was,
            &name,
            &p.name
        );
    } else {
        csnprintf!(&mut note, 400, tr("Read the license and accept it to install %s."), &p.name);
    }
    let note_r = Rect::new(area.x, area.y + ui_px(80.0), area.w, ui_px(80.0));
    ui_text(c, FontFace::Regular, ui_small_size(), ui_small_size(), note_r, &note, 2, true, 80);
    let px = ui_text_size();
    let line_h = (font_ascent(FontFace::Regular, px) + font_descent(FontFace::Regular, px)).max(1) + ui_px(2.0);
    let accept = Rect::new(area.x, area.y + area.h - ui_px(110.0), area.w, ui_px(100.0));
    let cancel = Rect::new(area.x, accept.y - ui_px(72.0), area.w, ui_px(58.0));
    let pager = Rect::new(area.x, cancel.y - ui_px(14.0) - PAGER_H, area.w, PAGER_H);
    let top = note_r.y + note_r.h + ui_px(12.0);
    let body_h = (pager.y - ui_px(10.0) - top).max(line_h);
    let text = if ui.license_text.is_empty() { ui.license_note.as_str() } else { ui.license_text.as_str() };
    let lines = crate::widgetpkgui::license_lines(text, px, area.w);
    let per = (body_h / line_h).max(1);
    let pages = (((lines.len() as i32) + per - 1) / per).max(1);
    let pg = ui.license_page.clamp(0, pages - 1);
    ui.license_page = pg;
    let from = pg as usize * per as usize;
    let to = (from + per as usize).min(lines.len());
    let gray = if ui.license_text.is_empty() { 80 } else { 0 };
    for (i, line) in lines[from..to].iter().enumerate() {
        font_draw_left(c, FontFace::Regular, px, Rect::new(area.x, top + i as i32 * line_h, area.w, line_h), line, gray);
    }
    if pages > 1 {
        let bw = ui_px(110.0);
        let prev = Rect::new(pager.x, pager.y, bw, pager.h);
        let next = Rect::new(pager.x + pager.w - bw, pager.y, bw, pager.h);
        gui_icon_button(c, prev, IconKind::Prev, if pg > 0 { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        gui_icon_button(c, next, IconKind::Next, if pg + 1 < pages { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        if pg > 0 {
            hits_add(&mut ui.hits, prev, H_LICENSE_PREV, 0);
        }
        if pg + 1 < pages {
            hits_add(&mut ui.hits, next, H_LICENSE_NEXT, 0);
        }
        font_draw_centered(
            c,
            FontFace::Fixed,
            ui_small_size(),
            Rect::new(prev.x + prev.w, prev.y, (next.x - prev.x - prev.w).max(0), prev.h),
            &format!("{}/{}", pg + 1, pages),
            0,
        );
    }
    gui_button_row(c, accept, IconKind::Check, tr("I accept the license"), ButtonStyle::Primary);
    hits_add(&mut ui.hits, accept, H_AGREE, 0);
    ui_text(c, FontFace::Regular, ui_small_size(), ui_small_size(), cancel, tr("Don't install"), 1, true, 120);
    hits_add(&mut ui.hits, cancel, H_BACK, 0);
}

fn draw_license(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("License"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    let Some(p) = ui.selected_package().cloned() else {
        ui.view = StoreView::Detail;
        return;
    };
    let was = installed_license(&p);
    // (the page is only opened from the package page: its entry is the installed one there, or the
    // catalog's newer one, whose license is what an update would install)
    let offered = p.license.clone();
    let head = if !was.is_empty() {
        was.as_str()
    } else if !offered.is_empty() {
        offered.as_str()
    } else {
        tr("License")
    };
    font_draw_left(c, FontFace::Bold, ui_title_size() * 0.8, Rect::new(area.x, area.y, area.w, ui_px(76.0)), head, 0);
    let mut note = String::new();
    if !was.is_empty() && !offered.is_empty() && !was.eq_ignore_ascii_case(&offered) {
        csnprintf!(&mut note, 500, tr("This update changes the license from %s to %s."), &was, &offered);
    } else if !was.is_empty() {
        csnprintf!(&mut note, 500, tr("The license %s is installed under."), &p.name);
    } else {
        csnprintf!(&mut note, 500, tr("The license %s is published under."), &p.name);
    }
    let note_r = Rect::new(area.x, area.y + ui_px(80.0), area.w, ui_px(80.0));
    ui_text(c, FontFace::Regular, ui_small_size(), ui_small_size(), note_r, &note, 2, true, 80);
    let px = ui_text_size();
    let line_h = (font_ascent(FontFace::Regular, px) + font_descent(FontFace::Regular, px)).max(1) + ui_px(2.0);
    let top = note_r.y + note_r.h + ui_px(12.0);
    let bottom = area.y + area.h;
    let body_h = (bottom - PAGER_H - top).max(line_h);
    let text = if ui.license_text.is_empty() { ui.license_note.as_str() } else { ui.license_text.as_str() };
    let lines = crate::widgetpkgui::license_lines(text, px, area.w);
    let per = (body_h / line_h).max(1);
    let pages = (((lines.len() as i32) + per - 1) / per).max(1);
    let pg = ui.license_page.clamp(0, pages - 1);
    ui.license_page = pg;
    let from = pg as usize * per as usize;
    let to = (from + per as usize).min(lines.len());
    let gray = if ui.license_text.is_empty() { 80 } else { 0 };
    for (i, line) in lines[from..to].iter().enumerate() {
        font_draw_left(c, FontFace::Regular, px, Rect::new(area.x, top + i as i32 * line_h, area.w, line_h), line, gray);
    }
    if pages > 1 {
        let y = bottom - PAGER_H;
        let bw = ui_px(110.0);
        let prev = Rect::new(area.x, y, bw, PAGER_H);
        let next = Rect::new(area.x + area.w - bw, y, bw, PAGER_H);
        gui_icon_button(c, prev, IconKind::Prev, if pg > 0 { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        gui_icon_button(c, next, IconKind::Next, if pg + 1 < pages { ButtonStyle::Plain } else { ButtonStyle::Disabled });
        if pg > 0 {
            hits_add(&mut ui.hits, prev, H_LICENSE_PREV, 0);
        }
        if pg + 1 < pages {
            hits_add(&mut ui.hits, next, H_LICENSE_NEXT, 0);
        }
        font_draw_centered(
            c,
            FontFace::Fixed,
            ui_small_size(),
            Rect::new(prev.x + prev.w, y, (next.x - prev.x - prev.w).max(0), PAGER_H),
            &format!("{}/{}", pg + 1, pages),
            0,
        );
    }
}

fn draw_remove(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Uninstall this package?"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    let Some(p) = ui.selected_package().cloned() else {
        ui.view = StoreView::List;
        return;
    };
    let mut text = String::new();
    csnprintf!(
        &mut text,
        1200,
        tr("Uninstall %s? Its widget or app is closed and its files are deleted. Your layout is kept."),
        &p.name
    );
    draw_description(c, Rect::new(area.x, area.y, area.w, ui_px(320.0)), &StorePackage { description: text, ..p.clone() });
    let bh = ui_px(100.0);
    let gap = ui_px(14.0);
    let bw = (area.w - gap) / 2;
    let yes = Rect::new(area.x, area.y + area.h - bh, bw, bh);
    gui_button_row(c, yes, IconKind::Close, tr("Uninstall"), ButtonStyle::Primary);
    hits_add(&mut ui.hits, yes, H_UNINSTALL_YES, 0);
    let keep = Rect::new(area.x + bw + gap, yes.y, bw, bh);
    gui_button_row(c, keep, IconKind::Back, tr("Keep it"), ButtonStyle::Plain);
    hits_add(&mut ui.hits, keep, H_BACK, 0);
}

fn draw_install_repo(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Install from GitHub"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    ui_text(
        c,
        FontFace::Regular,
        ui_text_size(),
        ui_small_size(),
        Rect::new(area.x, area.y, area.w, ui_px(220.0)),
        tr("Type a GitHub repository as user/repo, or an https address ending in .yaml, on the keyboard below."),
        4,
        true,
        0,
    );
    ui_text(
        c,
        FontFace::Regular,
        ui_small_size(),
        ui_small_size(),
        Rect::new(area.x, area.y + ui_px(240.0), area.w, ui_px(120.0)),
        tr("Tap Done to download it. Nothing is written until you trust the author."),
        3,
        true,
        90,
    );
}

fn draw_qr(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Repository"), true, false);
    draw_header_hits(ui, &page);
    let Some(p) = ui.selected_package() else { ui.view = StoreView::Detail; return };
    let area = page.content;
    let side = (area.w.min(area.h) - ui_px(260.0)).max(ui_px(260.0)).min(ui_px(900.0));
    let x = area.x + (area.w - side) / 2;
    let qr = Rect::new(x, area.y + ui_px(50.0), side, side);
    let sym = crate::qr::ui_qr(c, qr, &p.repository, false);
    let ty = if rect_is_empty(sym) { qr.y } else { sym.y + sym.h + ui_px(24.0) };
    font_draw_centered(c, FontFace::Regular, ui_small_size(), Rect::new(area.x, ty, area.w, ui_px(36.0)), tr("Scan with a phone's camera"), 90);
    ui_text(c, FontFace::Regular, ui_text_size(), ui_small_size(), Rect::new(x, ty + ui_px(52.0), side, ui_px(100.0)), &p.repository, 2, true, 0);
}

fn draw_error(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Package Store"), true, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    let title = if ui.index.packages.is_empty() { tr("Couldn't open the Store") } else { tr("Something went wrong") };
    font_draw_centered(c, FontFace::Bold, ui_title_size(), Rect::new(area.x, area.y + ui_px(80.0), area.w, ui_px(90.0)), title, 0);
    ui_text(c, FontFace::Regular, ui_text_size(), ui_small_size(), Rect::new(area.x, area.y + ui_px(200.0), area.w, ui_px(220.0)), message_shown(&ui.error), 4, true, 80);
    let r = Rect::new(area.x + area.w / 2 - ui_px(180.0), area.y + ui_px(480.0), ui_px(360.0), ui_px(100.0));
    gui_button(c, r, IconKind::Update, if ui.index.packages.is_empty() { tr("Try again") } else { tr("Back to the Store") }, ButtonStyle::Primary);
    hits_add(&mut ui.hits, r, H_RETRY, 0);
}

fn draw_busy(c: &mut Canvas, ui: &mut PackageStoreUi, aw: i32, ah: i32) {
    let page = store_page(c, aw, ah, tr("Package Store"), false, false);
    draw_header_hits(ui, &page);
    let area = page.content;
    ui_icon(c, IconKind::Hourglass, Rect::new(area.x + (area.w - ui_px(180.0)) / 2, area.y + ui_px(160.0), ui_px(180.0), ui_px(180.0)), 80);
    ui_text(c, FontFace::Bold, ui_title_size(), ui_text_size(), Rect::new(area.x, area.y + ui_px(400.0), area.w, ui_px(90.0)), message_shown(&ui.status), 2, true, 0);
}

pub fn store_draw(a: &mut App) {
    if !a.store.open {
        return;
    }
    let App { screen: c, store: ui, w, h, .. } = a;
    ui.hits.h.clear();
    match ui.view {
        StoreView::List => draw_list(c, ui, *w, *h),
        StoreView::Detail => draw_detail(c, ui, *w, *h),
        StoreView::Trust => draw_trust(c, ui, *w, *h),
        StoreView::Qr => draw_qr(c, ui, *w, *h),
        StoreView::Error => draw_error(c, ui, *w, *h),
        StoreView::Busy => draw_busy(c, ui, *w, *h),
        StoreView::InstallRepo => draw_install_repo(c, ui, *w, *h),
        StoreView::Remove => draw_remove(c, ui, *w, *h),
        StoreView::License => draw_license(c, ui, *w, *h),
        StoreView::Agree => draw_agree(c, ui, *w, *h),
    }
    if ui.kb.open {
        let screen = Rect::new(0, 0, *w, *h);
        keyboard_draw(&mut ui.kb, c, screen);
    }
}

// ---- probes and the scenario selector ----

pub fn store_probe(ui: &PackageStoreUi, key: &str) -> Option<String> {
    let view = match ui.view {
        StoreView::List => "list",
        StoreView::Detail => "detail",
        StoreView::Trust => "trust",
        StoreView::Busy => "busy",
        StoreView::Error => "error",
        StoreView::Qr => "qr",
        StoreView::InstallRepo => "install-repo",
        StoreView::Remove => "remove",
        StoreView::License => "license",
        StoreView::Agree => "agree",
    };
    Some(match key {
        "view" => view.to_string(),
        "open" => (ui.open as i32).to_string(),
        "filter" => ui.filter.to_string(),
        "query" => ui.query.clone(),
        "installed" => ui.installed.len().to_string(),
        "featured" => ui.index.featured.clone(),
        "selected" => ui.selected.clone().unwrap_or_default(),
        "packages" => ui.index.packages.len().to_string(),
        "visible" => filtered(ui)
            .iter()
            .filter_map(|r| pkg_at(ui, *r))
            .map(|p| p.id.clone())
            .collect::<Vec<_>>()
            .join(" "),
        "screenshots" => ui
            .selected_package()
            .map_or(0, |p| p.screenshots.len())
            .to_string(),
        "shot" => ui.shot_index.to_string(),
        "shot.loaded" => (!ui.shot.px.is_empty() as i32).to_string(),
        "shot.coming" => ((ui.shot.px.is_empty() && shot_coming(ui)) as i32).to_string(),
        "icon.loaded" => (!ui.icon.image.px.is_empty() as i32).to_string(),
        "list.icons" => ui.list_icons.iter().filter(|icon| !icon.px.is_empty()).count().to_string(),
        "status" => ui.status.clone(),
        "dev.note" => ui.dev_note.clone(),
        "desc.more" => (ui.desc_more as i32).to_string(),
        "desc.page" => ui.desc_page.to_string(),
        "license" => ui.license_text.chars().take(120).collect::<String>(),
        "license.note" => ui.license_note.clone(),
        "license.page" => ui.license_page.to_string(),
        "error" => ui.error.clone(),
        "page" => ui.page.to_string(),
        _ => return None,
    })
}

/// Named controls for the scenario runner: `store.package.ID`, `store.install`, `store.trust`, ...
pub fn store_control_point(ui: &PackageStoreUi, name: &str) -> Option<(f32, f32)> {
    let id = match name {
        "close" => H_CLOSE,
        "back" => H_BACK,
        "refresh" => H_REFRESH,
        "search" => H_SEARCH,
        "repo" => H_REPO,
        "install" => H_INSTALL,
        "uninstall" => H_UNINSTALL,
        "uninstall-yes" => H_UNINSTALL_YES,
        "trust" => H_TRUST,
        "qr" => H_QR,
        "retry" => H_RETRY,
        "prev" => H_PREV,
        "next" => H_NEXT,
        "shot.prev" => H_SHOT_PREV,
        "shot.next" => H_SHOT_NEXT,
        "license" => H_LICENSE,
        "license.prev" => H_LICENSE_PREV,
        "license.next" => H_LICENSE_NEXT,
        "accept" => H_AGREE,
        "desc.more" => H_DESC_MORE,
        "desc.prev" => H_DESC_PREV,
        "desc.next" => H_DESC_NEXT,
        _ => {
            if let Some(n) = name.strip_prefix("filter.") {
                let i = n.parse::<i32>().ok()?;
                return ui.hits.h.iter().find(|h| h.id == H_FILTER && h.arg == i).map(|h| ((h.r.x + h.r.w / 2) as f32, (h.r.y + h.r.h / 2) as f32));
            }
            let n = name.strip_prefix("package.").or_else(|| name.strip_prefix("row."))?;
            let found = n
                .parse::<i32>()
                .ok()
                .and_then(card_ref)
                .or_else(|| {
                    ui.index
                        .packages
                        .iter()
                        .position(|p| p.id == n)
                        .map(PkgRef::Catalog)
                        .or_else(|| ui.installed.iter().position(|p| p.id == n).map(PkgRef::Installed))
                })?;
            return ui.hits.h.iter().find(|h| h.id == H_CARD && h.arg == card_arg(found)).map(|h| ((h.r.x + h.r.w / 2) as f32, (h.r.y + h.r.h / 2) as f32));
        }
    };
    ui.hits.h.iter().find(|h| h.id == id).map(|h| ((h.r.x + h.r.w / 2) as f32, (h.r.y + h.r.h / 2) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> &'static [u8] {
        br#"{
          "version": 1,
          "name": "Test Store",
          "tagline": "Small things",
          "updated": "2026-11-22",
          "featured": "com.example.app",
          "packages": [
            {"id":"com.example.app","name":"Example","version":"1.0.0","type":"app","author":"A",
             "category":"Tools","summary":"An app.","description":"A longer app.","license":"MIT",
             "repository":"https://github.com/example/app","manifest":"packages/com.example.app/package.yaml",
             "icon":"packages/com.example.app/icon.ico","screenshots":["packages/com.example.app/screens/1.png"],
             "tags":["one","two"]}
          ]
        }"#
    }

    #[test]
    fn a_catalog_resolves_its_local_contents() {
        let i = store_index_parse(catalog(), "https://example.org/store/index.json").unwrap();
        assert_eq!(i.name, "Test Store");
        assert_eq!(i.packages.len(), 1);
        let p = &i.packages[0];
        assert_eq!(p.manifest_url, "https://example.org/store/packages/com.example.app/package.yaml");
        assert_eq!(p.screenshots[0].url, "https://example.org/store/packages/com.example.app/screens/1.png");
        assert_eq!(p.screenshots[0].key, "packages/com.example.app/screens/1.png");
        assert!(matches!(p.icon, StoreIconRef::Image(_)));
    }

    // A picture the worker made goes to the slots that show its file: every card whose package names it and has
    // none yet (two packages may name one icon), the detail page's icon and screenshot only while the page still
    // shows that package and that screenshot. A key no job's path can carry has no picture and no download.
    #[test]
    fn a_picture_goes_to_the_slots_that_show_its_file() {
        let json = r#"{"version": 1, "packages": [
            {"id":"com.example.app","name":"Example","version":"1.0.0","type":"app","manifest":"packages/com.example.app/package.yaml",
             "icon":"packages/shared/icon.png","screenshots":["packages/com.example.app/screens/1.png","packages/com.example.app/screens/2.png"]},
            {"id":"com.example.other","name":"Other","version":"1.0.0","type":"app","manifest":"packages/com.example.other/package.yaml",
             "icon":"packages/shared/icon.png","screenshots":["packages/com.example.other/screens/1.png"]},
            {"id":"com.example.glyph","name":"Glyph","version":"1.0.0","type":"widget","manifest":"packages/com.example.glyph/package.yaml","icon":"package"}
          ]}"#;
        let mut ui = PackageStoreUi { index: store_index_parse(json.as_bytes(), "https://example.org/store/index.json").unwrap(), ..Default::default() };
        ui.reset_list_icons();
        let pic = |w: i32| {
            let mut c = Canvas::default();
            assert_eq!(canvas_init(&mut c, w, 3), 0);
            PicNow::There(c)
        };
        let icon = "packages/shared/icon.png";
        assert!(!place_picture(&mut ui, FetchStage::ListIcon, "packages/another.png", pic(4)));
        assert!(place_picture(&mut ui, FetchStage::ListIcon, icon, pic(4)));
        assert_eq!(ui.list_icons.iter().map(|c| c.w).collect::<Vec<_>>(), [4, 4, 0]);
        // (each has one: nothing more to place, and a slot that has its picture keeps it)
        assert!(!place_picture(&mut ui, FetchStage::ListIcon, icon, pic(5)));
        assert_eq!(ui.list_icons[0].w, 4);
        assert!(!place_picture(&mut ui, FetchStage::ListIcon, icon, PicNow::Coming) && !place_picture(&mut ui, FetchStage::ListIcon, icon, PicNow::None));
        // the detail page: no package shown, then one
        let shot = "packages/com.example.app/screens/1.png";
        assert!(!place_picture(&mut ui, FetchStage::Icon, icon, pic(6)) && !place_picture(&mut ui, FetchStage::Screenshot, shot, pic(7)));
        ui.selected = Some("com.example.app".into());
        ui.icon_for = "com.example.app".into();
        ui.shot_for = "com.example.app".into();
        assert!(place_picture(&mut ui, FetchStage::Icon, icon, pic(6)) && ui.icon.image.w == 6);
        assert!(place_picture(&mut ui, FetchStage::Screenshot, shot, pic(7)) && ui.shot.w == 7);
        // another screenshot is on the page now, or another package: a picture asked for before does not land
        ui.shot_index = 1;
        assert!(!place_picture(&mut ui, FetchStage::Screenshot, shot, pic(8)) && ui.shot.w == 7);
        assert!(place_picture(&mut ui, FetchStage::Screenshot, "packages/com.example.app/screens/2.png", pic(8)) && ui.shot.w == 8);
        ui.selected = Some("com.example.other".into());
        assert!(!place_picture(&mut ui, FetchStage::Screenshot, "packages/com.example.app/screens/2.png", pic(9)));
        assert!(!place_picture(&mut ui, FetchStage::Icon, icon, pic(9)) && ui.icon.image.w == 6);
        // keys that name no file a picture can be asked for
        assert!(matches!(asset_picture(&mut ui, FetchStage::ListIcon, "packages/a\nb.png"), PicNow::None));
        assert!(matches!(asset_picture(&mut ui, FetchStage::Screenshot, "../b.png"), PicNow::NoFile));
        assert!(ui.pic_waits.is_empty());
        // the screenshot's frame: one is to come until it was tried, and while its download or its job is on
        let mut ui = PackageStoreUi::default();
        assert!(shot_coming(&ui));
        ui.shot_tried = true;
        assert!(!shot_coming(&ui));
        ui.pic_waits.push(PicWait { stage: FetchStage::Icon, key: icon.into(), src: PicSrc { rel: format!("package-store/assets/{icon}"), size: 1, mtime: 1, form: PicForm::StoreIcon }, job: None });
        assert!(!shot_coming(&ui));
        ui.pic_waits[0].stage = FetchStage::Screenshot;
        assert!(shot_coming(&ui));
        ui.drop_waits(&|stage| stage == FetchStage::Icon);
        assert!(shot_coming(&ui));
        ui.drop_waits(&|stage| stage == FetchStage::Screenshot);
        assert!(!shot_coming(&ui) && ui.pic_waits.is_empty());
    }

    // A data root of its own with a saved catalog of `n` packages, each with an icon file and two screenshots in
    // `package-store/assets/` (their bytes are never read here: a picture is asked for by a stat of its file).
    fn saved_catalog(name: &str, n: usize) -> PackageStoreUi {
        let base = std::env::var("GRIMOIRE_TEST_TMP").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
        let root = base.join(format!("grimoire-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        let mut packages = Vec::new();
        for k in 0..n {
            for file in ["icon.png", "screens/1.png", "screens/2.png"] {
                let path = asset_cache_path(&format!("packages/p{k}/{file}"));
                std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
                std::fs::write(&path, b"a picture's file").unwrap();
            }
            packages.push(format!(
                r#"{{"id":"p{k}","name":"P {k}","version":"1.0.0","type":"app","manifest":"packages/p{k}/package.yaml","icon":"packages/p{k}/icon.png","screenshots":["packages/p{k}/screens/1.png","packages/p{k}/screens/2.png"]}}"#
            ));
        }
        let json = format!(r#"{{"version": 1, "packages": [{}]}}"#, packages.join(","));
        let mut ui = PackageStoreUi { open: true, view: StoreView::List, index: store_index_parse(json.as_bytes(), "https://example.org/store/index.json").unwrap(), ..Default::default() };
        ui.reset_list_icons();
        ui
    }

    // The picture jobs queued, in the queue's order, each by its form's letter and its file's name from the
    // assets folder ("i packages/p0/icon.png").
    fn queued() -> Vec<String> {
        bg_job_paths(&[BgKind::PackagePictures])
            .iter()
            .map(|p| format!("{} {}", &p[..1], p.split_once(":package-store/assets/").map_or("?", |(_, key)| key)))
            .collect()
    }

    // The pictures of the page in front are asked for first. The list asks for its cards' icons, a few jobs at a
    // time; a package's page that opens takes the list's queued jobs back out of the queue and has its own icon and
    // screenshot asked for before them, and the list's are asked for again in the places left. Another screenshot
    // of the page: the one no longer shown leaves the queue, and the new one is again ahead of the list's. Back at
    // the list, its icons are the first asked for. (The jobs stay queued here: no child runs.)
    #[test]
    fn a_pages_pictures_are_asked_for_before_the_lists() {
        let mut ui = saved_catalog("page-first", 6);
        crate::bgjob::bg_test_hold(true);
        for k in 0..6 {
            assert!(matches!(asset_picture(&mut ui, FetchStage::ListIcon, &format!("packages/p{k}/icon.png")), PicNow::Coming));
        }
        assert!(!tick_pictures(&mut ui));
        let icon = |k: usize| format!("i packages/p{k}/icon.png");
        assert_eq!(queued(), [icon(0), icon(1), icon(2), icon(3)]);
        assert_eq!(ui.pic_waits.iter().filter(|w| w.job.is_some()).count(), 4);
        // package 4's page opens, as open_detail opens it
        unqueue_list_pictures(&mut ui);
        assert!(queued().is_empty() && ui.pic_waits.len() == 6 && ui.pic_waits.iter().all(|w| w.job.is_none()));
        ui.selected = Some("p4".into());
        ui.view = StoreView::Detail;
        ui.icon_for = "p4".into();
        ui.shot_for = "p4".into();
        assert!(matches!(asset_picture(&mut ui, FetchStage::Icon, "packages/p4/icon.png"), PicNow::Coming));
        assert!(matches!(asset_picture(&mut ui, FetchStage::Screenshot, "packages/p4/screens/1.png"), PicNow::Coming));
        assert!(!tick_pictures(&mut ui));
        assert_eq!(queued(), [icon(4), "s packages/p4/screens/1.png".to_string(), icon(0), icon(1)]);
        // (the page's icon is its card's file: the card waits on that very job)
        assert_eq!(ui.pic_waits.iter().filter(|w| w.key == "packages/p4/icon.png" && w.job.is_some()).count(), 2);
        // the page's second screenshot, as shot_move shows it: the first one's queued job leaves the queue, and the
        // new one is asked for ahead of the list's icons, which are asked for again behind it
        ui.shot_index = 1;
        ui.drop_waits(&|stage| stage == FetchStage::Screenshot);
        assert_eq!(queued(), [icon(4), icon(0), icon(1)]);
        assert!(matches!(asset_picture(&mut ui, FetchStage::Screenshot, "packages/p4/screens/2.png"), PicNow::Coming));
        assert!(!tick_pictures(&mut ui));
        assert_eq!(queued(), [icon(4), "s packages/p4/screens/2.png".to_string(), icon(0), icon(1)]);
        // the page is left for another package's: its queued jobs leave the queue, but for the icon's, which the
        // card still waits on
        ui.drop_waits(&|stage| matches!(stage, FetchStage::Icon | FetchStage::Screenshot));
        assert_eq!(queued(), [icon(4), icon(0), icon(1)]);
        assert!(ui.pic_waits.len() == 6 && ui.pic_waits.iter().all(|w| w.stage == FetchStage::ListIcon));
        // back at the list with a picture of a page left behind: the list's icons are the first asked for
        ui.view = StoreView::List;
        unqueue_list_pictures(&mut ui);
        ui.pic_waits.insert(0, PicWait { stage: FetchStage::Screenshot, key: "packages/p5/screens/1.png".into(), src: PicSrc::of(&asset_cache_path("packages/p5/screens/1.png"), PicForm::Screenshot).unwrap(), job: None });
        assert!(!tick_pictures(&mut ui));
        assert_eq!(queued(), [icon(0), icon(1), icon(2), icon(3)]);
        // the Store closes: nothing of its pictures stays queued
        ui.cancel_jobs();
        assert!(queued().is_empty() && ui.pic_waits.is_empty());
        crate::bgjob::bg_test_hold(false);
    }

    // A picture that comes for a slot the screen in front does not show is kept in its slot and asks for no draw:
    // a card of another page of the list, the list's cards while a package's page is up, a page's pictures under
    // another view. One that comes for a card of the page drawn, or for the page that is up, does. (The pictures
    // are entries written to the worker's cache here, as its job writes them.)
    #[test]
    fn a_picture_for_a_slot_not_on_the_screen_asks_for_no_draw() {
        let mut ui = saved_catalog("off-screen", 6);
        // the worker's picture of an asset, in its cache, and a wait whose job has ended
        let came = |ui: &mut PackageStoreUi, stage: FetchStage, key: &str| {
            let form = if stage == FetchStage::Screenshot { PicForm::Screenshot } else { PicForm::StoreIcon };
            let src = PicSrc::of(&asset_cache_path(key), form).unwrap();
            std::fs::create_dir_all(crate::pkgpics::picture_cache_dir(form)).unwrap();
            let mut pgm = b"P5\n3 2\n255\n".to_vec();
            pgm.extend_from_slice(&[9; 6]);
            std::fs::write(crate::pkgpics::picture_file(&src, "pgm"), &pgm).unwrap();
            let info = crate::pkgpics::PicInfo { size: src.size, mtime: src.mtime, form, made: crate::bgwork_previews::Made::Preview(3, 2), path: src.rel.clone() };
            std::fs::write(crate::pkgpics::picture_file(&src, "info"), info.to_text()).unwrap();
            let job = src.job_path();
            ui.pic_waits.push(PicWait { stage, key: key.to_string(), src, job });
            tick_pictures(ui)
        };
        // the list's first page drew the cards of packages 0 to 2
        for n in 0..3 {
            hits_add(&mut ui.hits, Rect::new(10 + 100 * n, 10, 90, 90), H_CARD, n);
        }
        assert!(came(&mut ui, FetchStage::ListIcon, "packages/p1/icon.png") && ui.list_icons[1].w == 3);
        assert!(!came(&mut ui, FetchStage::ListIcon, "packages/p4/icon.png") && ui.list_icons[4].w == 3);
        // a package's page is up: the list's icons go on coming into their slots and ask for nothing
        ui.view = StoreView::Detail;
        ui.selected = Some("p0".into());
        ui.icon_for = "p0".into();
        ui.shot_for = "p0".into();
        assert!(!came(&mut ui, FetchStage::ListIcon, "packages/p2/icon.png") && ui.list_icons[2].w == 3);
        assert!(came(&mut ui, FetchStage::Icon, "packages/p0/icon.png") && ui.icon.image.w == 3);
        assert!(came(&mut ui, FetchStage::Screenshot, "packages/p0/screens/1.png") && ui.shot.w == 3);
        // the repository's page over it: the page's screenshot is kept and asks for nothing
        ui.view = StoreView::Qr;
        canvas_free(&mut ui.shot);
        assert!(!came(&mut ui, FetchStage::Screenshot, "packages/p0/screens/1.png") && ui.shot.w == 3);
        // Settings > Updates, the Store not open: any row
        let mut ui = saved_catalog("off-screen-updates", 2);
        ui.open = false;
        assert!(!came(&mut ui, FetchStage::ListIcon, "packages/p0/icon.png"));
        ui.updates_center = true;
        assert!(came(&mut ui, FetchStage::ListIcon, "packages/p1/icon.png") && ui.list_icons[1].w == 3);
        // a screenshot's job that ends with nothing: its frame is drawn again only while the page is up
        let mut ui = saved_catalog("off-screen-none", 1);
        let src = PicSrc::of(&asset_cache_path("packages/p0/screens/1.png"), PicForm::Screenshot).unwrap();
        let wait = |src: &PicSrc| PicWait { stage: FetchStage::Screenshot, key: "packages/p0/screens/1.png".into(), src: src.clone(), job: src.job_path() };
        ui.view = StoreView::Detail;
        ui.pic_waits.push(wait(&src));
        assert!(tick_pictures(&mut ui) && ui.pic_waits.is_empty());
        ui.view = StoreView::Qr;
        ui.pic_waits.push(wait(&src));
        assert!(!tick_pictures(&mut ui) && ui.pic_waits.is_empty());
    }

    // A package cloned into `packages/import/` is installed as the Store opens, and again only when the folder
    // holds something newer (a higher version, or the same one rebuilt); a version older than the installed one
    // is left alone. It is in the Installed tab through the same index an install writes.
    #[test]
    fn a_repository_left_in_the_import_folder_is_installed() {
        let base = std::env::var("GRIMOIRE_TEST_TMP").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
        let root = base.join(format!("grimoire-store-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        let dir = root.join("packages/import/dev");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |version: &str, bytes: &[u8]| {
            let yaml = format!(
                "name: Dev App\nid: com.example.dev\ntype: app\nversion: {version}\nauthor: Tests\ndescription: In the import folder.\nlicense: MIT\nartifact:\n  path: app.wasm\n  sha256: {}\n  size: {}\nspec:\n  runtime: wasm\n  entry: _start\n",
                sha256_of(bytes),
                bytes.len()
            );
            std::fs::write(dir.join("package.yaml"), yaml).unwrap();
            std::fs::write(dir.join("app.wasm"), bytes).unwrap();
        };
        write("1.0.0", b"a module");
        let mut ui = PackageStoreUi::default();
        import_development_for(&mut ui, 1860, 2400);
        let it = app_package_installed("com.example.dev").expect("the folder's package is installed");
        assert_eq!(it.version, "1.0.0");
        assert!(ui.dev_note.contains("Dev App"), "{}", ui.dev_note);
        // (the same folder again: nothing installed, nothing said)
        ui.dev_note.clear();
        import_development_for(&mut ui, 1860, 2400);
        assert!(ui.dev_note.is_empty(), "{}", ui.dev_note);
        // a rebuild at the same version replaces it
        write("1.0.0", b"a newer module");
        import_development_for(&mut ui, 1860, 2400);
        assert!(ui.dev_note.contains("Dev App"));
        assert_eq!(app_package_installed("com.example.dev").unwrap().sha, sha256_of(b"a newer module"));
        // a newer version updates it
        write("1.1.0", b"a newer module");
        import_development_for(&mut ui, 1860, 2400);
        assert_eq!(app_package_installed("com.example.dev").unwrap().version, "1.1.0");
        // an older one is left alone
        write("1.0.0", b"a newer module");
        ui.dev_note.clear();
        import_development_for(&mut ui, 1860, 2400);
        assert!(ui.dev_note.is_empty(), "{}", ui.dev_note);
        assert_eq!(app_package_installed("com.example.dev").unwrap().version, "1.1.0");
    }

    #[test]
    fn a_fresh_address_is_one_no_cache_has_seen() {
        let a = fresh_url("https://example.org/store/index.json");
        assert!(a.starts_with("https://example.org/store/index.json?v="), "{a}");
        // an address that already names something (the GitHub API's `?ref=main`) gets another query
        let b = fresh_url("https://api.example.org/repos/a/b/contents?ref=main");
        assert!(b.starts_with("https://api.example.org/repos/a/b/contents?ref=main&v="), "{b}");
        // an address with no room left for the query is fetched as it is
        let long = format!("https://e.org/{}", "a".repeat(WIDGET_PKG_URL_MAX - "https://e.org/".len()));
        assert_eq!(long.len(), WIDGET_PKG_URL_MAX);
        assert_eq!(fresh_url(&long), long);
    }

    #[test]
    fn a_bad_or_empty_catalog_is_refused() {
        assert!(store_index_parse(b"{}", "https://example.org/index.json").is_err());
        assert!(store_index_parse(b"{\"version\":1,\"packages\":[]}", "https://example.org/index.json").is_err());
        assert!(store_index_parse(b"{\"version\":2,\"packages\":[]}", "https://example.org/index.json").is_err());
    }
}
