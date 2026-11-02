use regex::Regex;
use std::path::{Path, PathBuf};

const MEDIA_EXTS: &[&str] = &["mkv", "mp4", "avi", "m4v", "ts", "mov"];

fn safe_media(root: &Path, path: &Path) -> Result<bool, String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "media path escapes torrent root")?;
    if rel
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("media path contains traversal".into());
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Ok(false);
    }
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(MEDIA_EXTS.contains(&ext.as_str())
        && !name.contains("sample")
        && !name.contains("trailer")
        && !name.contains("extra"))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("unreachable torrent source {}: {e}", dir.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk(root, &path, out)?;
        } else if safe_media(root, &path)? {
            out.push(path);
        }
    }
    Ok(())
}

pub fn torrent_media(source: &Path) -> Result<Vec<PathBuf>, String> {
    let meta = std::fs::symlink_metadata(source)
        .map_err(|e| format!("unreachable torrent source {}: {e}", source.display()))?;
    if meta.file_type().is_symlink() {
        return Err("torrent source must not be a symlink".into());
    }
    let mut files = Vec::new();
    if meta.is_file() {
        let root = source.parent().ok_or("torrent file has no parent")?;
        if safe_media(root, source)? {
            files.push(source.to_path_buf());
        }
    } else if meta.is_dir() {
        walk(source, source, &mut files)?;
    }
    files.sort();
    Ok(files)
}

pub fn select_torrent_media(
    files: &[PathBuf],
    file_idx: Option<usize>,
    filename: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    if let Some(i) = file_idx {
        return files
            .get(i)
            .cloned()
            .map(|p| vec![p])
            .ok_or_else(|| format!("file_idx {i} is outside torrent media list"));
    }
    if let Some(name) = filename {
        let hits: Vec<_> = files
            .iter()
            .filter(|p| p.file_name().and_then(|v| v.to_str()) == Some(name))
            .cloned()
            .collect();
        if hits.len() == 1 {
            return Ok(hits);
        }
        return Err(format!(
            "source_filename {name:?} is not unique in torrent media list"
        ));
    }
    Ok(files.to_vec())
}

pub fn episode_number(path: &Path) -> Option<(u32, u32)> {
    let re = Regex::new(r"(?i)s(\d{1,2})[ ._-]*e(\d{1,3})").unwrap();
    let caps = re.captures(path.file_name()?.to_str()?)?;
    Some((caps[1].parse().ok()?, caps[2].parse().ok()?))
}

fn symlink_in_library(source: &Path, library: &Path, target: &Path) -> Result<(), String> {
    let root = library
        .canonicalize()
        .map_err(|e| format!("invalid library root: {e}"))?;
    let parent = target.parent().ok_or("library target has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let canonical_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_parent.starts_with(&root) {
        return Err("library target escapes canonical root".into());
    }
    symlink_exact(source, target)
}

pub fn symlink_exact(source: &Path, target: &Path) -> Result<(), String> {
    let source = source
        .canonicalize()
        .map_err(|e| format!("unreachable source {}: {e}", source.display()))?;
    if !source.is_file() {
        return Err("symlink source is not a regular file".into());
    }
    if let Ok(meta) = std::fs::symlink_metadata(target) {
        if meta.file_type().is_symlink()
            && std::fs::read_link(target).ok().as_deref() == Some(source.as_path())
        {
            return Ok(());
        }
        return Err(format!("conflicting library target {}", target.display()));
    }
    std::fs::create_dir_all(target.parent().ok_or("library target has no parent")?)
        .map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(source, target).map_err(|e| e.to_string())
}

