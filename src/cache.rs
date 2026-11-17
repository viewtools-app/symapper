//! Versioned, checksummed tag cache. Only this module writes cache files.
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::lang::Lang;
use crate::tags::{FileTags, TAGS_VERSION, Tag, TagKind};

const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;

const MAGIC: &[u8; 9] = b"RMAPCACHE";
const VERSION: u32 = 1;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn extend_hash(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
    }
    hash
}

fn fnv(bytes: &[u8]) -> u64 {
    extend_hash(FNV_OFFSET, bytes)
}

fn cache_file_name(canonical: &OsStr) -> String {
    let hash = fnv(canonical.as_encoded_bytes());
    format!("symapper-{hash:016x}.bin")
}

pub fn cache_path(common_dir: &Path, env_value: Option<&OsStr>) -> Result<PathBuf, String> {
    let Some(value) = env_value.filter(|value| !value.is_empty()) else {
        return Ok(common_dir.join("symapper-cache.bin"));
    };
    let dir = Path::new(value);
    if !dir.is_absolute() {
        return Err("path must be absolute".into());
    }
    if !dir.is_dir() {
        return Err("path must be an existing directory".into());
    }
    let common_dir = common_dir
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize git common directory: {error}"))?;
    Ok(dir.join(cache_file_name(common_dir.as_os_str())))
}

pub fn schema_hash() -> u64 {
    let langs = Lang::ALL.map(|lang| (lang.query_source(), lang.scope_kinds()));
    schema_hash_of(env!("CARGO_PKG_VERSION"), TAGS_VERSION, &langs)
}

fn schema_hash_of(version: &str, tags_version: u32, langs: &[(&str, &[&str])]) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut field = |bytes: &[u8]| {
        hash = extend_hash(hash, &(bytes.len() as u64).to_le_bytes());
        hash = extend_hash(hash, bytes);
    };
    field(version.as_bytes());
    field(&tags_version.to_le_bytes());
    field(&(langs.len() as u64).to_le_bytes());
    for (query, scope_kinds) in langs {
        field(query.as_bytes());
        field(&(scope_kinds.len() as u64).to_le_bytes());
        for kind in *scope_kinds {
            field(kind.as_bytes());
        }
    }
    hash
}

fn put_count(bytes: &mut Vec<u8>, count: usize) -> Option<()> {
    let count = u32::try_from(count).ok()?;
    bytes.extend_from_slice(&count.to_le_bytes());
    Some(())
}

fn put_string(bytes: &mut Vec<u8>, value: &str) -> Option<()> {
    put_count(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Some(())
}

/// Returns empty bytes if an in-memory length cannot fit the v1 format.
pub fn encode(entries: &[FileTags], schema: u64) -> Vec<u8> {
    try_encode(entries, schema).unwrap_or_default()
}

fn try_encode(entries: &[FileTags], schema: u64) -> Option<Vec<u8>> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&schema.to_le_bytes());
    put_count(&mut bytes, entries.len())?;
    for entry in entries {
        put_string(&mut bytes, &entry.path)?;
        bytes.extend_from_slice(&entry.mtime_ns.to_le_bytes());
        bytes.extend_from_slice(&entry.size.to_le_bytes());
        put_count(&mut bytes, entry.tags.len())?;
        for tag in &entry.tags {
            put_string(&mut bytes, &tag.name)?;
            bytes.push(match tag.kind {
                TagKind::Def => 0,
                TagKind::Ref => 1,
            });
            bytes.extend_from_slice(&tag.line.to_le_bytes());
            put_count(&mut bytes, tag.scopes.len())?;
            for scope in &tag.scopes {
                bytes.extend_from_slice(&scope.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&fnv(&bytes).to_le_bytes());
    Some(bytes)
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        if len > self.remaining.len() {
            return None;
        }
        let (taken, rest) = self.remaining.split_at(len);
        self.remaining = rest;
        Some(taken)
    }
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }
    fn count(&mut self, min_size: usize) -> Option<usize> {
        let count = usize::try_from(self.u32()?).ok()?;
        (count <= self.remaining.len() / min_size).then_some(count)
    }
    fn string(&mut self) -> Option<String> {
        let len = self.count(1)?;
        Some(std::str::from_utf8(self.take(len)?).ok()?.to_owned())
    }
}

