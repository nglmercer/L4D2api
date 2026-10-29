//! `l4d2api-pack`: deterministic VPK v1 packer for the addon.
//!
//! Writes a single-archive Valve VPK (magic `0x55AA1234`, version 1):
//! header, sorted file tree, then concatenated file data. Building the same
//! `addon/` source twice yields byte-identical output. `validate_vpk`
//! re-parses the archive and enforces the addon contract (required game
//! paths, no absolute/parent paths, no proprietary or executable payloads).
//!
//! Note: the exact root-directory tree encoding is validated for internal
//! consistency here; whether a Windows L4D2 client mounts the archive is
//! part of the live gate in `docs/WINDOWS_TESTING.md`, not claimed here.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

pub const VPK_MAGIC: u32 = 0x55AA_1234;
pub const VPK_VERSION: u32 = 1;
const ENTRY_TERMINATOR: u16 = 0xFFFF;
/// Archive index that means "the data is embedded in this VPK file". Any
/// other value makes the engine look for a sibling `<name>_<index:03>.vpk`
/// chunk; a single-file addon must use `0x7FFF` or it never mounts (L4D2
/// then loops forever on the missing chunk during startup).
pub const VPK_SINGLE_ARCHIVE: u16 = 0x7FFF;

/// Extensions allowed inside the addon archive. The addon ships only its
/// own scripts and metadata: no binaries, no Valve assets, no audio blobs.
const ALLOWED_EXTENSIONS: [&str; 2] = ["txt", "nut"];

/// Logical paths the game requires to load the addon.
const REQUIRED_PATHS: [&str; 3] = [
    "addoninfo.txt",
    "scripts/vscripts/director_base_addon.nut",
    "scripts/vscripts/tiktools_l4d2/bootstrap.nut",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("pack error: {0}")]
    Pack(String),
    #[error("invalid vpk: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    /// Unix-style path relative to the addon root, e.g. `scripts/vscripts/x.nut`.
    pub path: String,
    pub data: Vec<u8>,
}

/// CRC-32 (IEEE 802.3), bit-by-bit implementation to avoid extra deps.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = if crc & 1 == 1 { 0xEDB8_8320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn split_vpk_path(path: &str) -> Result<(String, String, String), Error> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains("..") {
        return Err(Error::Pack(format!("unsafe archive path: {path}")));
    }
    let (dir, file) = match path.rfind('/') {
        Some(index) => (path[..index].to_owned(), path[index + 1..].to_owned()),
        None => (String::new(), path.to_owned()),
    };
    if file.is_empty() || file.starts_with('.') {
        return Err(Error::Pack(format!("bad file name: {path}")));
    }
    let extension = match file.rfind('.') {
        Some(index) if index > 0 => file[index + 1..].to_ascii_lowercase(),
        _ => {
            return Err(Error::Pack(format!("extensionless file: {path}")));
        }
    };
    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(Error::Pack(format!(
            "extension .{extension} is not allowed in the addon archive: {path}"
        )));
    }
    Ok((extension, dir, file))
}

