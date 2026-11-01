use sha1::{Digest, Sha1};
use std::fmt::Write;

fn hex_hash(bytes: &[u8]) -> Option<String> {
    let mut hash = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hash, "{byte:02x}").ok()?;
    }
    Some(hash)
}

pub fn normalize_info_hash(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Some(value.to_ascii_lowercase());
    }
    if value.len() != 32 {
        return None;
    }

    let mut decoded = Vec::with_capacity(20);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes() {
        let digit = match byte.to_ascii_uppercase() {
            b'A'..=b'Z' => byte.to_ascii_uppercase() - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => return None,
        };
        buffer = (buffer << 5) | u32::from(digit);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            decoded.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    (decoded.len() == 20 && bits == 0)
        .then(|| hex_hash(&decoded))
        .flatten()
}

pub fn extract_btih(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let start = lower.find("btih:")? + "btih:".len();
    let tail = &value[start..];
    let end = tail.find(['&', '#']).unwrap_or(tail.len());
    normalize_info_hash(&tail[..end])
}

pub fn torrent_info_hash(bytes: &[u8]) -> Option<String> {
    let start = find_info_value_start(bytes)?;
    if bytes.get(start) != Some(&b'd') {
        return None;
    }
    let end = bencode_value_end(bytes, start)?;
    hex_hash(&Sha1::digest(&bytes[start..end]))
}

fn find_info_value_start(bytes: &[u8]) -> Option<usize> {
    if bytes.first().copied()? != b'd' {
        return None;
    }
    let mut pos = 1;
    while pos < bytes.len() && bytes[pos] != b'e' {
        let (key_start, key_end, value_start) = parse_string(bytes, pos)?;
        if &bytes[key_start..key_end] == b"info" {
            return Some(value_start);
        }
        pos = bencode_value_end(bytes, value_start)?;
    }
    None
}

fn bencode_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    bencode_value_end_at(bytes, start, 0)
}

fn bencode_value_end_at(bytes: &[u8], start: usize, depth: usize) -> Option<usize> {
    if depth > 128 || start >= bytes.len() {
        return None;
    }
    match bytes[start] {
        b'0'..=b'9' => parse_string(bytes, start).map(|(_, _, next)| next),
        b'i' => {
            let mut pos = start + 1;
            if pos >= bytes.len() {
                return None;
            }
            if bytes[pos] == b'-' {
                pos += 1;
            }
            let digits_start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            if pos == digits_start || bytes.get(pos).copied()? != b'e' {
                return None;
            }
            Some(pos + 1)
        }
        b'l' | b'd' => {
            let mut pos = start + 1;
            while pos < bytes.len() && bytes[pos] != b'e' {
                pos = bencode_value_end_at(bytes, pos, depth + 1)?;
            }
            if bytes.get(pos).copied()? == b'e' {
                Some(pos + 1)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_string(bytes: &[u8], start: usize) -> Option<(usize, usize, usize)> {
    let mut pos = start;
    if !bytes.get(pos)?.is_ascii_digit() {
        return None;
    }
    let mut len = 0usize;
    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
        len = len
            .checked_mul(10)?
            .checked_add((bytes[pos] - b'0') as usize)?;
        pos += 1;
    }
    if bytes.get(pos).copied()? != b':' {
        return None;
    }
    let payload_start = pos + 1;
    let payload_end = payload_start.checked_add(len)?;
    if payload_end > bytes.len() {
        return None;
    }
    Some((payload_start, payload_end, payload_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_exact_info_dict_slice() {
        let torrent = b"d4:infod6:lengthi123e4:name4:testee";
        assert_eq!(
            torrent_info_hash(torrent).as_deref(),
            Some("6eaf2d89d7a8e1ea1c640340f6b49e9126cc3df1")
        );
    }

    #[test]
    fn skips_other_top_level_values_before_info() {
        let torrent = b"d8:announce4:test4:infod6:lengthi123e4:name4:testee";
        assert_eq!(
            torrent_info_hash(torrent).as_deref(),
            Some("6eaf2d89d7a8e1ea1c640340f6b49e9126cc3df1")
        );
    }

    #[test]
    fn rejects_missing_or_invalid_info() {
        assert_eq!(torrent_info_hash(b"d4:name4:teste"), None);
        assert_eq!(torrent_info_hash(b"not-bencode"), None);
        assert_eq!(torrent_info_hash(b"d4:info4:fakee"), None);
    }

    #[test]
    fn normalizes_hex_and_base32_btih() {
        assert_eq!(
            normalize_info_hash("6162636465666768696A6B6C6D6E6F7071727374").as_deref(),
            Some("6162636465666768696a6b6c6d6e6f7071727374")
        );
        assert_eq!(
            normalize_info_hash("MFRGGZDFMZTWQ2LKNNWG23TPOBYXE43U").as_deref(),
            Some("6162636465666768696a6b6c6d6e6f7071727374")
        );
    }

    #[test]
    fn extracts_case_insensitive_canonical_btih() {
        assert_eq!(
            extract_btih("magnet:?xt=urn:BTIH:MFRGGZDFMZTWQ2LKNNWG23TPOBYXE43U&dn=test").as_deref(),
            Some("6162636465666768696a6b6c6d6e6f7071727374")
        );
        assert_eq!(extract_btih("magnet:?xt=urn:btih:not-a-hash"), None);
        assert_eq!(normalize_info_hash("xyz"), None);
    }
}
