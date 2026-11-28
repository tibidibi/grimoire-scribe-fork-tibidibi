//! App packages (2.3): manifests, installs, the apps index and app settings.
//!
//! Apps are a package kind of their own, beside widgets: a `package.yaml` with `type: app` and a
//! WebAssembly artifact under `packages/apps/<id>/`, listed in `packages/apps-index.json`. The
//! manifest parser and the package manager page are shared with widgets; only the folder, index,
//! validation rules and the runtime differ. The full-screen runtime lives in apphost.rs.
//!
//! An app package may use Grimoire's global UI and may request `storage:app-local` and
//! `network:http`. Its state folder survives updates and a remove that keeps state; the artifact
//! never includes it.

use crate::prelude::*;
use crate::sha256::sha256_of;

pub const APP_PKG_INDEX: &str = "packages/apps-index.json";
pub const APP_PKG_DIR: &str = "packages/apps";
pub const APP_PKG_COUNT_MAX: usize = 64;

/// A package kind that is installed as apps are (apps here, themes in themepkg.rs): its folder, its index, how many
/// may be installed, and its word in the messages.
pub(crate) struct PkgKind {
    pub type_: &'static str,
    pub dir: &'static str,
    pub index: &'static str,
    pub count_max: usize,
    pub what: &'static str,
}

impl PkgKind {
    pub fn dir_of(&self, id: &str) -> String {
        format!("{}/{}/{id}", root(), self.dir)
    }
    pub fn index_path(&self) -> String {
        format!("{}/{}", root(), self.index)
    }
}

pub(crate) const APP_KIND: PkgKind =
    PkgKind { type_: "app", dir: APP_PKG_DIR, index: APP_PKG_INDEX, count_max: APP_PKG_COUNT_MAX, what: "app" };
/// A bundled app icon is one small `.ico`; its pixels are decoded by the safe loader's picture worker (pkgpics.rs,
/// issue #236), never here or in appicon.rs.
pub const APP_ICON_MAX: usize = 256 * 1024;
const APP_SETTINGS_FILE_MAX: usize = 64 * 1024;
const APP_SETTINGS_VALUE_MAX: usize = 64;

fn root() -> String {
    store_root()
}

pub fn app_package_dir(id: &str) -> String {
    APP_KIND.dir_of(id)
}

pub fn app_package_index_path() -> String {
    APP_KIND.index_path()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// An app package's id follows the one rule for a package's id (`widget_id_ok`, widgetpkg.rs): lowercase
/// letters, digits, dots and hyphens, at most 128, no dot first or last and no two in a row, and not ending
/// in `.previous`, the name an update keeps the replaced version's folder under.
fn id_ok(id: &str) -> bool {
    widget_id_ok(id)
}

fn semver_ok(v: &str) -> bool {
    let core = v.split_once('-').map_or(v, |(c, _)| c);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    for p in &parts {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        if p.len() > 1 && p.starts_with('0') {
            return false;
        }
    }
    if let Some((_, pre)) = v.split_once('-') {
        if pre.is_empty() || !pre.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return false;
        }
    }
    true
}