fn collect_dir(base: &Path, rel: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<(), Error> {
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(base.join(rel))? {
        entries.push(entry?.path());
    }
    entries.sort();
    for path in entries {
        let file_type = fs::symlink_metadata(&path)?.file_type();
        if file_type.is_symlink() {
            return Err(Error::Pack(format!(
                "symlinks are not packed: {}",
                path.display()
            )));
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Pack(format!("non-utf8 file name: {}", path.display())))?;
        if name.starts_with('.') {
            continue;
        }
        let child = rel.join(name);
        if file_type.is_dir() {
            collect_dir(base, &child, out)?;
        } else {
            let mut rel_unix = child.to_string_lossy().replace('\\', "/");
            if rel_unix.starts_with("./") {
                rel_unix = rel_unix[3..].to_owned();
            }
            // Validates extension and safety now, so packing fails fast.
            split_vpk_path(&rel_unix)?;
            out.insert(rel_unix, fs::read(&path)?);
        }
    }
    Ok(())
}

/// Collect `addon/` source files in deterministic (sorted) order.
pub fn collect_files(addon_dir: &Path) -> Result<Vec<ArchiveFile>, Error> {
    let mut map = BTreeMap::new();
    collect_dir(addon_dir, Path::new(""), &mut map)?;
    if map.is_empty() {
        return Err(Error::Pack("addon directory holds no files".into()));
    }
    Ok(map
        .into_iter()
        .map(|(path, data)| ArchiveFile { path, data })
        .collect())
}

fn push_cstr(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(0);
}

/// Build single-archive VPK v1 bytes. Deterministic for identical input.
pub fn build_vpk(files: &[ArchiveFile]) -> Result<Vec<u8>, Error> {
    // Group by (extension, directory); BTreeMap iteration is sorted and
    // each member list is sorted by full path for determinism.
    let mut by_group: BTreeMap<(String, String), Vec<&ArchiveFile>> = BTreeMap::new();
    for file in files {
        let (extension, dir, _name) = split_vpk_path(&file.path)?;
        by_group.entry((extension, dir)).or_default().push(file);
    }
    let mut group_list: Vec<((String, String), Vec<&ArchiveFile>)> = by_group.into_iter().collect();
    let mut tree: Vec<u8> = Vec::new();
    for (_, members) in &mut group_list {
        members.sort_by(|a, b| a.path.cmp(&b.path));
    }

    // Entry offsets depend on the final tree size; first compute each
    // entry's data offset relative to the data section start.
    let mut data_offsets: BTreeMap<&str, u32> = BTreeMap::new();
    let mut cursor: u32 = 0;
    for (_, members) in &group_list {
        for file in members {
            data_offsets.insert(file.path.as_str(), cursor);
            cursor = cursor
                .checked_add(file.data.len() as u32)
                .ok_or_else(|| Error::Pack("archive exceeds 4 GiB".into()))?;
        }
    }

    // Emit the tree with an explicit state machine:
    // ext { dir { file* "" }* "" }* "".
    // A root-level directory is encoded as " " (single space): an empty
    // dir string would be indistinguishable from the end-of-dirs sentinel.
    // (Whether a Windows L4D2 client mounts this encoding is live-gate
    // item 6 in docs/WINDOWS_TESTING.md, tracked in docs/RESEARCH.md.)
    let mut tree_entries: Vec<(u32, u32)> = Vec::new(); // (tree_pos_of_offset, data_offset)
    let mut cur_ext: Option<&str> = None;
    for ((extension, dir), members) in &group_list {
        if cur_ext != Some(extension.as_str()) {
            if cur_ext.is_some() {
                push_cstr(&mut tree, ""); // close previous dir's files
                push_cstr(&mut tree, ""); // close previous ext's dirs
            }
            push_cstr(&mut tree, extension);
            cur_ext = Some(extension);
        } else {
            push_cstr(&mut tree, ""); // close previous dir's files
        }
        if dir.is_empty() {
            push_cstr(&mut tree, " ");
        } else {
            push_cstr(&mut tree, dir);
        }
        for file in members {
            // Tree file names exclude the extension (it is the group key).
            let full_name = file.path.rsplit('/').next().unwrap_or(&file.path);
            let stem = match full_name.rfind('.') {
                Some(index) => &full_name[..index],
                None => full_name,
            };
            push_cstr(&mut tree, stem);
            tree.extend_from_slice(&crc32(&file.data).to_le_bytes());
            tree.extend_from_slice(&0u16.to_le_bytes()); // preload bytes
            tree.extend_from_slice(&VPK_SINGLE_ARCHIVE.to_le_bytes()); // data in this file
            tree_entries.push((tree.len() as u32, data_offsets[file.path.as_str()]));
            tree.extend_from_slice(&0u32.to_le_bytes()); // offset (patched)
            tree.extend_from_slice(&(file.data.len() as u32).to_le_bytes());
            tree.extend_from_slice(&ENTRY_TERMINATOR.to_le_bytes());
        }
    }
    if cur_ext.is_some() {
        push_cstr(&mut tree, ""); // close last dir's files
        push_cstr(&mut tree, ""); // close last ext's dirs
    }
    push_cstr(&mut tree, ""); // end of extensions

    let data_start = 12u32
        .checked_add(tree.len() as u32)
        .ok_or_else(|| Error::Pack("archive exceeds 4 GiB".into()))?;
    // Entry offsets are relative to the start of the data section (the byte
    // right after the header + tree), exactly like Valve's own VPKs. Writing
    // absolute offsets makes the engine read every file shifted by
    // `data_start` bytes (and past EOF for the last one).
    for (pos, relative) in tree_entries {
        let at = pos as usize;
        tree[at..at + 4].copy_from_slice(&relative.to_le_bytes());
    }

    let mut out = Vec::with_capacity(data_start as usize + cursor as usize);
    out.extend_from_slice(&VPK_MAGIC.to_le_bytes());
    out.extend_from_slice(&VPK_VERSION.to_le_bytes());
    out.extend_from_slice(&(tree.len() as u32).to_le_bytes());
    out.extend_from_slice(&tree);
    for (_, members) in &group_list {
        for file in members {
            out.extend_from_slice(&file.data);
        }
    }
    debug_assert_eq!(out.len() as u32, data_start + cursor);
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: String,
    pub crc: u32,
    /// Offset relative to the start of the data section (header + tree size).
    pub offset: u32,
    pub length: u32,
}

fn read_cstr(data: &[u8], pos: &mut usize) -> Result<String, Error> {
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= data.len() {
        return Err(Error::Invalid("truncated string in vpk tree".into()));
    }
    let text = std::str::from_utf8(&data[start..*pos])
        .map_err(|_| Error::Invalid("non-utf8 string in vpk tree".into()))?;
    *pos += 1;
    Ok(text.to_owned())
}

fn read_u16(data: &[u8], pos: &mut usize) -> Result<u16, Error> {
    if *pos + 2 > data.len() {
        return Err(Error::Invalid("truncated entry in vpk tree".into()));
    }
    let value = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
    *pos += 2;
    Ok(value)
}

fn read_u32(data: &[u8], pos: &mut usize) -> Result<u32, Error> {
    if *pos + 4 > data.len() {
        return Err(Error::Invalid("truncated entry in vpk tree".into()));
    }
    let value = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
    *pos += 4;
    Ok(value)
}

struct RawEntry {
    path: String,
    crc: u32,
    preload: u16,
    archive: u16,
    offset: u32,
    length: u32,
}

/// Walk the tree without judging entry kinds. Multi-archive and preload
/// entries (normal in stock game VPKs) are reported as-is; callers that
/// need data decide what they support.
fn walk_tree(bytes: &[u8]) -> Result<Vec<RawEntry>, Error> {
    if bytes.len() < 12 {
        return Err(Error::Invalid("file smaller than vpk header".into()));
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let tree_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if magic != VPK_MAGIC {
        return Err(Error::Invalid("bad vpk magic".into()));
    }
    if version != VPK_VERSION {
        return Err(Error::Invalid(format!("unsupported vpk version {version}")));
    }
    if bytes.len() < 12 + tree_size {
        return Err(Error::Invalid("vpk tree overruns file".into()));
    }
    let tree = &bytes[12..12 + tree_size];
    let mut pos = 0usize;
    let mut entries = Vec::new();
    loop {
        let extension = read_cstr(tree, &mut pos)?;
        if extension.is_empty() {
            break;
        }
        loop {
            let dir = read_cstr(tree, &mut pos)?;
            if dir.is_empty() {
                break;
            }
            loop {
                let name = read_cstr(tree, &mut pos)?;
                if name.is_empty() {
                    break;
                }
                let crc = read_u32(tree, &mut pos)?;
                let preload = read_u16(tree, &mut pos)?;
                let archive = read_u16(tree, &mut pos)?;
                let offset = read_u32(tree, &mut pos)?;
                let length = read_u32(tree, &mut pos)?;
                let terminator = read_u16(tree, &mut pos)?;
                if terminator != ENTRY_TERMINATOR {
                    return Err(Error::Invalid("bad vpk entry terminator".into()));
                }
                if preload as usize > tree.len().saturating_sub(pos) {
                    return Err(Error::Invalid("preload overruns vpk tree".into()));
                }
                pos += preload as usize;
                // " " is the on-disk encoding of the archive root.
                let path = if dir == " " {
                    format!("{name}.{extension}")
                } else {
                    format!("{dir}/{name}.{extension}")
                };
                entries.push(RawEntry {
                    path,
                    crc,
                    preload,
                    archive,
                    offset,
                    length,
                });
            }
        }
    }
    Ok(entries)
}

/// Parse the tree; returns entries with logical `dir/name.ext` paths.
/// Only single-archive, preload-free entries are supported (the addon
/// contract); stock game archives need [`read_tree_paths`].
pub fn read_tree(bytes: &[u8]) -> Result<Vec<TreeEntry>, Error> {
    let mut entries = Vec::new();
    for raw in walk_tree(bytes)? {
        if raw.preload != 0 || raw.archive != VPK_SINGLE_ARCHIVE {
            return Err(Error::Invalid(
                "multi-archive or preload entries are not supported".into(),
            ));
        }
        entries.push(TreeEntry {
            path: raw.path,
            crc: raw.crc,
            offset: raw.offset,
            length: raw.length,
        });
    }
    Ok(entries)
}

/// List every logical path in a VPK tree, tolerating the multi-archive and
/// preload entries stock game archives use. No data is read. Used for
/// install-time scans (stock manifests, addon conflict detection), never
/// for packing validation.
pub fn read_tree_paths(bytes: &[u8]) -> Result<Vec<String>, Error> {
    Ok(walk_tree(bytes)?.into_iter().map(|raw| raw.path).collect())
}

/// Validate archive contents against the addon contract.
pub fn validate_vpk(bytes: &[u8]) -> Result<Vec<TreeEntry>, Error> {
    let entries = read_tree(bytes)?;
    if entries.is_empty() {
        return Err(Error::Invalid("vpk holds no files".into()));
    }
    if bytes.len() < 12 {
        return Err(Error::Invalid("file smaller than vpk header".into()));
    }
    let tree_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let data_start = 12usize
        .checked_add(tree_size)
        .ok_or_else(|| Error::Invalid("vpk tree size overflow".into()))?;
    let mut seen = std::collections::BTreeSet::new();
    for entry in &entries {
        if !seen.insert(entry.path.clone()) {
            return Err(Error::Invalid(format!("duplicate path {}", entry.path)));
        }
        // Reuse the safety policy: rejects escapes and non-shipped types.
        split_vpk_path(&entry.path).map_err(|e| Error::Invalid(e.to_string()))?;
        let start = data_start
            .checked_add(entry.offset as usize)
            .ok_or_else(|| Error::Invalid("entry range overflow".into()))?;
        let end = start
            .checked_add(entry.length as usize)
            .ok_or_else(|| Error::Invalid("entry range overflow".into()))?;
        if end > bytes.len() {
            return Err(Error::Invalid(format!(
                "entry {} overruns file",
                entry.path
            )));
        }
        let data = &bytes[start..end];
        if crc32(data) != entry.crc {
            return Err(Error::Invalid(format!("crc mismatch for {}", entry.path)));
        }
    }
    for required in REQUIRED_PATHS {
        if !seen.contains(required) {
            return Err(Error::Invalid(format!("missing required {required}")));
        }
    }
    Ok(entries)
}

pub struct Release {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub entries: Vec<TreeEntry>,
    pub manifest: String,
}

/// Pack `addon_dir` and render the release manifest:
/// `tiktools-l4d2api-<version>.vpk`, its SHA-256, and the file list.
pub fn write_release(addon_dir: &Path, out_vpk: &Path, version: &str) -> Result<Release, Error> {
    if version.is_empty() || version.len() > 32 {
        return Err(Error::Pack("bad version".into()));
    }
    let files = collect_files(addon_dir)?;
    let bytes = build_vpk(&files)?;
    let entries = validate_vpk(&bytes)?;
    let sha256 = sha256_hex(&bytes);
    let mut manifest = format!("version {version}\nsha256 {sha256}\n");
    for entry in &entries {
        manifest.push_str(&format!("file {}\n", entry.path));
    }
    if let Some(parent) = out_vpk.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(out_vpk, &bytes)?;
    fs::write(out_vpk.with_extension("vpk.sha256"), format!("{sha256}\n"))?;
    fs::write(out_vpk.with_extension("vpk.manifest.txt"), &manifest)?;
    Ok(Release {
        bytes,
        sha256,
        entries,
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<ArchiveFile> {
        vec![
            ArchiveFile {
                path: "addoninfo.txt".into(),
                data: b"addon".to_vec(),
            },
            ArchiveFile {
                path: "scripts/vscripts/director_base_addon.nut".into(),
                data: b"base".to_vec(),
            },
            ArchiveFile {
                path: "scripts/vscripts/tiktools_l4d2/bootstrap.nut".into(),
                data: b"boot".to_vec(),
            },
        ]
    }

    #[test]
    fn round_trip_and_determinism() {
        let files = sample_files();
        let first = build_vpk(&files).unwrap();
        let second = build_vpk(&files).unwrap();
        assert_eq!(first, second);
        let entries = validate_vpk(&first).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn entries_mark_data_as_embedded_in_this_vpk() {
        // 0x7FFF is the only archive index that makes L4D2 read the data
        // from the same file; 0 makes it hunt for a missing `_000.vpk`
        // chunk and spin on the startup loading screen.
        let bytes = build_vpk(&sample_files()).unwrap();
        let raw = walk_tree(&bytes).unwrap();
        assert!(!raw.is_empty());
        for entry in raw {
            assert_eq!(entry.archive, VPK_SINGLE_ARCHIVE, "{}", entry.path);
        }
    }

    #[test]
    fn entries_are_relative_to_data_section() {
        // Valve stores EntryOffset relative to the first data byte, not as
        // an absolute file offset. Getting this wrong shifts every read and
        // makes the engine parse garbage scripts.
        let bytes = build_vpk(&sample_files()).unwrap();
        let tree_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let data_start = 12 + tree_size;
        let raw = walk_tree(&bytes).unwrap();
        let min_offset = raw.iter().map(|e| e.offset as usize).min().unwrap();
        let max_end = raw
            .iter()
            .map(|e| e.offset as usize + e.length as usize)
            .max()
            .unwrap();
        assert_eq!(min_offset, 0);
        assert_eq!(data_start + max_end, bytes.len());
    }

    #[test]
    fn rejects_banned_payloads() {
        let files = vec![ArchiveFile {
            path: "evil/hack.dll".into(),
            data: b"x".to_vec(),
        }];
        assert!(build_vpk(&files).is_err());
    }

    #[test]
    fn rejects_missing_required_paths() {
        let files = vec![ArchiveFile {
            path: "scripts/vscripts/tiktools_l4d2/bootstrap.nut".into(),
            data: b"boot".to_vec(),
        }];
        let bytes = build_vpk(&files).unwrap();
        assert!(validate_vpk(&bytes).is_err());
    }

    #[test]
    fn crc32_is_ieee() {
        // Check value from the CRC-32 catalogue for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
