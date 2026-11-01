use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFile {
    pub id: u64,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct Episode {
    pub season: u32,
    pub episode: u32,
}

impl Episode {
    pub const fn new(season: u32, episode: u32) -> Self {
        Self { season, episode }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInput {
    pub info_hash: String,
    pub file_idx: Option<u64>,
    pub source_filename: Option<String>,
    pub selected_episodes: Vec<Episode>,
}

impl SelectionInput {
    pub fn new(
        info_hash: impl Into<String>,
        file_idx: Option<u64>,
        source_filename: Option<&str>,
        selected_episodes: Vec<Episode>,
    ) -> Self {
        Self {
            info_hash: info_hash.into(),
            file_idx,
            source_filename: source_filename.map(str::to_owned),
            selected_episodes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    InvalidInput,
    Missing,
    Ambiguous,
    ZeroSize,
}

pub fn select_files(
    input: &SelectionInput,
    manifest: &[ProviderFile],
) -> Result<Vec<ProviderFile>, SelectionError> {
    validate(input)?;

    if let Some(file_idx) = input.file_idx {
        let file = manifest
            .get(usize::try_from(file_idx).map_err(|_| SelectionError::Missing)?)
            .ok_or(SelectionError::Missing)?;
        if manifest
            .iter()
            .filter(|candidate| candidate.id == file.id)
            .count()
            != 1
        {
            return Err(SelectionError::Ambiguous);
        }
        if let Some(filename) = input.source_filename.as_deref() {
            let expected = normalize_path(filename);
            let actual = normalize_path(&file.name);
            let basename_match = std::path::Path::new(&actual).file_name()
                == std::path::Path::new(&expected).file_name();
            if actual != expected && !basename_match {
                return Err(SelectionError::Missing);
            }
        }
        return selected([file]);
    }

    if let Some(filename) = input.source_filename.as_deref() {
        let expected = normalize_path(filename);
        let matches: Vec<_> = manifest
            .iter()
            .filter(|file| normalize_path(&file.name) == expected)
            .collect();
        return match matches.as_slice() {
            [file] => selected([*file]),
            [] => Err(SelectionError::Missing),
            _ => Err(SelectionError::Ambiguous),
        };
    }

    if input.selected_episodes.is_empty() {
        return Err(SelectionError::Missing);
    }

    let mut ids = HashSet::new();
    for episode in &input.selected_episodes {
        let matches: Vec<_> = manifest
            .iter()
            .filter(|file| filename_has_episode(&file.name, *episode))
            .collect();
        match matches.as_slice() {
            [file] => {
                ids.insert(file.id);
            }
            [] => return Err(SelectionError::Missing),
            _ => return Err(SelectionError::Ambiguous),
        }
    }
    selected(manifest.iter().filter(|file| ids.contains(&file.id)))
}

fn validate(input: &SelectionInput) -> Result<(), SelectionError> {
    if input.info_hash.trim().is_empty()
        || input
            .source_filename
            .as_ref()
            .is_some_and(|name| name.is_empty())
        || input
            .selected_episodes
            .iter()
            .any(|episode| episode.season == 0 || episode.episode == 0)
        || input
            .selected_episodes
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != input.selected_episodes.len()
    {
        Err(SelectionError::InvalidInput)
    } else {
        Ok(())
    }
}

fn selected<'a>(
    files: impl IntoIterator<Item = &'a ProviderFile>,
) -> Result<Vec<ProviderFile>, SelectionError> {
    let files: Vec<_> = files.into_iter().cloned().collect();
    if files.is_empty() {
        Err(SelectionError::Missing)
    } else if files.iter().any(|file| file.size == 0) {
        Err(SelectionError::ZeroSize)
    } else {
        Ok(files)
    }
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn filename_has_episode(filename: &str, episode: Episode) -> bool {
    let filename = filename.to_ascii_uppercase();
    let tokens = [
        format!("S{:02}E{:02}", episode.season, episode.episode),
        format!("{}X{:02}", episode.season, episode.episode),
    ];
    tokens.iter().any(|token| {
        filename.match_indices(token).any(|(start, matched)| {
            let end = start + matched.len();
            !filename[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
                && !filename[end..].starts_with(|c: char| c.is_ascii_digit())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, name: &str) -> ProviderFile {
        ProviderFile {
            id,
            name: name.into(),
            size: 10,
        }
    }

    #[test]
    fn stremio_file_idx_is_manifest_position_not_provider_id() {
        let manifest = [file(0, "first.mkv"), file(7, "pack/second.mkv")];
        assert_eq!(
            select_files(
                &SelectionInput::new("hash", Some(0), Some("first.mkv"), vec![]),
                &manifest
            )
            .unwrap()[0]
                .id,
            0
        );
        assert_eq!(
            select_files(
                &SelectionInput::new("hash", Some(1), Some("second.mkv"), vec![]),
                &manifest
            )
            .unwrap()[0]
                .id,
            7
        );
    }

    #[test]
    fn positional_selection_fails_closed() {
        let unique = [file(3, "pack/episode.mkv")];
        assert_eq!(
            select_files(
                &SelectionInput::new("hash", Some(0), Some("wrong.mkv"), vec![]),
                &unique
            ),
            Err(SelectionError::Missing)
        );
        let manifest = [file(7, "pack/episode.mkv"), file(7, "other.mkv")];
        assert_eq!(
            select_files(
                &SelectionInput::new("hash", Some(1), None, vec![]),
                &manifest
            ),
            Err(SelectionError::Ambiguous)
        );
        assert_eq!(
            select_files(
                &SelectionInput::new("hash", Some(2), None, vec![]),
                &manifest
            ),
            Err(SelectionError::Missing)
        );
    }
}
