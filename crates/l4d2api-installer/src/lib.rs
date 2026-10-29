//! `l4d2api-installer`: Windows Steam discovery and versioned install.
//!
//! Never assumes `C:\Program Files (x86)\Steam`. Discovers the Steam root
//! from supported config locations, enumerates `libraryfolders.vdf`,
//! resolves AppID 550 via `appmanifest_550.acf`, and validates a real
//! install before touching anything. Only files owned by this integration
//! (the versioned `.vpk` plus the `ems/tiktools_l4d2/` handshake files) are
//! ever written or removed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

pub const APP_ID: u32 = 550;
pub const ADDON_FILE_PREFIX: &str = "tiktools-l4d2api-";
pub const ADDON_FILE_SUFFIX: &str = ".vpk";
pub const EMS_DIR: &str = "left4dead2/ems/tiktools_l4d2";
pub const MANIFEST_FILE: &str = "ems/tiktools_l4d2/installed.manifest";

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("not a L4D2 game directory: {0}")]
    NotAGameDir(String),
    #[error("refusing path escape: {0}")]
    PathEscape(String),
    #[error("manifest error: {0}")]
    Manifest(String),
    #[error("game appears to be running; restart it before {0}")]
    GameRunning(String),
}

/// SHA-256 hex of bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Join `base` with a manifest-relative Unix path, refusing escapes.
/// Only normal components are allowed; `..`, drive prefixes and absolute
/// paths are rejected so an inventory can never reach outside the game dir.
pub fn join_owned(base: &Path, rel: &str) -> Result<PathBuf, Error> {
    if rel.is_empty() || rel.starts_with('/') || rel.starts_with('\\') {
        return Err(Error::PathEscape(rel.to_owned()));
    }
    let mut out = base.to_owned();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(Error::PathEscape(rel.to_owned()));
        }
        let component = Path::new(part);
        if component.components().count() != 1
            || matches!(
                component.components().next(),
                Some(Component::Prefix(_) | Component::RootDir)
            )
        {
            return Err(Error::PathEscape(rel.to_owned()));
        }
        out.push(part);
    }
    Ok(out)
}