fn sha_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn path_ok(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 240
        && !p.starts_with('/')
        && !p.contains('\\')
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Whether a path inside an app's folder is its state folder or a file of it, by the folder's name without regard
/// to ASCII case, as the Kindle's storage compares names (FAT): `State/saved.ico` is a file of `state/` there.
fn in_state(p: &str) -> bool {
    p.split('/').next().is_some_and(|first| first.eq_ignore_ascii_case("state"))
}

/// The capabilities a manifest may declare, and what the host gives a module for each (apphost.rs): its own
/// saved state, and the `http_*` calls. An app needs neither to draw and to be touched.
pub const APP_CAP_STORAGE: &str = "storage:app-local";
pub const APP_CAP_NETWORK: &str = "network:http";

fn cap_ok(c: &str) -> bool {
    c == APP_CAP_STORAGE || c == APP_CAP_NETWORK
}

/// Whether `spec.icon` names a bundled file rather than one of Grimoire's built-in icon names.
pub fn app_icon_is_file(name: &str) -> bool {
    let name = name.trim();
    name.len() > 4 && name.to_ascii_lowercase().ends_with(".ico")
}

/// Where a repository's bundled `.ico` lives: beside its manifest, like the artifact.
pub fn app_package_icon_url(manifest_url: &str, p: &WidgetPackage) -> Option<String> {
    if !app_icon_is_file(&p.spec.icon) {
        return None;
    }
    let base = manifest_url.rsplit_once('/').map_or("", |(b, _)| b);
    if base.is_empty() {
        return None;
    }
    Some(format!("{base}/{}", p.spec.icon.trim()))
}

/// An installed app's `.ico`, or None when its manifest uses a built-in icon name.
pub fn app_package_icon_path(it: &WidgetInstall) -> Option<String> {
    let icon = it.icon.trim();
    // (never a file of the app's state folder: the app writes those itself, and the icon's file is handed to the
    // picture worker)
    if !app_icon_is_file(icon) || !path_ok(icon) || icon == "package.yaml" || in_state(icon) {
        return None;
    }
    Some(format!("{}/{icon}", app_package_dir(&it.id)))
}

/// The picture the worker made of an installed app's bundled icon goes from its cache (pkgpics.rs) as the app is
/// removed or replaced. The icon's file is looked at (one stat), never read.
fn forget_icon_picture(it: &WidgetInstall) {
    use crate::pkgpics::{picture_forget, PicForm, PicSrc};
    if let Some(src) = app_package_icon_path(it).and_then(|path| PicSrc::of(&path, PicForm::AppIcon)) {
        picture_forget(&src);
    }
}

/// Validates an app manifest for this build and device. `canvas_w`/`canvas_h` are accepted for the
/// package manager's shared call shape; an app owns the whole screen and has no size in its spec.
pub fn app_package_validate(p: &WidgetPackage, _canvas_w: i32, _canvas_h: i32) -> Result<(), String> {
    package_validate_common(p, &APP_KIND)?;
    for c in &p.capabilities {
        if !cap_ok(c) {
            return Err(format!("Unknown app capability '{c}'."));
        }
    }
    if p.spec.runtime != "wasm" {
        return Err("`spec.runtime` must be `wasm`; Grimoire does not run native app code".into());
    }
    let icon = p.spec.icon.trim();
    if app_icon_is_file(icon) {
        if !path_ok(icon) || icon == "package.yaml" || in_state(icon) || icon == p.artifact.path {
            return Err("`spec.icon` is not a safe relative path for the app's icon".into());
        }
        if !sha_ok(&p.spec.icon_sha256) {
            return Err("`spec.icon_sha256` is not a SHA-256 digest".into());
        }
        if p.spec.icon_size == 0 || p.spec.icon_size > APP_ICON_MAX as u64 {
            return Err("`spec.icon_size` is outside the allowed size".into());
        }
    } else if !p.spec.icon_sha256.trim().is_empty() || p.spec.icon_size != 0 {
        return Err("`spec.icon_sha256` and `spec.icon_size` are only for a `.ico` icon".into());
    }
    Ok(())
}

/// What every package installed as an app is (an app's or a theme's manifest): its type, id, texts, versions,
/// platforms, artifact and releases.
pub(crate) fn package_validate_common(p: &WidgetPackage, k: &PkgKind) -> Result<(), String> {
    let what = k.what;
    if p.type_ != k.type_ {
        return Err(format!("This repository is not {} {what} package", if what.starts_with('a') { "an" } else { "a" }));
    }
    if p.id.ends_with(WIDGET_ID_PREVIOUS) {
        return Err("`id` cannot end in `.previous`; an update keeps the replaced version's folder under that name".into());
    }
    if !id_ok(&p.id) {
        return Err("`id` must be lowercase letters, digits, dots and hyphens".into());
    }
    if p.name.trim().is_empty() {
        return Err("`name` is missing".into());
    }
    // (the index keeps these four of a package: no longer than its reader takes of a string)
    for (field, text) in [("name", &p.name), ("version", &p.version), ("author", &p.author), ("description", &p.description)] {
        if text.len() > WIDGET_PKG_TEXT_MAX {
            return Err(format!("`{field}` is longer than {WIDGET_PKG_TEXT_MAX} bytes"));
        }
    }
    if !semver_ok(&p.version) {
        return Err(format!("`version` (`{}`) is not a valid version", p.version));
    }
    if !p.min_grimoire_version.is_empty() {
        if !update_version_key(&p.min_grimoire_version, &mut [0; 3]) {
            return Err("`min_grimoire_version` is not a valid version".into());
        }
        if update_version_cmp(GRIMOIRE_VERSION, &p.min_grimoire_version) < 0 {
            return Err(format!("This {what} needs Grimoire {} or newer.", p.min_grimoire_version));
        }
    }
    if p.platforms.is_empty() {
        return Err("`platforms` is an empty list (omit it to mean all devices)".into());
    }
    for name in &p.platforms {
        if !widget_platform_known(name) {
            let all: Vec<&str> = WIDGET_PLATFORMS.iter().map(|(k, _)| *k).collect();
            return Err(format!("Unknown platform '{name}'. Supported: {}", all.join(", ")));
        }
    }
    if !p.platforms.iter().any(|x| x == "all") && !widget_platform_supported(&p.platforms) {
        let current = widget_current_platform();
        let label = if current == "unknown" { "this device".to_string() } else { widget_platform_label(current).to_string() };
        return Err(format!("This {what} does not support your device ({label})."));
    }
    // (the one rule for a package's module, widgetpkg.rs: a relative path inside the package's folder, neither the
    // manifest nor anything of `state/` in any case of their letters, of names the storage walk takes)
    if !crate::widgetpkg::artifact_path_ok(&p.artifact.path) {
        return Err("`artifact.path` is not a safe relative path (it cannot be package.yaml or inside state/)".into());
    }
    if !sha_ok(&p.artifact.sha256) {
        return Err("`artifact.sha256` is not a SHA-256 digest".into());
    }
    if p.artifact.size == 0 || p.artifact.size > WIDGET_PKG_ARTIFACT_MAX {
        return Err("`artifact.size` is outside the allowed size".into());
    }
    for r in &p.releases {
        if !semver_ok(&r.version) {
            return Err("a `releases` entry has an invalid version".into());
        }
        if !(r.url.starts_with("https://") || r.url.starts_with("http://")) {
            return Err("a `releases` entry has no http or https URL".into());
        }
        if !sha_ok(&r.sha256) {
            return Err("a `releases` entry has an invalid sha256".into());
        }
    }
    Ok(())
}

fn json_escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn app_package_index_save(items: &[WidgetInstall]) -> i32 {
    pkg_index_save(&APP_KIND, items)
}

pub(crate) fn pkg_index_save(k: &PkgKind, items: &[WidgetInstall]) -> i32 {
    if root().is_empty() {
        return 0;
    }
    let mut s = String::from("{\n  \"version\": 1,\n  \"packages\": [");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("\n    {\"id\":");
        json_escape(&it.id, &mut s);
        s.push_str(",\"name\":");
        json_escape(&it.name, &mut s);
        s.push_str(",\"version\":");
        json_escape(&it.version, &mut s);
        s.push_str(",\"author\":");
        json_escape(&it.author, &mut s);
        s.push_str(",\"description\":");
        json_escape(&it.description, &mut s);
        s.push_str(",\"icon\":");
        json_escape(&it.icon, &mut s);
        s.push_str(",\"path\":");
        json_escape(&it.path, &mut s);
        s.push_str(",\"sha\":");
        json_escape(&it.sha, &mut s);
        s.push_str(&format!(",\"installed_at\":{},\"source_url\":", it.installed_at));
        json_escape(&it.source_url, &mut s);
        s.push('}');
    }
    s.push_str("\n  ]\n}\n");
    mkdir_p(&format!("{}/packages", root()));
    write_file_atomic(&k.index_path(), s.as_bytes())
}