pub fn decode(bytes: &[u8], schema: u64) -> Option<Vec<FileTags>> {
    let end = bytes.len().checked_sub(8)?;
    let (payload, checksum) = bytes.split_at(end);
    if fnv(payload) != u64::from_le_bytes(checksum.try_into().ok()?) {
        return None;
    }
    let mut cursor = Cursor { remaining: payload };
    if cursor.take(MAGIC.len())? != MAGIC
        || cursor.u32()? != VERSION
        || u64::from_le_bytes(cursor.array()?) != schema
    {
        return None;
    }
    // Minimum encoded sizes include all mandatory fields, even for empty strings.
    let count = cursor.count(32)?;
    let mut entries = Vec::new();
    for _ in 0..count {
        let path = cursor.string()?;
        let mtime_ns = u128::from_le_bytes(cursor.array()?);
        let size = u64::from_le_bytes(cursor.array()?);
        let tag_count = cursor.count(13)?;
        let mut tags = Vec::new();
        for _ in 0..tag_count {
            let name = cursor.string()?;
            let kind = match cursor.take(1)?[0] {
                0 => TagKind::Def,
                1 => TagKind::Ref,
                _ => return None,
            };
            let line = cursor.u32()?;
            let scope_count = cursor.count(4)?;
            let mut scopes = Vec::new();
            for _ in 0..scope_count {
                scopes.push(cursor.u32()?);
            }
            tags.push(Tag {
                name,
                kind,
                line,
                scopes,
            });
        }
        entries.push(FileTags {
            path,
            mtime_ns,
            size,
            tags,
        });
    }
    cursor.remaining.is_empty().then_some(entries)
}

pub fn load(path: &Path) -> Option<Vec<FileTags>> {
    let before = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            eprintln!("symapper: ignoring cache: {error}");
            return None;
        }
    };
    if !before.is_file() {
        eprintln!("symapper: ignoring cache: not a regular file");
        return None;
    }
    if before.len() > MAX_CACHE_BYTES {
        eprintln!("symapper: ignoring cache: too large");
        return None;
    }
    // No new platform dependency: check the opened handle against the path metadata.
    // This detects replacement after opening, but cannot make the open itself atomic.
    let read = || -> io::Result<Option<Vec<u8>>> {
        let file = File::open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() {
            eprintln!("symapper: ignoring cache: not a regular file");
            return Ok(None);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if before.dev() != opened.dev() || before.ino() != opened.ino() {
                eprintln!("symapper: ignoring cache: file changed while opening");
                return Ok(None);
            }
        }
        if opened.len() > MAX_CACHE_BYTES {
            eprintln!("symapper: ignoring cache: too large");
            return Ok(None);
        }
        let mut bytes = Vec::new();
        file.take(MAX_CACHE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            eprintln!("symapper: ignoring cache: too large");
            return Ok(None);
        }
        Ok(Some(bytes))
    };
    let bytes = match read() {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return None,
        Err(error) => {
            eprintln!("symapper: ignoring cache: {error}");
            return None;
        }
    };
    let entries = decode(&bytes, schema_hash());
    if entries.is_none() {
        eprintln!("symapper: ignoring cache: invalid format, schema, or checksum");
    }
    entries
}

