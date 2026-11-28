//! Theme packages: a `package.yaml` with `type: theme` and a `theme.ini` artifact under `packages/themes/<id>/`,
//! listed in `packages/themes-index.json`. They are installed, updated and removed as apps are (apppkg.rs's
//! PkgKind), from the Package Store or `packages/import/`, and carry no code: looks/package.rs draws one as Classic
//! with the measures its theme.ini sets. Settings > Display > Style lists them after the built-in themes.

use crate::apppkg::{package_validate_common, pkg_index_load, pkg_install_files, pkg_remove, PkgKind};
use crate::looks::package::{theme_spec_parse, ThemeSpec, THEME_FILE_MAX};
use crate::prelude::*;
use crate::sha256::sha256_of;

pub const THEME_PKG_DIR: &str = "packages/themes";
pub const THEME_PKG_INDEX: &str = "packages/themes-index.json";
/// (with the built-in themes, a Settings.theme number fits a byte)
pub const THEME_PKG_COUNT_MAX: usize = 32;

const THEME_KIND: PkgKind =
    PkgKind { type_: "theme", dir: THEME_PKG_DIR, index: THEME_PKG_INDEX, count_max: THEME_PKG_COUNT_MAX, what: "theme" };

/// Validates a theme manifest: what every package is, and an artifact no larger than a theme.ini may be. The file's
/// keys are checked at the install, when its bytes are there.
pub fn theme_package_validate(p: &WidgetPackage) -> Result<(), String> {
    package_validate_common(p, &THEME_KIND)?;
    if p.artifact.size > THEME_FILE_MAX as u64 {
        return Err(format!("A theme's `artifact` is at most {THEME_FILE_MAX} bytes"));
    }
    if !p.capabilities.is_empty() {
        return Err("A theme package asks for no capabilities".into());
    }
    Ok(())
}

pub fn theme_package_install(manifest: &str, artifact: &[u8], source_url: &str) -> Result<WidgetInstall, String> {
    let p = widget_package_parse(manifest)?;
    theme_package_validate(&p)?;
    let (want_sha, want_size) = widget_package_artifact_digest(&p);
    if want_size != artifact.len() as u64 {
        return Err(format!("The artifact is {} bytes, not {want_size}.", artifact.len()));
    }
    let got = sha256_of(artifact);
    if !got.eq_ignore_ascii_case(want_sha) {
        return Err("The artifact's SHA-256 does not match the manifest.".into());
    }
    theme_spec_parse(artifact)?;
    pkg_install_files(&THEME_KIND, &p, manifest, artifact, &got, None, "", source_url)
}

pub fn theme_package_remove(id: &str) -> Result<(), String> {
    pkg_remove(&THEME_KIND, id, false)
}

pub fn theme_package_installed(id: &str) -> Option<WidgetInstall> {
    pkg_index_load(&THEME_KIND).into_iter().find(|p| p.id == id)
}

pub fn theme_package_installed_manifest(id: &str) -> Option<WidgetPackage> {
    let data = read_file_max(&format!("{}/package.yaml", THEME_KIND.dir_of(id)), WIDGET_PKG_MANIFEST_MAX)?;
    let m = widget_package_parse(&String::from_utf8_lossy(&data)).ok()?;
    (m.id == id && m.type_ == "theme").then_some(m)
}

/// An installed theme's theme.ini, read again and checked against the digest its install recorded.
fn installed_spec(it: &WidgetInstall) -> Option<ThemeSpec> {
    let m = theme_package_installed_manifest(&it.id)?;
    if !crate::widgetpkg::artifact_path_ok(&m.artifact.path) {
        return None;
    }
    let bytes = read_file_max(&format!("{}/{}", THEME_KIND.dir_of(&it.id), m.artifact.path), THEME_FILE_MAX)?;
    if !sha256_of(&bytes).eq_ignore_ascii_case(&it.sha) {
        qlog!("themes: %s's theme.ini is not the one installed; it is left out", &it.id);
        return None;
    }
    theme_spec_parse(&bytes).ok()
}