fn json_get_str(text: &[u8], toks: &[JsonTok], obj: i32, key: &str) -> String {
    let v = json_get(text, toks, obj, key);
    if v < 0 {
        return String::new();
    }
    let mut b = Vec::new();
    if json_string(text, &toks[v as usize], &mut b, WIDGET_PKG_TEXT_MAX + 1) {
        String::from_utf8_lossy(&b).into_owned()
    } else {
        String::new()
    }
}

pub fn app_package_index_load() -> Vec<WidgetInstall> {
    pkg_index_load(&APP_KIND)
}

pub(crate) fn pkg_index_load(k: &PkgKind) -> Vec<WidgetInstall> {
    let Some(data) = read_file_max(&k.index_path(), 256 * 1024) else {
        return Vec::new();
    };
    let mut toks = vec![JsonTok::default(); 4096];
    let n = json_parse(&data, &mut toks);
    if n < 0 {
        qlog!("packages: %s did not parse", k.index);
        return Vec::new();
    }
    let toks = &toks[..n as usize];
    let arr = json_get(&data, toks, 0, "packages");
    if arr < 0 || toks[arr as usize].type_ != JsonType::Array {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..toks[arr as usize].size.min(k.count_max as i32) {
        let it = json_item(toks, arr, i);
        if it < 0 || toks[it as usize].type_ != JsonType::Object {
            continue;
        }
        let id = json_get_str(&data, toks, it, "id");
        if !id_ok(&id) {
            continue;
        }
        out.push(WidgetInstall {
            name: json_get_str(&data, toks, it, "name"),
            id,
            version: json_get_str(&data, toks, it, "version"),
            author: json_get_str(&data, toks, it, "author"),
            description: json_get_str(&data, toks, it, "description"),
            icon: json_get_str(&data, toks, it, "icon"),
            path: json_get_str(&data, toks, it, "path"),
            sha: json_get_str(&data, toks, it, "sha"),
            installed_at: {
                let v = json_get(&data, toks, it, "installed_at");
                let mut n2 = 0i32;
                if v >= 0 && json_int(&data, &toks[v as usize], &mut n2) { n2 as i64 } else { 0 }
            },
            source_url: json_get_str(&data, toks, it, "source_url"),
        });
    }
    out
}

pub fn app_package_installed(id: &str) -> Option<WidgetInstall> {
    app_package_index_load().into_iter().find(|p| p.id == id)
}

fn parse_installed(p: &WidgetInstall) -> Option<WidgetPackage> {
    let path = app_package_dir(&p.id);
    let data = read_file_max(&format!("{path}/package.yaml"), WIDGET_PKG_MANIFEST_MAX)?;
    let text = String::from_utf8_lossy(&data);
    let m = widget_package_parse(&text).ok()?;
    (m.id == p.id && m.type_ == "app").then_some(m)
}

pub fn app_package_installed_manifest(id: &str) -> Option<WidgetPackage> {
    parse_installed(&app_package_installed(id)?)
}

/// An installed app's index entry, its manifest and its module's bytes, for a launch (apphost.rs). The file is
/// checked again here, each time the app is opened, by the check a widget's module has
/// (`widgetpkg::package_artifact`): the installed manifest's `artifact.path` is one the manifest check takes and
/// the index names the same file; it is a plain file inside the app's own folder, reached with no link on the
/// way; it is no larger than an artifact may be, and its SHA-256 is the one the index recorded at the install.
/// What failed goes to the log; the launch says that the artifact could not be read.
pub fn app_package_artifact(id: &str) -> Result<(WidgetInstall, WidgetPackage, Vec<u8>), String> {
    let installed = || "That app is no longer installed.".to_string();
    let it = app_package_installed(id).ok_or_else(installed)?;
    let m = parse_installed(&it).ok_or_else(installed)?;
    match crate::widgetpkg::package_artifact(&app_package_dir(id), APP_PKG_DIR, &it, &m) {
        Ok(bytes) => Ok((it, m, bytes)),
        Err(why) => {
            qlog!("app %s: its module is not loaded: %s", id, why);
            Err("The app's artifact could not be read.".to_string())
        }
    }
}

pub fn app_package_list() -> Vec<WidgetInstall> {
    let mut v = app_package_index_load();
    v.retain(|p| parse_installed(p).is_some());
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    v
}

/// Installs a validated app manifest and artifact. Existing versions are swapped in whole; the
/// app's state is carried over and the old directory is kept as `.previous`.
pub fn app_package_install(manifest: &str, artifact: &[u8], icon: Option<&[u8]>, source_url: &str, canvas_w: i32, canvas_h: i32) -> Result<WidgetInstall, String> {
    let p = widget_package_parse(manifest)?;
    app_package_validate(&p, canvas_w, canvas_h)?;
    let (want_sha, want_size) = widget_package_artifact_digest(&p);
    if want_size != artifact.len() as u64 {
        return Err(format!("The artifact is {} bytes, not {want_size}.", artifact.len()));
    }
    let got = sha256_of(artifact);
    if !got.eq_ignore_ascii_case(want_sha) {
        return Err("The artifact's SHA-256 does not match the manifest.".into());
    }
    let icon_bytes = if app_icon_is_file(&p.spec.icon) {
        let bytes = icon.ok_or("The app's icon was not downloaded.")?;
        if bytes.len() as u64 != p.spec.icon_size {
            return Err(format!("The icon is {} bytes, not {}.", bytes.len(), p.spec.icon_size));
        }
        if !sha256_of(bytes).eq_ignore_ascii_case(&p.spec.icon_sha256) {
            return Err("The icon's SHA-256 does not match the manifest.".into());
        }
        Some(bytes)
    } else {
        None
    };
    if let Some(was) = app_package_installed(&p.id) {
        forget_icon_picture(&was);
    }
    let icon = icon_bytes.map(|b| (p.spec.icon.trim(), b));
    pkg_install_files(&APP_KIND, &p, manifest, artifact, &got, icon, p.spec.icon.trim(), source_url)
}

/// Puts a checked package in place: its manifest, its artifact (whose SHA-256 is `sha`) and an icon file, staged
/// then swapped in whole; the replaced version's folder is kept as `.previous` and its `state` carried over; then the
/// kind's index. The caller has validated the manifest and checked the artifact's size and digest.
pub(crate) fn pkg_install_files(
    k: &PkgKind,
    p: &WidgetPackage,
    manifest: &str,
    artifact: &[u8],
    sha: &str,
    icon_bytes: Option<(&str, &[u8])>,
    icon_name: &str,
    source_url: &str,
) -> Result<WidgetInstall, String> {
    let what = k.what;
    let index = pkg_index_load(k);
    if index.len() >= k.count_max && !index.iter().any(|x| x.id == p.id) {
        return Err(format!("No more {what} packages can be installed."));
    }
    let dir = k.dir_of(&p.id);
    let stage = format!("{}/{}/.install-{}", root(), k.dir, std::process::id());
    let _ = std::fs::remove_dir_all(&stage);
    if std::fs::create_dir_all(&stage).is_err() {
        return Err(format!("Could not make the {what}'s folder."));
    }
    if write_file_atomic(&format!("{stage}/package.yaml"), manifest.as_bytes()) != 0 {
        let _ = std::fs::remove_dir_all(&stage);
        return Err("Could not write the manifest.".into());
    }
    let dest = format!("{stage}/{}", p.artifact.path);
    if let Some(parent) = dest.rsplit_once('/').map(|(d, _)| d) {
        if std::fs::create_dir_all(parent).is_err() {
            let _ = std::fs::remove_dir_all(&stage);
            return Err("Could not make the artifact's folder.".into());
        }
    }
    if write_file_atomic(&dest, artifact) != 0 {
        let _ = std::fs::remove_dir_all(&stage);
        return Err("Could not write the artifact.".into());
    }
    if let Some((icon, bytes)) = icon_bytes {
        let icon_dest = format!("{stage}/{icon}");
        if let Some(parent) = icon_dest.rsplit_once('/').map(|(d, _)| d) {
            if std::fs::create_dir_all(parent).is_err() {
                let _ = std::fs::remove_dir_all(&stage);
                return Err("Could not make the icon's folder.".into());
            }
        }
        if write_file_atomic(&icon_dest, bytes) != 0 {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(format!("Could not write the {what}'s icon."));
        }
    }
    let old = format!("{dir}{WIDGET_ID_PREVIOUS}");
    let _ = std::fs::remove_dir_all(&old);
    if std::fs::metadata(&dir).is_ok() && std::fs::rename(&dir, &old).is_err() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err("Could not move the installed version aside.".into());
    }
    if let Err(e) = std::fs::rename(&stage, &dir) {
        if std::fs::metadata(&old).is_ok() {
            let _ = std::fs::rename(&old, &dir);
        }
        let _ = std::fs::remove_dir_all(&stage);
        return Err(format!("Could not put the {what} in place: {e}"));
    }
    if std::fs::metadata(format!("{old}/state")).is_ok() {
        let _ = std::fs::rename(format!("{old}/state"), format!("{dir}/state"));
    }
    let it = WidgetInstall {
        id: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone(),
        author: p.author.clone(),
        description: p.description.clone(),
        icon: icon_name.to_string(),
        path: format!("{}/{}/{}", k.dir, p.id, p.artifact.path),
        sha: sha.to_string(),
        installed_at: now_secs(),
        source_url: source_url.to_string(),
    };
    let mut next: Vec<WidgetInstall> = index.into_iter().filter(|x| x.id != it.id).collect();
    next.push(it.clone());
    if pkg_index_save(k, &next) != 0 {
        if std::fs::metadata(format!("{dir}/state")).is_ok() {
            let _ = std::fs::rename(format!("{dir}/state"), format!("{old}/state"));
        }
        let _ = std::fs::remove_dir_all(&dir);
        if std::fs::metadata(&old).is_ok() {
            let _ = std::fs::rename(&old, &dir);
        }
        return Err(format!("Could not save the {what} index."));
    }
    Ok(it)
}

