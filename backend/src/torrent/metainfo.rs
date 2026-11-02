const MAX_DEPTH: usize = 128;
const MAX_FILE_COUNT: usize = 10_000;
const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;
const MAX_NODES: usize = 100_000;

pub(crate) fn is_media_path(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mkv" | "mp4" | "avi" | "mov" | "wmv" | "m4v" | "ts" | "m2ts" | "webm")
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MetainfoFile {
    pub(crate) path_components: Vec<String>,
    pub(crate) path: String,
    pub(crate) length: u64,
    pub(crate) index: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Metainfo {
    pub(crate) root_name: String,
    pub(crate) files: Vec<MetainfoFile>,
    pub(crate) total_size: u64,
    pub(crate) file_count: usize,
    pub(crate) media_file_count: usize,
}

enum Value<'a> {
    Bytes(&'a [u8]),
    Integer(u64),
    List(Vec<Value<'a>>),
    Dict(Vec<(&'a [u8], Value<'a>)>),
}

struct Parser<'a> {
    input: &'a [u8],
    offset: usize,
    nodes: usize,
}

impl<'a> Parser<'a> {
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let start = self.offset;
        while self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        if self.offset == start || self.input.get(self.offset) != Some(&b':') {
            return Err("invalid byte string".into());
        }
        if self.offset - start > 1 && self.input[start] == b'0' {
            return Err("invalid byte string length".into());
        }
        let length = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| "invalid byte string length")?
            .parse::<usize>()
            .map_err(|_| "byte string length overflow")?;
        self.offset += 1;
        let end = self
            .offset
            .checked_add(length)
            .ok_or("byte string length overflow")?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or("truncated byte string")?;
        self.offset = end;
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<Value<'a>, String> {
        if depth > MAX_DEPTH {
            return Err("maximum bencode depth exceeded".into());
        }
        self.nodes = self.nodes.checked_add(1).ok_or("node budget exceeded")?;
        if self.nodes > MAX_NODES {
            return Err("node budget exceeded".into());
        }
        match self.input.get(self.offset).copied() {
            Some(b'0'..=b'9') => Ok(Value::Bytes(self.bytes()?)),
            Some(b'i') => {
                self.offset += 1;
                let start = self.offset;
                while self.input.get(self.offset) != Some(&b'e') {
                    match self.input.get(self.offset) {
                        Some(b'0'..=b'9') => self.offset += 1,
                        _ => return Err("invalid integer".into()),
                    }
                }
                if self.offset == start || (self.offset - start > 1 && self.input[start] == b'0') {
                    return Err("invalid integer".into());
                }
                let integer = std::str::from_utf8(&self.input[start..self.offset])
                    .map_err(|_| "invalid integer")?
                    .parse::<u64>()
                    .map_err(|_| "integer overflow")?;
                self.offset += 1;
                Ok(Value::Integer(integer))
            }
            Some(b'l') => {
                self.offset += 1;
                let mut values = Vec::new();
                while self.input.get(self.offset) != Some(&b'e') {
                    values.push(self.value(depth + 1)?);
                }
                self.offset += 1;
                Ok(Value::List(values))
            }
            Some(b'd') => {
                self.offset += 1;
                let mut values = Vec::new();
                while self.input.get(self.offset) != Some(&b'e') {
                    let key = self.bytes()?;
                    let value = self.value(depth + 1)?;
                    values.push((key, value));
                }
                self.offset += 1;
                Ok(Value::Dict(values))
            }
            _ => Err("invalid bencode value".into()),
        }
    }
}