/// The installed themes whose files read, by name.
pub fn theme_package_list() -> Vec<WidgetInstall> {
    let mut v = pkg_index_load(&THEME_KIND);
    v.retain(|p| installed_spec(p).is_some());
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    v
}

/// Each installed theme's id, name and theme.ini, in the Style list's order (looks/package.rs).
pub fn theme_package_specs() -> Vec<(String, String, ThemeSpec)> {
    let mut v = pkg_index_load(&THEME_KIND);
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    v.into_iter().filter_map(|it| installed_spec(&it).map(|s| (it.id, it.name, s))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::looks::package::{package_look, package_look_count, package_looks_reload};
    use crate::looks::{Corner, UiTheme, LOOKS};

    fn manifest_for(ini: &[u8], version: &str) -> String {
        format!(
            "name: Paper Cut\nid: test.paper-cut\ntype: theme\nversion: {version}\nauthor: Tester\ndescription: Cut corners.\n\
             license: MIT\nartifact:\n  path: theme.ini\n  sha256: {}\n  size: {}\n",
            sha256_of(ini),
            ini.len()
        )
    }

    // A theme package goes in as an app does, is a Look in the Style list after the built-in themes, follows its
    // update and goes again; a theme.ini the parser refuses, or a theme that asks for capabilities, is not installed.
    #[test]
    fn a_theme_package_installs_is_a_look_and_is_removed() {
        ui_init(300.0);
        let root = std::env::temp_dir().join(format!("grimoire-themepkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(store_init(root.to_str().unwrap()), 0);
        package_looks_reload();
        assert_eq!(UiTheme::count(), LOOKS.len());

        let ini = b"frame = cut\ncard = 8\n";
        let it = theme_package_install(&manifest_for(ini, "1.0.0"), ini, "https://example.org/theme.yaml").unwrap();
        assert_eq!((it.id.as_str(), it.path.as_str()), ("test.paper-cut", "packages/themes/test.paper-cut/theme.ini"));
        assert_eq!(theme_package_list().len(), 1);
        package_looks_reload();
        assert_eq!((UiTheme::count(), package_look_count()), (LOOKS.len() + 1, 1));
        let t = UiTheme::from_id(b"test.paper-cut").unwrap();
        assert!(t.is_package());
        assert_eq!((t.look().name(), t.look().corner(Corner::Card)), ("Paper Cut", ui_px(8.0)));
        let first = package_look(0).unwrap() as *const _;
        package_looks_reload();
        assert_eq!(package_look(0).unwrap() as *const _, first, "an unchanged theme is the same look after a reload");

        let ini2 = b"frame = cut\ncard = 12\n";
        theme_package_install(&manifest_for(ini2, "1.1.0"), ini2, "").unwrap();
        package_looks_reload();
        assert_eq!(UiTheme::from_id(b"test.paper-cut").unwrap().look().corner(Corner::Card), ui_px(12.0));

        // the theme.ini changed behind the index's back: left out, not drawn
        std::fs::write(root.join("packages/themes/test.paper-cut/theme.ini"), b"card = 40\n").unwrap();
        assert!(theme_package_list().is_empty());
        package_looks_reload();
        assert_eq!(UiTheme::from_id(b"test.paper-cut"), None);

        let bad = b"colour = red\n";
        assert!(theme_package_install(&manifest_for(bad, "2.0.0"), bad, "").unwrap_err().contains("unknown key"));
        let caps = manifest_for(ini, "2.0.0") + "capabilities: [network:http]\n";
        assert!(theme_package_install(&caps, ini, "").is_err());
        let app = manifest_for(ini, "2.0.0").replace("type: theme", "type: app");
        assert!(theme_package_install(&app, ini, "").unwrap_err().contains("not a theme package"));

        theme_package_remove("test.paper-cut").unwrap();
        assert!(theme_package_installed("test.paper-cut").is_none());
        package_looks_reload();
        assert_eq!(UiTheme::count(), LOOKS.len());
        let _ = std::fs::remove_dir_all(&root);
    }
}