/// Removes an installed app and its files. Its `state` folder is kept when `keep_state` is set.
pub fn app_package_remove(id: &str, keep_state: bool) -> Result<(), String> {
    if !id_ok(id) {
        return Err("That id is not an app package's.".into());
    }
    if let Some(was) = app_package_installed(id) {
        forget_icon_picture(&was);
    }
    pkg_remove(&APP_KIND, id, keep_state)
}

/// Removes an installed package of kind k and its files (its `state` folder kept when `keep_state` is set), then its
/// line of the kind's index.
pub(crate) fn pkg_remove(k: &PkgKind, id: &str, keep_state: bool) -> Result<(), String> {
    if !id_ok(id) {
        return Err(format!("That id is not {} {} package's.", if k.what.starts_with('a') { "an" } else { "a" }, k.what));
    }
    let dir = k.dir_of(id);
    if keep_state {
        let state = format!("{dir}/state");
        if std::fs::metadata(&state).is_ok() {
            let keep = format!("{}/{}/.state-{}", root(), k.dir, std::process::id());
            let _ = std::fs::remove_dir_all(&keep);
            std::fs::rename(&state, &keep).ok();
            let _ = std::fs::remove_dir_all(&dir);
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::rename(&keep, format!("{dir}/state"));
            }
        } else {
            let _ = std::fs::remove_dir_all(&dir);
        }
    } else {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut index = pkg_index_load(k);
    index.retain(|p| p.id != id);
    if pkg_index_save(k, &index) != 0 {
        return Err(format!("Could not save the {} index.", k.what));
    }
    Ok(())
}