/// Minimal `libraryfolders.vdf` reader: extracts every quoted `path` value.
/// Full VDF parsing is unnecessary; library entries always carry a `path` key.
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut pending_path_key = false;
    for raw_line in vdf.lines() {
        let mut quoted: Vec<&str> = Vec::new();
        let mut rest = raw_line;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            match after.find('"') {
                Some(end) => {
                    quoted.push(&after[..end]);
                    rest = &after[end + 1..];
                }
                None => break,
            }
        }
        if quoted.is_empty() {
            pending_path_key = false;
            continue;
        }
        if quoted.len() == 1 && quoted[0].eq_ignore_ascii_case("path") {
            pending_path_key = true;
            continue;
        }
        if quoted.len() >= 2 && quoted[quoted.len() - 2].eq_ignore_ascii_case("path") {
            paths.push(PathBuf::from(quoted[quoted.len() - 1]));
            pending_path_key = false;
        } else if pending_path_key && quoted.len() == 1 {
            paths.push(PathBuf::from(quoted[0]));
            pending_path_key = false;
        } else {
            pending_path_key = false;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Minimal `appmanifest_550.acf` reader: returns the `installdir` value.
pub fn parse_appmanifest_installdir(acf: &str) -> Option<String> {
    for line in acf.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(at) = lower.find("\"installdir\"") {
            let after = &line[at + "\"installdir\"".len()..];
            if let Some(value) = last_quoted(after) {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn last_quoted(text: &str) -> Option<&str> {
    let mut found = None;
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        match after.find('"') {
            Some(end) => {
                found = Some(&after[..end]);
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    found
}

/// Candidate Steam roots in priority order: explicit override, Windows
/// registry-adjacent config locations are resolved by the caller (TikTools
/// native host); this helper only orders and dedups.
pub fn candidate_steam_roots(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = explicit {
        roots.push(dir.to_owned());
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(program_files) = std::env::var("ProgramFiles(x86)") {
            roots.push(PathBuf::from(program_files).join("Steam"));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            roots.push(PathBuf::from(program_files).join("Steam"));
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// Resolve every AppID-550 install under the given Steam roots (primary +
/// secondary libraries). Returns validated game directories.
pub fn find_l4d2_installs(steam_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut installs = Vec::new();
    for root in steam_roots {
        let mut libraries = vec![root.clone()];
        let folders = root.join("steamapps/libraryfolders.vdf");
        if let Ok(text) = fs::read_to_string(&folders) {
            libraries.extend(parse_library_folders(&text));
        }
        for library in libraries {
            let manifest = library.join("steamapps").join("appmanifest_550.acf");
            let Ok(text) = fs::read_to_string(&manifest) else {
                continue;
            };
            let Some(installdir) = parse_appmanifest_installdir(&text) else {
                continue;
            };
            let game_dir = library.join("steamapps").join("common").join(installdir);
            if validate_game_dir(&game_dir).is_ok() {
                installs.push(game_dir);
            }
        }
    }
    installs.sort();
    installs.dedup();
    installs
}

/// A real install has `left4dead2/` plus the game executable. The user may
/// pick a folder when discovery fails; that folder must pass this check.
pub fn validate_game_dir(game_dir: &Path) -> Result<(), Error> {
    if !game_dir.join("left4dead2").is_dir() {
        return Err(Error::NotAGameDir(game_dir.display().to_string()));
    }
    #[cfg(target_os = "windows")]
    let exe = "left4dead2.exe";
    #[cfg(not(target_os = "windows"))]
    let exe = "left4dead2.exe";
    if !game_dir.join(exe).is_file() {
        return Err(Error::NotAGameDir(format!(
            "{} (missing {exe})",
            game_dir.display()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedFile {
    /// Path relative to the game dir, Unix separators.
    pub rel: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallManifest {
    pub version: String,
    pub files: Vec<OwnedFile>,
}

impl InstallManifest {
    /// One line per file: `<sha256>  <rel>`. Final line holds the version.
    pub fn encode(&self) -> String {
        let mut out = String::new();
        for file in &self.files {
            out.push_str(&file.sha256);
            out.push_str("  ");
            out.push_str(&file.rel);
            out.push('\n');
        }
        out.push_str("version ");
        out.push_str(&self.version);
        out.push('\n');
        out
    }

    pub fn decode(text: &str) -> Result<InstallManifest, Error> {
        let mut files = Vec::new();
        let mut version = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("version ") {
                if version.is_some() {
                    return Err(Error::Manifest("duplicate version line".into()));
                }
                if rest.is_empty() || rest.len() > 32 {
                    return Err(Error::Manifest("bad version".into()));
                }
                version = Some(rest.to_owned());
                continue;
            }
            let (hash, rel) = line
                .split_once("  ")
                .ok_or_else(|| Error::Manifest(format!("bad manifest line: {line}")))?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::Manifest("bad sha256 in manifest".into()));
            }
            // Rejects escapes up front so a hostile manifest cannot be
            // laundered through repair/uninstall.
            if join_owned(Path::new("."), rel).is_err() {
                return Err(Error::Manifest(format!("unsafe path: {rel}")));
            }
            files.push(OwnedFile {
                rel: rel.to_owned(),
                sha256: hash.to_owned(),
            });
        }
        let Some(version) = version else {
            return Err(Error::Manifest("manifest has no version line".into()));
        };
        Ok(InstallManifest { version, files })
    }
}

/// Currently installed manifest, if any.
pub fn installed_manifest(game_dir: &Path) -> Result<Option<InstallManifest>, Error> {
    match fs::read_to_string(game_dir.join(MANIFEST_FILE)) {
        Ok(text) => Ok(Some(InstallManifest::decode(&text)?)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::Io(error)),
    }
}

pub struct InstallPlan {
    pub vpk_rel: String,
    pub manifest: InstallManifest,
}

/// Install (or upgrade) a versioned `.vpk`: writes
/// `left4dead2/addons/<vpk>`, creates the owned `ems/` dir, records the
/// manifest. Idempotent: reinstalling the same version repairs in place.
/// Removes a previous version's `.vpk` only when upgrading.
pub fn install(game_dir: &Path, vpk_bytes: &[u8], version: &str) -> Result<InstallPlan, Error> {
    validate_game_dir(game_dir)?;
    if version.is_empty() || version.len() > 32 {
        return Err(Error::Manifest("bad version".into()));
    }
    let vpk_rel = format!("left4dead2/addons/{ADDON_FILE_PREFIX}{version}{ADDON_FILE_SUFFIX}");
    let previous = installed_manifest(game_dir)?;

    fs::create_dir_all(game_dir.join("left4dead2/addons"))?;
    fs::create_dir_all(game_dir.join(EMS_DIR))?;
    // Keep existing installation manifests in their original location.
    fs::create_dir_all(game_dir.join("ems/tiktools_l4d2"))?;

    // Remove the previous version's vpk on upgrade; never touch other mods.
    if let Some(old) = &previous {
        for file in &old.files {
            if file.rel != vpk_rel && file.rel.ends_with(ADDON_FILE_SUFFIX) {
                let _ = fs::remove_file(game_dir.join(&file.rel));
            }
        }
    }

    let target = join_owned(game_dir, &vpk_rel)?;
    // Atomic write: temp + rename.
    let tmp = target.with_extension("vpk.tmp");
    fs::write(&tmp, vpk_bytes)?;
    fs::rename(&tmp, &target)?;

    let manifest = InstallManifest {
        version: version.to_owned(),
        files: vec![OwnedFile {
            rel: vpk_rel.clone(),
            sha256: sha256_hex(vpk_bytes),
        }],
    };
    fs::write(game_dir.join(MANIFEST_FILE), manifest.encode())?;
    Ok(InstallPlan { vpk_rel, manifest })
}

/// Verify every owned file still matches its checksum.
pub fn verify(game_dir: &Path) -> Result<Vec<String>, Error> {
    let Some(manifest) = installed_manifest(game_dir)? else {
        return Err(Error::Manifest("not installed".into()));
    };
    let mut bad = Vec::new();
    for file in &manifest.files {
        let path = join_owned(game_dir, &file.rel)?;
        match fs::read(&path) {
            Ok(bytes) if sha256_hex(&bytes) == file.sha256 => {}
            _ => bad.push(file.rel.clone()),
        }
    }
    Ok(bad)
}

/// Repair = reinstall the same bytes; kept separate so UIs can label the
/// action honestly and tests can pin the behavior.
pub fn repair(game_dir: &Path, vpk_bytes: &[u8], version: &str) -> Result<InstallPlan, Error> {
    install(game_dir, vpk_bytes, version)
}

/// Uninstall removes ONLY files listed in the owned manifest (plus the
/// manifest itself and the owned `ems/` dir when empty). Unrelated mods,
/// shared configs and other `.vpk` files are never touched.
pub fn uninstall(game_dir: &Path) -> Result<Vec<String>, Error> {
    let Some(manifest) = installed_manifest(game_dir)? else {
        return Err(Error::Manifest("not installed".into()));
    };
    let mut removed = Vec::new();
    for file in &manifest.files {
        let path = join_owned(game_dir, &file.rel)?;
        match fs::remove_file(&path) {
            Ok(()) => removed.push(file.rel.clone()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Io(error)),
        }
    }
    let _ = fs::remove_file(game_dir.join(MANIFEST_FILE));
    // Remove the owned ems dir only when it holds no foreign files.
    let ems = game_dir.join(EMS_DIR);
    if ems.is_dir()
        && fs::read_dir(&ems)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
    {
        let _ = fs::remove_dir(&ems);
    }
    Ok(removed)
}

/// Best-effort running-game detection via the heartbeat freshness marker
/// the addon maintains. Returns true when a live tick was seen recently;
/// callers should ask for a restart only when an update requires it.
pub fn game_running(game_dir: &Path, max_age_secs: u64) -> bool {
    let path = game_dir.join(EMS_DIR).join("heartbeat.txt");
    let Ok(meta) = fs::metadata(&path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    std::time::SystemTime::now()
        .duration_since(modified)
        .map(|age| age.as_secs() <= max_age_secs)
        .unwrap_or(false)
}

pub fn owned_inventory(version: &str, vpk_sha256: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            format!("left4dead2/addons/{ADDON_FILE_PREFIX}{version}{ADDON_FILE_SUFFIX}"),
            vpk_sha256.to_owned(),
        ),
        (MANIFEST_FILE.to_owned(), String::new()),
    ])
}

// --- Workshop mod-conflict handling --------------------------------------
//
// A workshop addon that overrides a stock consistency-critical file (e.g.
// `scripts/weapon_rifle_sg552.txt`, measured live 2026-10-29: client kicked
// with "Server is enforcing consistency for this file") cannot join a stock
// dedicated server, with any RCON password or TikTools version. Two durable
// fixes exist and this module implements the client-side one:
//
// - Own server: set `sv_consistency 0` (operator opt-in; private servers).
//   Zero client action, any mod works. No code needed beyond the setting.
// - Someone else's stock server: detect the colliding addon(s) and
//   quarantine them with one click; restore them afterwards. Implemented
//   below: detection is automatic, quarantine/restore are reversible and
//   never touch files outside `left4dead2/addons/` or files TikTools did
//   not quarantine itself.
//
// The future local-host (`local_ipc`) transport has no client/server file
// comparison at all, so it is inherently mod-agnostic.

pub const QUARANTINE_DIR: &str = "ems/tiktools_l4d2/quarantine";
pub const QUARANTINE_MANIFEST: &str = "ems/tiktools_l4d2/quarantine.manifest";

/// Stock paths whose override triggers engine consistency kicks
/// (`scripts/*.txt`: weapon scripts and siblings). Skins, sounds and models
/// do not kick with `sv_pure 0` and are deliberately NOT flagged.
fn is_consistency_critical(logical_path: &str) -> bool {
    logical_path.starts_with("scripts/") && logical_path.ends_with(".txt")
}

/// How seriously to take a stock-script override.
///
/// Only the weapon-script family is a *measured* kick trigger (live:
/// `scripts/weapon_rifle_sg552.txt` vs stock server, 2026-10-29). Other
/// stock `scripts/*.txt` overrides are reported as suspicious: they ride
/// the same engine consistency path, but quarantining HUD-only mods on
/// sight would be overreach — surface them, let the user decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// `scripts/weapon_*.txt`: measured consistency kick.
    WeaponScript,
    /// Other stock `scripts/*.txt`: suspicious, user decides.
    StockScript,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonConflict {
    /// Addon file relative to the game dir, e.g.
    /// `left4dead2/addons/workshop/script-override-mod.vpk`.
    pub addon_rel: String,
    /// Stock path it overrides, e.g. `scripts/weapon_rifle_sg552.txt`.
    pub stock_path: String,
    pub kind: ConflictKind,
}

fn classify_conflict(stock_path: &str) -> ConflictKind {
    let file = stock_path.rsplit('/').next().unwrap_or(stock_path);
    if file.starts_with("weapon_") {
        ConflictKind::WeaponScript
    } else {
        ConflictKind::StockScript
    }
}

fn read_vpk_tree_paths(bytes: &[u8], source: &str) -> Result<Vec<String>, Error> {
    // Tolerant listing: stock game archives use multi-archive/preload
    // entries. Only paths are needed for scans; no data is read.
    l4d2api_pack::read_tree_paths(bytes)
        .map_err(|error| Error::Manifest(format!("{source}: invalid vpk ({error})")))
}

/// All stock logical paths from every `*_dir.vpk` under `left4dead2/`.
pub fn stock_paths(game_dir: &Path) -> Result<BTreeSet<String>, Error> {
    let left4dead2 = game_dir.join("left4dead2");
    let mut entries = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&left4dead2)
        .map_err(|_| Error::NotAGameDir(game_dir.display().to_string()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_dir.vpk"))
        })
        .collect();
    dirs.sort();
    for dir in dirs {
        let bytes = fs::read(&dir)?;
        entries.extend(read_vpk_tree_paths(&bytes, &dir.to_string_lossy())?);
    }
    Ok(entries.into_iter().collect())
}

fn addon_vpks(game_dir: &Path) -> Result<Vec<(String, PathBuf)>, Error> {
    let mut found = Vec::new();
    for sub in ["left4dead2/addons", "left4dead2/addons/workshop"] {
        let Ok(read_dir) = fs::read_dir(game_dir.join(sub)) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = read_dir
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".vpk"))
            })
            .collect();
        paths.sort();
        for path in paths {
            let rel = path
                .strip_prefix(game_dir)
                .map_err(|_| Error::PathEscape(path.display().to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            found.push((rel, path));
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Detects installed addons overriding stock consistency-critical files.
/// Deterministic: sorted by `(addon, stock path)`. Skins/sounds/models are
/// never flagged — only `scripts/*.txt` overrides kick clients.
pub fn scan_addon_conflicts(game_dir: &Path) -> Result<Vec<AddonConflict>, Error> {
    let stock = stock_paths(game_dir)?;
    let mut conflicts = Vec::new();
    for (rel, path) in addon_vpks(game_dir)? {
        let bytes = fs::read(&path)?;
        let tree = match read_vpk_tree_paths(&bytes, &rel) {
            Ok(tree) => tree,
            // An unreadable addon is the game's problem, not a conflict.
            Err(_) => continue,
        };
        for entry_path in tree {
            if is_consistency_critical(&entry_path) && stock.contains(&entry_path) {
                conflicts.push(AddonConflict {
                    addon_rel: rel.clone(),
                    stock_path: entry_path.clone(),
                    kind: classify_conflict(&entry_path),
                });
            }
        }
    }
    conflicts.sort_by(|a, b| (&a.addon_rel, &a.stock_path).cmp(&(&b.addon_rel, &b.stock_path)));
    conflicts.dedup();
    Ok(conflicts)
}

fn quarantine_backup_name(addon_rel: &str) -> String {
    addon_rel.replace(['/', '\\'], "!")
}

/// Moves a conflicting addon out of the game load path into the owned
/// quarantine dir. Only files under `left4dead2/addons/` are eligible;
/// anything else is refused. Reversible via [`restore_quarantined`].
pub fn quarantine_addon(game_dir: &Path, addon_rel: &str) -> Result<String, Error> {
    if !(addon_rel.starts_with("left4dead2/addons/") && addon_rel.ends_with(".vpk")) {
        return Err(Error::PathEscape(addon_rel.to_owned()));
    }
    let source = join_owned(game_dir, addon_rel)?;
    if !source.is_file() {
        return Err(Error::Manifest(format!("not installed: {addon_rel}")));
    }
    let backup = quarantine_backup_name(addon_rel);
    let target = join_owned(game_dir, &format!("{QUARANTINE_DIR}/{backup}"))?;
    fs::create_dir_all(game_dir.join(QUARANTINE_DIR))?;
    fs::rename(&source, &target)?;

    let manifest_path = game_dir.join(QUARANTINE_MANIFEST);
    let mut lines: Vec<String> = fs::read_to_string(&manifest_path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .filter(|line| !line.is_empty() && line != "version 1")
        .collect();
    let record = format!("{backup}  {addon_rel}");
    if !lines.contains(&record) {
        lines.push(record);
    }
    let mut manifest = String::from("version 1\n");
    for line in lines {
        manifest.push_str(&line);
        manifest.push('\n');
    }
    fs::write(&manifest_path, manifest)?;
    Ok(backup)
}

fn read_quarantine_manifest(game_dir: &Path) -> Result<Vec<(String, String)>, Error> {
    let text = match fs::read_to_string(game_dir.join(QUARANTINE_MANIFEST)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Error::Io(error)),
    };
    let mut records = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if line == "version 1" {
            continue;
        }
        let (backup, original) = line
            .split_once("  ")
            .ok_or_else(|| Error::Manifest(format!("bad quarantine line: {line}")))?;
        if join_owned(Path::new("."), original).is_err() || backup.contains(['/', '\\']) {
            return Err(Error::Manifest(format!("unsafe quarantine entry: {line}")));
        }
        records.push((backup.to_owned(), original.to_owned()));
    }
    Ok(records)
}

/// Currently quarantined addons as `(backup, original_rel)` records.
pub fn quarantined_addons(game_dir: &Path) -> Result<Vec<(String, String)>, Error> {
    read_quarantine_manifest(game_dir)
}

/// Restores every quarantined addon to its original path. Entries whose
/// backup went missing are dropped (already restored by hand). Removes the
/// manifest when empty. Returns the restored original rel paths.
pub fn restore_quarantined(game_dir: &Path) -> Result<Vec<String>, Error> {
    let mut restored = Vec::new();
    for (backup, original) in read_quarantine_manifest(game_dir)? {
        let from = join_owned(game_dir, &format!("{QUARANTINE_DIR}/{backup}"))?;
        if !from.is_file() {
            continue;
        }
        let to = join_owned(game_dir, &original)?;
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&from, &to)?;
        restored.push(original);
    }
    let remaining = read_quarantine_manifest(game_dir)?
        .into_iter()
        .filter(|(_, original)| !restored.contains(original))
        .collect::<Vec<_>>();
    if remaining.is_empty() {
        let _ = fs::remove_file(game_dir.join(QUARANTINE_MANIFEST));
        let quarantine = game_dir.join(QUARANTINE_DIR);
        if quarantine.is_dir()
            && fs::read_dir(&quarantine)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
        {
            let _ = fs::remove_dir(&quarantine);
        }
    } else {
        let mut manifest = String::from("version 1\n");
        for (backup, original) in &remaining {
            manifest.push_str(&format!("{backup}  {original}\n"));
        }
        fs::write(game_dir.join(QUARANTINE_MANIFEST), manifest)?;
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn test_game_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("l4d2api-installer-{name}-{nanos}"));
        fs::create_dir_all(dir.join("left4dead2/addons")).unwrap();
        fs::write(dir.join("left4dead2.exe"), b"fake exe").unwrap();
        // An unrelated mod that must survive everything we do.
        fs::write(dir.join("left4dead2/addons/someone-else.vpk"), b"foreign").unwrap();
        dir
    }

    #[test]
    fn discovers_secondary_library() {
        let steam = std::env::temp_dir().join("l4d2api-steam-test");
        let _ = fs::remove_dir_all(&steam);
        let secondary = steam.join("Library2");
        fs::create_dir_all(secondary.join("steamapps/common/Left 4 Dead 2/left4dead2")).unwrap();
        fs::write(
            secondary.join("steamapps/common/Left 4 Dead 2/left4dead2.exe"),
            b"exe",
        )
        .unwrap();
        fs::write(
            secondary.join("steamapps/appmanifest_550.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"550\"\n\t\"installdir\"\t\t\"Left 4 Dead 2\"\n}\n",
        )
        .unwrap();
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
                steam.display(),
                secondary.display()
            ),
        )
        .unwrap();
        let installs = find_l4d2_installs(&[steam.clone()]);
        assert_eq!(installs.len(), 1);
        assert!(installs[0].ends_with("Left 4 Dead 2"));
        let _ = fs::remove_dir_all(&steam);
    }

    #[test]
    fn install_verify_uninstall_keeps_foreign_mods() {
        let game = test_game_dir("lifecycle");
        let vpk = b"fake vpk bytes v1";
        install(&game, vpk, "0.1.0").unwrap();
        assert!(verify(&game).unwrap().is_empty());
        // Corrupt the installed file: verify must report it.
        fs::write(
            game.join("left4dead2/addons/tiktools-l4d2api-0.1.0.vpk"),
            b"tampered",
        )
        .unwrap();
        assert_eq!(verify(&game).unwrap().len(), 1);
        repair(&game, vpk, "0.1.0").unwrap();
        assert!(verify(&game).unwrap().is_empty());
        let removed = uninstall(&game).unwrap();
        assert_eq!(removed.len(), 1);
        // Foreign mod survives.
        assert!(game.join("left4dead2/addons/someone-else.vpk").is_file());
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn rejects_non_game_folders_and_escapes() {
        let dir = std::env::temp_dir().join("l4d2api-nogame");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            install(&dir, b"x", "0.1.0"),
            Err(Error::NotAGameDir(_))
        ));
        assert!(join_owned(Path::new("/game"), "../evil.vpk").is_err());
        assert!(join_owned(Path::new("/game"), "a/../../b").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unicode_paths_work() {
        let mut game = std::env::temp_dir();
        game.push("l4d2api-Juego-üñí-游戏");
        let _ = fs::remove_dir_all(&game);
        fs::create_dir_all(game.join("left4dead2/addons")).unwrap();
        fs::write(game.join("left4dead2.exe"), b"exe").unwrap();
        install(&game, b"vpk", "0.1.0").unwrap();
        assert!(verify(&game).unwrap().is_empty());
        let _ = fs::remove_dir_all(&game);
    }

    fn write_stock_pack(left4dead2: &Path) {
        // Minimal stock tree: consistency-critical scripts plus a non-script
        // stock file a benign mod may legitimately override. (The packer
        // only allows the archive's own text types, which is exactly the
        // policy boundary under test: only scripts/*.txt counts.)
        let files = vec![
            l4d2api_pack::ArchiveFile {
                path: "scripts/weapon_rifle_sg552.txt".into(),
                data: b"stock sg552".to_vec(),
            },
            l4d2api_pack::ArchiveFile {
                path: "scripts/weapon_rifle_ak47.txt".into(),
                data: b"stock ak47".to_vec(),
            },
            l4d2api_pack::ArchiveFile {
                path: "sound/weapons/custom.txt".into(),
                data: b"stock sound script".to_vec(),
            },
            l4d2api_pack::ArchiveFile {
                path: "scripts/hud/damagecutout.txt".into(),
                data: b"stock hud".to_vec(),
            },
        ];
        let bytes = l4d2api_pack::build_vpk(&files).unwrap();
        fs::write(left4dead2.join("pak01_dir.vpk"), bytes).unwrap();
    }

    fn write_addon(path: &Path, files: Vec<l4d2api_pack::ArchiveFile>) {
        let bytes = l4d2api_pack::build_vpk(&files).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn conflict_game_dir(name: &str) -> PathBuf {
        let game = test_game_dir(name);
        let left4dead2 = game.join("left4dead2");
        write_stock_pack(&left4dead2);
        fs::create_dir_all(left4dead2.join("addons/workshop")).unwrap();
        // Script-overriding addon (the measured SG552 case) plus a
        // suspicious-but-unmeasured HUD script override.
        write_addon(
            &left4dead2.join("addons/workshop/script-override-mod.vpk"),
            vec![
                l4d2api_pack::ArchiveFile {
                    path: "scripts/weapon_rifle_sg552.txt".into(),
                    data: b"modded sg552".to_vec(),
                },
                l4d2api_pack::ArchiveFile {
                    path: "scripts/hud/damagecutout.txt".into(),
                    data: b"modded hud".to_vec(),
                },
            ],
        );
        // Benign sound-only addon: must never be flagged.
        write_addon(
            &left4dead2.join("addons/cool-skin.vpk"),
            vec![l4d2api_pack::ArchiveFile {
                path: "sound/weapons/custom.txt".into(),
                data: b"custom sound script".to_vec(),
            }],
        );
        game
    }

    #[test]
    fn detects_only_script_overrides() {
        let game = conflict_game_dir("conflicts");
        let conflicts = scan_addon_conflicts(&game).unwrap();
        assert_eq!(
            conflicts,
            vec![
                AddonConflict {
                    addon_rel: "left4dead2/addons/workshop/script-override-mod.vpk".into(),
                    stock_path: "scripts/hud/damagecutout.txt".into(),
                    kind: ConflictKind::StockScript,
                },
                AddonConflict {
                    addon_rel: "left4dead2/addons/workshop/script-override-mod.vpk".into(),
                    stock_path: "scripts/weapon_rifle_sg552.txt".into(),
                    kind: ConflictKind::WeaponScript,
                },
            ]
        );
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn quarantine_restore_roundtrip() {
        let game = conflict_game_dir("quarantine");
        let addon = "left4dead2/addons/workshop/script-override-mod.vpk";
        let before = fs::read(game.join(addon)).unwrap();

        quarantine_addon(&game, addon).unwrap();
        assert!(!game.join(addon).exists());
        // Skin mod untouched; conflict gone from scan.
        assert!(game.join("left4dead2/addons/cool-skin.vpk").is_file());
        assert!(scan_addon_conflicts(&game).unwrap().is_empty());
        assert_eq!(quarantined_addons(&game).unwrap().len(), 1);

        let restored = restore_quarantined(&game).unwrap();
        assert_eq!(restored, vec![addon.to_owned()]);
        assert_eq!(fs::read(game.join(addon)).unwrap(), before);
        assert!(quarantined_addons(&game).unwrap().is_empty());
        assert_eq!(scan_addon_conflicts(&game).unwrap().len(), 2);
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn quarantine_refuses_out_of_scope_paths() {
        let game = conflict_game_dir("qscope");
        assert!(quarantine_addon(&game, "left4dead2/cfg/server.cfg").is_err());
        assert!(quarantine_addon(&game, "left4dead2/addons/../../evil.vpk").is_err());
        assert!(quarantine_addon(&game, "left4dead2/addons/missing.vpk").is_err());
        let _ = fs::remove_dir_all(&game);
    }

    /// Real-world shape check: every reported conflict must name a stock
    /// path present in the game VPKs (self-consistency, no hardcoded ids).
    /// Requires a local Steam L4D2 install; ignored in CI.
    #[test]
    #[ignore = "requires a local Steam L4D2 install"]
    fn conflicts_reference_real_stock_paths() {
        let home = std::env::var("HOME").expect("HOME must be set");
        let game = PathBuf::from(home).join(".local/share/Steam/steamapps/common/Left 4 Dead 2");
        let stock = stock_paths(&game).expect("game VPKs must parse");
        assert!(!stock.is_empty());
        let conflicts = scan_addon_conflicts(&game).expect("scan must run");
        eprintln!("real install: {} stock paths", stock.len());
        for conflict in conflicts.iter().take(10) {
            eprintln!(
                "real conflict: {} overrides {}",
                conflict.addon_rel, conflict.stock_path
            );
        }
        for conflict in &conflicts {
            assert!(
                stock.contains(&conflict.stock_path),
                "conflict {:?} names a non-stock path",
                conflict
            );
            assert!(conflict.addon_rel.starts_with("left4dead2/addons/"));
        }
    }
}