pub fn save(path: &Path, entries: &[FileTags]) -> Result<(), io::Error> {
    let mut temp_name = path.as_os_str().to_os_string();
    temp_name.push(format!(".tmp-{}", std::process::id()));
    let temp = PathBuf::from(temp_name);
    let bytes = try_encode(entries, schema_hash()).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "cache length exceeds v1 limit")
    })?;
    // Do not truncate or remove a temporary file owned by another writer.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCHEMA: u64 = 42;

    fn entries() -> Vec<FileTags> {
        vec![
            FileTags {
                path: "src/café.rs".into(),
                mtime_ns: u128::MAX,
                size: 123,
                tags: vec![
                    Tag {
                        name: "định_nghĩa".into(),
                        kind: TagKind::Def,
                        line: 17,
                        scopes: vec![1, 4, 12],
                    },
                    Tag {
                        name: "reference".into(),
                        kind: TagKind::Ref,
                        line: 21,
                        scopes: vec![],
                    },
                ],
            },
            FileTags {
                path: "empty.rs".into(),
                mtime_ns: 0,
                size: 0,
                tags: vec![],
            },
        ]
    }
    fn rechecksum(bytes: &mut [u8]) {
        let end = bytes.len() - 8;
        let checksum = fnv(&bytes[..end]);
        bytes[end..].copy_from_slice(&checksum.to_le_bytes());
    }
    fn corrupt(offset: usize, replacement: &[u8]) -> Vec<u8> {
        let mut bytes = encode(&entries(), SCHEMA);
        bytes[offset..offset + replacement.len()].copy_from_slice(replacement);
        rechecksum(&mut bytes);
        bytes
    }
    #[test]
    fn round_trip() {
        assert_eq!(decode(&encode(&entries(), SCHEMA), SCHEMA), Some(entries()));
    }
    #[test]
    fn empty_entries() {
        assert_eq!(decode(&encode(&[], SCHEMA), SCHEMA), Some(vec![]));
    }
    #[test]
    fn bad_magic() {
        assert_eq!(decode(&corrupt(0, b"X"), SCHEMA), None);
    }
    #[test]
    fn old_version() {
        assert_eq!(decode(&corrupt(9, &0u32.to_le_bytes()), SCHEMA), None);
    }
    #[test]
    fn other_schema() {
        assert_eq!(decode(&encode(&entries(), SCHEMA), SCHEMA + 1), None);
    }
    #[test]
    fn bad_checksum() {
        let mut bytes = encode(&entries(), SCHEMA);
        let end = bytes.len() - 1;
        bytes[end] ^= 1;
        assert_eq!(decode(&bytes, SCHEMA), None);
    }
    #[test]
    fn truncated() {
        let bytes = encode(&entries(), SCHEMA);
        for end in 0..bytes.len() {
            assert_eq!(decode(&bytes[..end], SCHEMA), None);
        }
        let mut bytes = bytes[..bytes.len() - 9].to_vec();
        bytes.extend_from_slice(&[0; 8]);
        rechecksum(&mut bytes);
        assert_eq!(decode(&bytes, SCHEMA), None);
    }
    #[test]
    fn length_past_end() {
        assert_eq!(decode(&corrupt(25, &u32::MAX.to_le_bytes()), SCHEMA), None);
    }
    #[test]
    fn excessive_counts() {
        let tag_count = 29 + entries()[0].path.len() + 24;
        let scope_count = tag_count + 4 + 4 + entries()[0].tags[0].name.len() + 5;
        for offset in [21, tag_count, scope_count] {
            assert_eq!(
                decode(&corrupt(offset, &u32::MAX.to_le_bytes()), SCHEMA),
                None
            );
        }
    }
    #[test]
    fn trailing_bytes() {
        let mut bytes = encode(&entries(), SCHEMA);
        bytes.insert(bytes.len() - 8, 0);
        rechecksum(&mut bytes);
        assert_eq!(decode(&bytes, SCHEMA), None);
    }
    #[test]
    fn bad_utf8() {
        assert_eq!(decode(&corrupt(29, &[0xff]), SCHEMA), None);
        let name_start = 29 + entries()[0].path.len() + 24 + 4 + 4;
        assert_eq!(decode(&corrupt(name_start, &[0xff]), SCHEMA), None);
    }
    #[test]
    fn bad_kind() {
        let kind = 29 + entries()[0].path.len() + 24 + 4 + 4 + entries()[0].tags[0].name.len();
        assert_eq!(decode(&corrupt(kind, &[2]), SCHEMA), None);
    }
    #[test]
    fn default_path_for_unset_or_empty_env() {
        let dir = tempfile::tempdir().unwrap();
        for value in [None, Some(OsStr::new(""))] {
            assert_eq!(
                cache_path(dir.path(), value).unwrap(),
                dir.path().join("symapper-cache.bin")
            );
        }
    }
    #[test]
    fn env_path_hashes_canonical_common_dir_bytes() {
        let common = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let canonical = common.path().canonicalize().unwrap();
        let hash = fnv(canonical.as_os_str().as_encoded_bytes());
        let expected = dir.path().join(format!("symapper-{hash:016x}.bin"));
        for path in [common.path().to_owned(), common.path().join(".")] {
            assert_eq!(
                cache_path(&path, Some(dir.path().as_os_str())).unwrap(),
                expected
            );
        }
    }
    #[test]
    fn invalid_env_paths_are_rejected_without_creating_directories() {
        let common = tempfile::tempdir().unwrap();
        let missing = common.path().join("missing");
        assert!(cache_path(common.path(), Some(OsStr::new("relative"))).is_err());
        assert!(cache_path(common.path(), Some(missing.as_os_str())).is_err());
        assert!(!missing.exists());
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_cache_file_name_uses_raw_path_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let canonical = OsStr::from_bytes(b"/repo-\xff");
        let other = OsStr::from_bytes(b"/repo-\xfe");
        let hash = fnv(canonical.as_bytes());
        assert_eq!(
            cache_file_name(canonical),
            format!("symapper-{hash:016x}.bin")
        );
        assert_ne!(cache_file_name(canonical), cache_file_name(other));
        assert_ne!(
            cache_file_name(canonical),
            cache_file_name(OsStr::new(canonical.to_string_lossy().as_ref()))
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_common_dir_uses_raw_path_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let root = tempfile::tempdir().unwrap();
        let common = root.path().join(OsStr::from_bytes(b"repo-\xff"));
        fs::create_dir(&common).unwrap();
        let canonical = common.canonicalize().unwrap();
        let hash = fnv(canonical.as_os_str().as_bytes());
        assert_eq!(
            cache_path(&common, Some(root.path().as_os_str())).unwrap(),
            root.path().join(format!("symapper-{hash:016x}.bin"))
        );
    }
    #[test]
    fn save_load_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = cache_path(dir.path(), None).unwrap();
        assert_eq!(load(&path), None);
        save(&path, &entries()).unwrap();
        assert_eq!(load(&path), Some(entries()));
        save(&path, &[]).unwrap();
        assert_eq!(load(&path), Some(vec![]));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        std::fs::write(&path, b"corrupt").unwrap();
        assert_eq!(load(&path), None);
    }
    #[test]
    fn failed_rename_cleans_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = cache_path(dir.path(), None).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(save(&path, &entries()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn exact_empty_format() {
        let mut expected = b"RMAPCACHE".to_vec();
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&SCHEMA.to_le_bytes());
        expected.extend_from_slice(&0u32.to_le_bytes());
        expected.extend_from_slice(&fnv(&expected).to_le_bytes());
        assert_eq!(encode(&[], SCHEMA), expected);
    }

    #[test]
    fn fnv_known_vector() {
        assert_eq!(fnv(b"hello"), 0xa430d84680aabd0b);
    }

    #[test]
    fn schema_includes_versions_and_every_query_and_scope_kind() {
        let mut bytes = Vec::new();
        let mut field = |value: &[u8]| {
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value);
        };
        field(env!("CARGO_PKG_VERSION").as_bytes());
        field(&crate::tags::TAGS_VERSION.to_le_bytes());
        field(&(Lang::ALL.len() as u64).to_le_bytes());
        for lang in Lang::ALL {
            field(lang.query_source().as_bytes());
            field(&(lang.scope_kinds().len() as u64).to_le_bytes());
            for kind in lang.scope_kinds() {
                field(kind.as_bytes());
            }
        }
        assert_eq!(schema_hash(), fnv(&bytes));
    }

    #[test]
    fn schema_changes_with_each_query_scope_and_version() {
        let langs: Vec<_> = Lang::ALL
            .iter()
            .map(|lang| (lang.query_source(), lang.scope_kinds()))
            .collect();
        let version = env!("CARGO_PKG_VERSION");
        let tags_version = crate::tags::TAGS_VERSION;
        let baseline = schema_hash_of(version, tags_version, &langs);
        assert_eq!(baseline, schema_hash());
        assert_ne!(baseline, schema_hash_of("changed", tags_version, &langs));
        assert_ne!(baseline, schema_hash_of(version, tags_version + 1, &langs));
        for (index, lang) in Lang::ALL.iter().enumerate() {
            let mut changed = langs.clone();
            changed[index].0 = "changed query";
            assert_ne!(baseline, schema_hash_of(version, tags_version, &changed));
            for scope_index in 0..lang.scope_kinds().len() {
                let mut scopes = lang.scope_kinds().to_vec();
                scopes[scope_index] = "changed_scope";
                let mut changed = langs.clone();
                changed[index].1 = &scopes;
                assert_ne!(baseline, schema_hash_of(version, tags_version, &changed));
            }
        }
    }

    #[test]
    fn unrepresentable_length_is_rejected() {
        if let Some(length) = (u32::MAX as usize).checked_add(1) {
            let mut bytes = Vec::new();
            assert_eq!(put_count(&mut bytes, length), None);
            assert!(bytes.is_empty());
        }
    }
}

#[cfg(all(test, unix))]
mod symlink_tests {
    use super::*;

    #[test]
    fn save_replaces_symlink_without_touching_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let path = dir.path().join("cache");
        fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        save(&path, &[]).unwrap();
        assert!(fs::symlink_metadata(&path).unwrap().is_file());
        assert_eq!(fs::read(&target).unwrap(), b"untouched");
        assert_eq!(load(&path), Some(vec![]));
    }
}