pub fn app_settings_path(id: &str) -> String {
    format!("{}/state/settings.json", app_package_dir(id))
}

pub fn app_settings_defaults(schema: &[WidgetSetting]) -> Vec<(String, String)> {
    schema.iter().map(|s| (s.key.clone(), s.default.clone())).collect()
}

fn setting_ok(s: &WidgetSetting, value: &str) -> bool {
    match s.kind {
        WidgetSettingKind::Toggle => value == "true" || value == "false",
        WidgetSettingKind::Choice => s.choices.iter().any(|c| c == value),
    }
}

pub fn app_settings_load(id: &str, schema: &[WidgetSetting]) -> Vec<(String, String)> {
    let mut out = app_settings_defaults(schema);
    if !id_ok(id) {
        return out;
    }
    let Some(data) = read_file_max(&app_settings_path(id), APP_SETTINGS_FILE_MAX) else { return out };
    let mut toks = vec![JsonTok::default(); 2048];
    let n = json_parse(&data, &mut toks);
    if n < 0 {
        return out;
    }
    let toks = &toks[..n as usize];
    let values = json_get(&data, toks, 0, "values");
    if values < 0 || toks[values as usize].type_ != JsonType::Object {
        return out;
    }
    for (i, s) in schema.iter().enumerate() {
        let tok = json_get(&data, toks, values, &s.key);
        if tok < 0 {
            continue;
        }
        let mut buf = Vec::new();
        if !json_string(&data, &toks[tok as usize], &mut buf, APP_SETTINGS_VALUE_MAX + 1) {
            continue;
        }
        let value = String::from_utf8_lossy(&buf);
        if setting_ok(s, &value) {
            out[i].1 = value.into_owned();
        }
    }
    out
}

pub fn app_settings_save(id: &str, values: &[(String, String)]) -> i32 {
    if !id_ok(id) {
        return -1;
    }
    let mut body = String::from("{\n  \"version\": 1,\n  \"values\": {");
    let mut n = 0usize;
    for (key, value) in values {
        if key.is_empty() || value.len() > APP_SETTINGS_VALUE_MAX || value.chars().any(char::is_control) {
            continue;
        }
        if n > 0 {
            body.push(',');
        }
        body.push_str("\n    ");
        json_escape(key, &mut body);
        body.push_str(": ");
        json_escape(value, &mut body);
        n += 1;
    }
    if n > 0 {
        body.push('\n');
    }
    body.push_str("  }\n}\n");
    let dir = format!("{}/state", app_package_dir(id));
    if mkdir_p(&dir) != 0 {
        return -1;
    }
    write_file_atomic(&app_settings_path(id), body.as_bytes())
}