pub fn move_exact(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|e| format!("unreachable source {}: {e}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("move source is not a regular file".into());
    }
    if std::fs::symlink_metadata(target).is_ok() {
        return Err(format!("conflicting library target {}", target.display()));
    }
    std::fs::create_dir_all(target.parent().ok_or("library target has no parent")?)
        .map_err(|e| e.to_string())?;
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(18) => {
            let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| e.to_string())?;
            if let Err(error) =
                std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all())
            {
                let _ = std::fs::remove_file(target);
                return Err(error.to_string());
            }
            std::fs::remove_file(source).map_err(|e| e.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn place_torrent(
    source: &Path,
    library: &Path,
    movie: bool,
    file_idx: Option<usize>,
    source_filename: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let files = torrent_media(source)?;
    let explicit = file_idx.is_some() || source_filename.is_some();
    let mut selected = select_torrent_media(&files, file_idx, source_filename)?;
    if selected.is_empty() {
        return Err(format!(
            "torrent source contains no eligible media: {}",
            source.display()
        ));
    }
    if movie && !explicit {
        selected = vec![selected
            .into_iter()
            .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .unwrap()];
    }
    let mut targets = Vec::new();
    for media in selected {
        let target = if movie {
            library.join(media.file_name().ok_or("torrent media has no filename")?)
        } else {
            let (season, episode) = episode_number(&media).ok_or_else(|| {
                format!("torrent TV media has no S/E mapping: {}", media.display())
            })?;
            let ext = media.extension().and_then(|v| v.to_str()).unwrap_or("mkv");
            library.join(format!("Season {season}/S{season:02}E{episode:02}.{ext}"))
        };
        symlink_in_library(&media, library, &target)?;
        targets.push(target);
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;

    fn media(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7; 32]).unwrap();
    }

    #[test]
    fn torrent_single_file_unchanged_symlink_only() {
        let d = tempdir().unwrap();
        let src = d.path().join("Original.mkv");
        media(&src);
        let ino = src.metadata().unwrap().ino();
        let dst = d.path().join("library/Movie.mkv");
        symlink_exact(&src, &dst).unwrap();
        assert_eq!(src.metadata().unwrap().ino(), ino);
        assert_eq!(std::fs::read(&src).unwrap(), vec![7; 32]);
        assert!(dst.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[test]
    fn direct_file_moves_without_leaving_source_or_symlink() {
        let d = tempdir().unwrap();
        let src = d.path().join("download/Movie.mkv");
        let dst = d.path().join("library/Movie.mkv");
        media(&src);

        move_exact(&src, &dst).unwrap();

        assert!(!src.exists());
        assert!(dst.is_file());
        assert!(!dst.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(dst).unwrap(), vec![7; 32]);
    }

    #[test]
    fn season_pack_filters_and_maps_without_mutation() {
        let d = tempdir().unwrap();
        let root = d.path().join("pack");
        for n in [
            "Show.S01E01.mkv",
            "Show.S01E02.mkv",
            "sample.mkv",
            "Show.S01E01.srt",
        ] {
            media(&root.join(n));
        }
        let before: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        let files = torrent_media(&root).unwrap();
        assert_eq!(files.len(), 2);
        for f in &files {
            let (s, e) = episode_number(f).unwrap();
            symlink_exact(
                f,
                &d.path()
                    .join(format!("lib/Season {s}/Show - S{s:02}E{e:02}.mkv")),
            )
            .unwrap();
        }
        let after: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(before, after);
        assert_eq!(
            std::fs::read_dir(d.path().join("lib/Season 1"))
                .unwrap()
                .count(),
            2
        );
    }

    #[test]
    fn exact_selection_idempotency_conflict_and_guards() {
        let d = tempdir().unwrap();
        let root = d.path().join("pack");
        let a = root.join("A.S01E01.mkv");
        let b = root.join("B.S01E02.mkv");
        media(&a);
        media(&b);
        let files = torrent_media(&root).unwrap();
        assert_eq!(
            select_torrent_media(&files, Some(1), None).unwrap(),
            vec![b.clone()]
        );
        assert_eq!(
            select_torrent_media(&files, None, Some("A.S01E01.mkv")).unwrap(),
            vec![a.clone()]
        );
        let target = d.path().join("lib/x.mkv");
        symlink_exact(&a, &target).unwrap();
        symlink_exact(&a, &target).unwrap();
        assert!(symlink_exact(&b, &target)
            .unwrap_err()
            .contains("conflicting"));
        let escape = root.join("escape.mkv");
        std::os::unix::fs::symlink(&a, &escape).unwrap();
        assert_eq!(torrent_media(&root).unwrap().len(), 2);
        assert!(torrent_media(&d.path().join("missing"))
            .unwrap_err()
            .contains("unreachable"));
    }

    #[test]
    fn library_parent_symlink_cannot_escape_temp_root() {
        let d = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let source = d.path().join("source.mkv");
        media(&source);
        let library = d.path().join("library");
        std::fs::create_dir(&library).unwrap();
        std::os::unix::fs::symlink(outside.path(), library.join("Season 1")).unwrap();

        let error =
            symlink_in_library(&source, &library, &library.join("Season 1/Show.S01E01.mkv"))
                .unwrap_err();
        assert!(error.contains("escapes canonical root"));
        assert!(!outside.path().join("Show.S01E01.mkv").exists());
    }
}