fn field<'a>(dict: &'a [(&[u8], Value<'a>)], key: &[u8]) -> Result<Option<&'a Value<'a>>, String> {
    let mut found = dict.iter().filter(|(candidate, _)| *candidate == key);
    let value = found.next().map(|(_, value)| value);
    if found.next().is_some() {
        return Err("duplicate dictionary key".into());
    }
    Ok(value)
}

fn text(value: &Value<'_>) -> Result<String, String> {
    match value {
        Value::Bytes(bytes) if !bytes.is_empty() => Ok(String::from_utf8_lossy(bytes).into_owned()),
        _ => Err("expected non-empty byte string".into()),
    }
}

pub(crate) fn inspect_metainfo(input: &[u8]) -> Result<Metainfo, String> {
    if input.len() > MAX_INPUT_SIZE {
        return Err("metainfo exceeds size limit".into());
    }
    let mut parser = Parser {
        input,
        offset: 0,
        nodes: 0,
    };
    let root = parser.value(0)?;
    if parser.offset != input.len() {
        return Err("trailing bencode data".into());
    }
    let Value::Dict(root) = root else {
        return Err("metainfo root is not a dictionary".into());
    };
    let Some(Value::Dict(info)) = field(&root, b"info")? else {
        return Err("missing info dictionary".into());
    };
    let root_name = text(field(info, b"name")?.ok_or("missing name")?)?;
    validate_component(&root_name)?;

    let mut files = Vec::new();
    if let Some(value) = field(info, b"files")? {
        let Value::List(entries) = value else {
            return Err("files is not a list".into());
        };
        if entries.is_empty() || entries.len() > MAX_FILE_COUNT {
            return Err("invalid file count".into());
        }
        let mut paths = std::collections::HashSet::with_capacity(entries.len());
        for entry in entries {
            let Value::Dict(entry) = entry else {
                return Err("file is not a dictionary".into());
            };
            let length = match field(entry, b"length")?.ok_or("missing file length")? {
                Value::Integer(value) => *value,
                _ => return Err("invalid file length".into()),
            };
            let Value::List(parts) = field(entry, b"path")?.ok_or("missing file path")? else {
                return Err("invalid file path".into());
            };
            if parts.is_empty() {
                return Err("empty file path".into());
            }
            let path_components = parts.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            for component in &path_components {
                validate_component(component)?;
            }
            let path = path_components.join("/");
            if !paths.insert(path_components.clone()) {
                return Err("duplicate file path".into());
            }
            files.push(MetainfoFile {
                path_components,
                path,
                length,
                index: files.len(),
            });
        }
    } else {
        let length = match field(info, b"length")?.ok_or("missing length")? {
            Value::Integer(value) => *value,
            _ => return Err("invalid length".into()),
        };
        files.push(MetainfoFile {
            path_components: vec![root_name.clone()],
            path: root_name.clone(),
            length,
            index: 0,
        });
    }

    let total_size = files.iter().try_fold(0u64, |sum, file| {
        sum.checked_add(file.length).ok_or("total size overflow")
    })?;
    let media_file_count = files
        .iter()
        .filter(|file| is_media_path(&file.path))
        .count();
    Ok(Metainfo {
        root_name,
        file_count: files.len(),
        media_file_count,
        total_size,
        files,
    })
}

fn validate_component(component: &str) -> Result<(), String> {
    if component == "."
        || component == ".."
        || component.contains('\0')
        || component.contains('/')
        || component.contains('\\')
    {
        return Err("unsafe path component".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_single_file_metainfo() {
        let torrent = b"d4:infod6:lengthi123e4:name9:movie.mkvee";
        let info = inspect_metainfo(torrent).unwrap();
        assert_eq!(info.root_name, "movie.mkv");
        assert_eq!(
            info.files,
            vec![MetainfoFile {
                path_components: vec!["movie.mkv".into()],
                path: "movie.mkv".into(),
                length: 123,
                index: 0
            }]
        );
        assert_eq!(
            (info.total_size, info.file_count, info.media_file_count),
            (123, 1, 1)
        );
    }

    #[test]
    fn inspects_multi_file_metainfo_and_counts_media() {
        let torrent = b"d4:infod5:filesld6:lengthi10e4:pathl5:a.mkveed6:lengthi20e4:pathl10:readme.txteee4:name4:Packee";
        let info = inspect_metainfo(torrent).unwrap();
        assert_eq!(info.root_name, "Pack");
        assert_eq!(info.files[0].path_components, ["a.mkv"]);
        assert_eq!(info.files[1].path, "readme.txt");
        assert_eq!(
            (info.total_size, info.file_count, info.media_file_count),
            (30, 2, 1)
        );
    }

    #[test]
    fn uses_lossy_utf8_without_changing_structure() {
        let info = inspect_metainfo(b"d4:infod6:lengthi1e4:name5:\xff.mkvee").unwrap();
        assert_eq!(info.root_name, "�.mkv");
        assert_eq!(info.files[0].path_components, ["�.mkv"]);
    }

    #[test]
    fn rejects_malformed_negative_overflow_and_excessive_depth() {
        assert!(inspect_metainfo(b"not-bencode").is_err());
        assert!(inspect_metainfo(b"d4:infod6:lengthi-1e4:name1:xee").is_err());
        assert!(inspect_metainfo(b"d4:infod6:lengthi18446744073709551616e4:name1:xee").is_err());
        let mut deep = b"d4:info".to_vec();
        deep.extend(std::iter::repeat(b'l').take(129));
        deep.extend(std::iter::repeat(b'e').take(129));
        deep.push(b'e');
        assert!(inspect_metainfo(&deep).is_err());
    }

    #[test]
    fn rejects_more_than_bounded_file_count() {
        let mut torrent = b"d4:infod5:filesl".to_vec();
        for _ in 0..=MAX_FILE_COUNT {
            torrent.extend_from_slice(b"d6:lengthi0e4:pathl1:xee");
        }
        torrent.extend_from_slice(b"e4:name1:xee");
        assert!(inspect_metainfo(&torrent).is_err());
    }

    #[test]
    fn rejects_empty_duplicate_paths_and_total_overflow() {
        assert!(inspect_metainfo(b"d4:infod5:filesld6:lengthi1e4:pathleee4:name1:xee").is_err());
        assert!(inspect_metainfo(
            b"d4:infod5:filesld6:lengthi1e4:pathl1:xeed6:lengthi2e4:pathl1:xeee4:name1:xee"
        )
        .is_err());
        assert!(inspect_metainfo(b"d4:infod5:filesld6:lengthi18446744073709551615e4:pathl1:xeed6:lengthi1e4:pathl1:yeee4:name1:xee").is_err());
    }

    #[test]
    fn rejects_unsafe_components_and_oversized_input() {
        for torrent in [
            b"d4:infod6:lengthi1e4:name2:..ee".as_slice(),
            b"d4:infod6:lengthi1e4:name3:a/bee".as_slice(),
            b"d4:infod6:lengthi1e4:name3:a\\bee".as_slice(),
        ] {
            assert!(inspect_metainfo(torrent).is_err());
        }
        assert!(inspect_metainfo(&vec![b'x'; MAX_INPUT_SIZE + 1]).is_err());
    }
}