pub fn app_settings_clear(id: &str) -> i32 {
    if !id_ok(id) {
        return -1;
    }
    match std::fs::remove_file(app_settings_path(id)) {
        Ok(()) => 0,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> String {
        r#"name: Test App
id: test.app
type: app
version: 1.0.0
author: Tester
description: An app
platforms: [all]
artifact:
  path: app.wasm
  sha256: 0000000000000000000000000000000000000000000000000000000000000000
  size: 8
spec:
  runtime: wasm
  status_bar: false
capabilities: [storage:app-local, network:http]
settings:
  - key: color
    label: Colour
    type: choice
    choices: [Ink, Grey]
    default: Ink
"#.into()
    }

    #[test]
    fn an_app_manifest_is_an_app() {
        let p = widget_package_parse(&manifest()).unwrap();
        assert_eq!(p.type_, "app");
        assert!(!p.spec.status_bar);
        assert!(app_package_validate(&p, 1000, 1000).is_ok());
        let widget = manifest().replace("type: app", "type: widget");
        let p = widget_package_parse(&widget).unwrap();
        assert!(app_package_validate(&p, 1000, 1000).is_err());
    }

    #[test]
    fn install_update_remove_and_settings_keep_the_app_state_folder() {
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-apppkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        let artifact = b"fixture app bytes";
        let sha = sha256_of(artifact);
        let m = manifest()
            .replace("0000000000000000000000000000000000000000000000000000000000000000", &sha)
            .replace("size: 8", &format!("size: {}", artifact.len()));
        let it = app_package_install(&m, artifact, None, "https://example.org/app.yaml", 1860, 2480).unwrap();
        assert_eq!(it.id, "test.app");
        assert_eq!(app_package_list().len(), 1);
        assert_eq!(app_package_installed_manifest(&it.id).unwrap().name, "Test App");
        let schema = app_package_installed_manifest(&it.id).unwrap().settings;
        let changed = vec![("color".to_string(), "Grey".to_string())];
        assert_eq!(app_settings_save(&it.id, &changed), 0);
        assert_eq!(app_settings_load(&it.id, &schema), changed);

        let m2 = m.replace("version: 1.0.0", "version: 1.1.0");
        assert_eq!(app_package_install(&m2, artifact, None, "https://example.org/app.yaml", 1860, 2480).unwrap().version, "1.1.0");
        assert_eq!(app_settings_load(&it.id, &schema), changed);
        app_package_remove(&it.id, true).unwrap();
        assert!(app_package_list().is_empty());
        assert!(std::path::Path::new(&app_settings_path(&it.id)).exists());
        app_settings_clear(&it.id);
        let _ = std::fs::remove_dir_all(&root);
    }

    // An icon is never a file of the app's state folder, whatever the case its manifest writes the folder's name
    // in: the storage takes `State/` and `STATE/` for `state/`. Neither the launcher's look nor an install takes
    // such a path; a folder that only begins with the word, or has it further down, is another folder.
    #[test]
    fn an_icon_in_the_state_folder_is_refused_in_any_case() {
        let installed = |icon: &str| app_package_icon_path(&WidgetInstall { id: "test.app".into(), icon: icon.into(), ..Default::default() });
        for icon in ["state/saved.ico", "State/saved.ico", "STATE/saved.ico", "sTaTe/a/b.ico"] {
            assert_eq!(installed(icon), None, "{icon}");
        }
        for icon in ["icon.ico", "stateful/icon.ico", "art/state/icon.ico", "States/icon.ico"] {
            assert!(installed(icon).is_some_and(|p| p.ends_with(&format!("/test.app/{icon}"))), "{icon}");
        }
        assert!(in_state("state") && in_state("STATE") && in_state("State/x") && !in_state("") && !in_state("x/state"));
        let with_icon = |icon: &str| {
            let m = manifest().replace("  status_bar: false", &format!("  icon: {icon}\n  icon_sha256: {}\n  icon_size: 18\n  status_bar: false", sha256_of(b"fixture icon bytes")));
            app_package_validate(&widget_package_parse(&m).unwrap(), 1000, 1000)
        };
        assert!(with_icon("icon.ico").is_ok() && with_icon("art/state/icon.ico").is_ok());
        for icon in ["state/saved.ico", "State/saved.ico", "STATE/saved.ico"] {
            let err = with_icon(icon).unwrap_err();
            assert!(err.contains("`spec.icon` is not a safe relative path"), "{icon}: {err}");
        }
    }

    /// A data root of one test's own.
    fn fresh_root(name: &str) -> std::path::PathBuf {
        ui_init(300.0);
        let base = std::env::var("GRIMOIRE_TEST_TMP").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
        let root = base.join(format!("grimoire-apppkg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        root
    }

    /// The test manifest for these bytes as its artifact.
    fn manifest_for(artifact: &[u8]) -> String {
        manifest()
            .replace("0000000000000000000000000000000000000000000000000000000000000000", &sha256_of(artifact))
            .replace("size: 8", &format!("size: {}", artifact.len()))
    }

    // An app's id follows the one rule for a package's id (widget_id_ok): an update keeps the replaced
    // version's folder as <id>.previous, so no id ends in that. Through the manifest check, an install, the
    // index's reader, a removal and the settings.
    #[test]
    fn an_apps_id_follows_the_rule_for_a_packages_id() {
        for (id, ok) in [
            ("test.app", true),
            ("test.app.previous", false),
            ("a.previous", false),
            ("x-1.previous", false),
            // (only the end of an id: these name no replaced version's folder)
            ("previous", true),
            ("test.previous.app", true),
            ("test.app-previous", true),
            ("test.app.previous.previous", false),
            // (and the rest of the rule)
            ("Test.App", false),
            (".test.app", false),
            ("test.app.", false),
            ("test..app", false),
            ("test app", false),
            ("", false),
        ] {
            assert_eq!(id_ok(id), ok, "{id}");
            assert_eq!(id_ok(id), widget_id_ok(id), "{id}: the same rule as a widget's");
            let p = widget_package_parse(&manifest().replace("id: test.app", &format!("id: {id}"))).unwrap();
            assert_eq!(app_package_validate(&p, 1000, 1000).is_ok(), ok, "{id}");
        }
        assert!(id_ok(&"a".repeat(128)) && !id_ok(&"a".repeat(129)));
        let refused = |id: &str| app_package_validate(&widget_package_parse(&manifest().replace("id: test.app", &format!("id: {id}"))).unwrap(), 1000, 1000).unwrap_err();
        assert_eq!(refused("test.app.previous"), "`id` cannot end in `.previous`; an update keeps the replaced version's folder under that name");
        assert_eq!(refused("Test.App"), "`id` must be lowercase letters, digits, dots and hyphens");

        let root = fresh_root("id");
        let artifact = b"fixture app bytes";
        let m = manifest_for(artifact);
        // An app, and one whose id is that id with `.previous` after it: the second is not installed.
        let it = app_package_install(&m, artifact, None, "", 1860, 2480).unwrap();
        let e = app_package_install(&m.replace("id: test.app", "id: test.app.previous"), artifact, None, "", 1860, 2480).unwrap_err();
        assert!(e.contains("`id` cannot end in `.previous`"), "{e}");
        assert!(!root.join("packages/apps/test.app.previous").exists());
        // An update of the first keeps the replaced version in that folder, which is nobody else's, and the
        // app's state goes on with the new version.
        let state = format!("{}/state", app_package_dir(&it.id));
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(format!("{state}/session"), b"keep").unwrap();
        app_package_install(&m.replace("version: 1.0.0", "version: 1.1.0"), artifact, None, "", 1860, 2480).unwrap();
        let kept = std::fs::read_to_string(root.join("packages/apps/test.app.previous/package.yaml")).unwrap();
        assert!(kept.contains("version: 1.0.0") && kept.contains("id: test.app\n"));
        assert_eq!(std::fs::read(format!("{state}/session")).unwrap(), b"keep");
        // The index reads no such id, whoever wrote it there; a removal and the settings take none either.
        let index = std::fs::read_to_string(app_package_index_path()).unwrap();
        std::fs::write(app_package_index_path(), index.replace("\"id\":\"test.app\"", "\"id\":\"test.app.previous\"")).unwrap();
        assert!(app_package_index_load().is_empty(), "an index entry with such an id is skipped");
        assert!(app_package_list().is_empty() && app_package_installed("test.app.previous").is_none());
        assert!(app_package_remove("test.app.previous", false).is_err());
        assert!(root.join("packages/apps/test.app.previous").exists(), "and removing by that id touches nothing");
        assert_eq!(app_settings_save("test.app.previous", &[("color".to_string(), "Grey".to_string())]), -1);
        assert!(!root.join("packages/apps/test.app.previous/state").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    // A name, an author and a description are no longer than the index keeps of each (WIDGET_PKG_TEXT_MAX, as
    // for a widget): what an install writes into the index is what is read back.
    #[test]
    fn an_apps_name_version_author_and_description_are_no_longer_than_the_index_keeps() {
        assert_eq!(WIDGET_PKG_TEXT_MAX, 1023);
        let artifact = b"fixture app bytes";
        let good = manifest_for(artifact);
        // (words, `n` bytes of them, the last a letter)
        let long = |n: usize| {
            let mut s = "word ".repeat((n - 1) / 5);
            s.push_str(&"x".repeat(n - s.len()));
            s
        };
        // (a version, `n` bytes of it: three numbers and a prerelease of letters)
        let version = |n: usize| format!("1.0.0-{}", "x".repeat(n - 6));
        assert_eq!(long(1023).len(), 1023);
        assert_eq!(version(1023).len(), 1023);
        for (line, field) in [("name: Test App", "name"), ("version: 1.0.0", "version"), ("author: Tester", "author"), ("description: An app", "description")] {
            let text = |n: usize| if field == "version" { version(n) } else { long(n) };
            let at = widget_package_parse(&good.replace(line, &format!("{field}: {}", text(1023)))).unwrap();
            assert_eq!(app_package_validate(&at, 1860, 2480), Ok(()), "{field} of 1,023 bytes");
            let past = widget_package_parse(&good.replace(line, &format!("{field}: {}", text(1024)))).unwrap();
            assert_eq!(app_package_validate(&past, 1860, 2480), Err(format!("`{field}` is longer than 1023 bytes")), "{field} of 1,024 bytes");
            let far = widget_package_parse(&good.replace(line, &format!("{field}: {}", text(200_000)))).unwrap();
            assert_eq!(app_package_validate(&far, 1860, 2480), Err(format!("`{field}` is longer than 1023 bytes")), "{field} of 200,000 bytes");
        }
        // What an install writes into the index is what is read back: the four at their longest, whole.
        let root = fresh_root("texts");
        let m = good
            .replace("name: Test App", &format!("name: {}", long(1023)))
            .replace("version: 1.0.0", &format!("version: {}", version(1023)))
            .replace("author: Tester", &format!("author: {}", long(1023)))
            .replace("description: An app", &format!("description: {}", long(1023)));
        app_package_install(&m, artifact, None, "", 1860, 2480).unwrap();
        let read = app_package_index_load();
        assert_eq!(read.len(), 1);
        assert_eq!((read[0].name.as_str(), read[0].author.as_str(), read[0].description.as_str()), (long(1023).as_str(), long(1023).as_str(), long(1023).as_str()));
        assert_eq!(read[0].version, version(1023));
        // (and a manifest with a longer one is not installed, and leaves the index as it was)
        let e = app_package_install(&m.replace("id: test.app", "id: test.other").replace(&format!("description: {}", long(1023)), &format!("description: {}", long(255_000))), artifact, None, "", 1860, 2480).unwrap_err();
        assert_eq!(e, "`description` is longer than 1023 bytes");
        assert_eq!(app_package_index_load(), read);
        assert!(!root.join("packages/apps/test.other").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    // `artifact.path` follows the one rule for a package's module (artifact_path_ok, as for a widget): a relative
    // path inside the package's folder, neither the manifest nor anything of `state/` in any case of their
    // letters, and of names the storage walk takes. A refused path is not installed.
    #[test]
    fn an_apps_artifact_path_follows_the_rule_for_a_packages_module() {
        let artifact = b"fixture app bytes";
        let good = manifest_for(artifact);
        let with = |path: &str| good.replace("path: app.wasm", &format!("path: {path}"));
        let check = |path: &str| app_package_validate(&widget_package_parse(&with(path)).unwrap(), 1860, 2480);
        for path in ["app.wasm", "dist/app.wasm", "a/b/c/app.bin", "stateful/app.wasm", "States/app.wasm", "art/state/app.wasm", "package.yaml.wasm", "App.Wasm"] {
            assert_eq!(check(path), Ok(()), "{path}");
        }
        let longest = format!("{}/app.wasm", "d".repeat(231));
        assert_eq!((longest.len(), check(&longest)), (240, Ok(())));
        assert!(check(&format!("d{longest}")).is_err());
        let refused = "`artifact.path` is not a safe relative path (it cannot be package.yaml or inside state/)";
        for path in [
            "package.yaml",
            "state",
            "state/app.wasm",
            // (the manifest and the state folder in any case of their letters: a storage that ignores case finds them)
            "Package.yaml",
            "PACKAGE.YAML",
            "STATE/runtime.bin",
            "State/settings.json",
            "sTaTe",
            "state/http/1-2.body",
            "../app.wasm",
            "dist/../app.wasm",
            "./app.wasm",
            "/app.wasm",
            "dist//app.wasm",
            "dist\\app.wasm",
            // (names the storage walk does not take: the module is read through it when the app is opened)
            ".dist/app.wasm",
            "dist/.app.wasm",
            "dist./app.wasm",
            "app.sdr/app.wasm",
            "system/app.wasm",
        ] {
            assert_eq!(check(path), Err(refused.to_string()), "{path}");
        }
        let root = fresh_root("path");
        for path in ["Package.yaml", "STATE/runtime.bin", ".dist/app.wasm"] {
            assert_eq!(app_package_install(&with(path), artifact, None, "", 1860, 2480).unwrap_err(), refused, "{path}");
            assert!(!root.join("packages/apps/test.app").exists() && app_package_index_load().is_empty(), "{path}: nothing is installed");
        }
        // A path the rule takes installs where it says.
        let it = app_package_install(&with("dist/App.Wasm"), artifact, None, "", 1860, 2480).unwrap();
        assert_eq!(it.path, "packages/apps/test.app/dist/App.Wasm");
        assert_eq!(std::fs::read(root.join("packages/apps/test.app/dist/App.Wasm")).unwrap(), artifact);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bundled_icon_is_checked_and_written() {
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-appicon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        let artifact = b"fixture app bytes";
        let icon = b"fixture icon bytes";
        let sha = sha256_of(artifact);
        let icon_sha = sha256_of(icon);
        let m = manifest()
            .replace("0000000000000000000000000000000000000000000000000000000000000000", &sha)
            .replace("size: 8", &format!("size: {}", artifact.len()))
            .replace(
                "  status_bar: false",
                &format!("  icon: icon.ico\n  icon_sha256: {icon_sha}\n  icon_size: {}\n  status_bar: false", icon.len()),
            );
        let it = app_package_install(&m, artifact, Some(icon), "https://example.org/app.yaml", 1860, 2480).unwrap();
        let path = app_package_icon_path(&it).expect("the icon path is kept");
        assert_eq!(std::fs::read(path).unwrap(), icon);
        let err = app_package_install(&m, artifact, None, "https://example.org/app.yaml", 1860, 2480).unwrap_err();
        assert!(err.contains("icon"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
