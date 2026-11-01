use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::Path};
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct AddonPreview {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub logo: Option<String>,
    pub origin: String,
    pub credential_present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub logo: Option<String>,
    pub origin: String,
    pub credential_present: bool,
    pub enabled: bool,
    pub sort_order: i64,
    pub stream_types: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct StoredAddon {
    pub public: AddonRecord,
    pub encrypted_config_url: String,
}
impl StoredAddon {
    pub fn supports(&self, media_type: &str) -> bool {
        self.public
            .stream_types
            .as_ref()
            .is_none_or(|types| types.iter().any(|kind| kind == media_type))
    }
}
pub struct AddonRegistry {
    cipher: Aes256Gcm,
}
impl AddonRegistry {
    pub fn load_or_create() -> anyhow::Result<Self> {
        if let Ok(raw) = std::env::var("FLASHARR_ADDON_CONFIG_KEY") {
            return Self::from_key(Some(&raw));
        }
        Self::from_key(Some(&load_or_create_key(&addon_key_path())?))
    }
    pub(crate) fn load_or_create_at(path: &Path) -> anyhow::Result<Self> {
        Self::from_key(Some(&load_or_create_key(path)?))
    }
    pub(crate) fn from_key(raw: Option<&str>) -> anyhow::Result<Self> {
        let raw = raw.ok_or_else(|| anyhow::anyhow!("FLASHARR_ADDON_CONFIG_KEY is required"))?;
        let key = decode_key(raw)?;
        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&key).expect("validated AES-256 key"),
        })
    }
    pub fn encrypt(&self, url: &Url) -> anyhow::Result<String> {
        self.encrypt_bytes(url.as_str().as_bytes())
    }
    pub fn decrypt(&self, encrypted_config_url: &str) -> anyhow::Result<Url> {
        let plaintext = self.decrypt_bytes(encrypted_config_url)?;
        Ok(Url::parse(std::str::from_utf8(&plaintext)?)?)
    }
    pub fn encrypt_secret(&self, scope: &str, value: &str) -> anyhow::Result<String> {
        let plaintext = [scope.as_bytes(), &[0], value.as_bytes()].concat();
        self.encrypt_bytes(&plaintext)
    }
    pub fn decrypt_secret(&self, scope: &str, ciphertext: &str) -> anyhow::Result<String> {
        let plaintext = self.decrypt_bytes(ciphertext)?;
        let prefix = [scope.as_bytes(), &[0]].concat();
        Ok(String::from_utf8(
            plaintext
                .strip_prefix(prefix.as_slice())
                .ok_or_else(|| anyhow::anyhow!("secret scope mismatch"))?
                .to_vec(),
        )?)
    }
    fn encrypt_bytes(&self, plaintext: &[u8]) -> anyhow::Result<String> {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        out.extend(
            self.cipher
                .encrypt(Nonce::from_slice(&nonce), plaintext)
                .map_err(|_| anyhow::anyhow!("encryption failed"))?,
        );
        Ok(hex(&out))
    }
    fn decrypt_bytes(&self, ciphertext: &str) -> anyhow::Result<Vec<u8>> {
        let b = unhex(ciphertext)?;
        if b.len() < 13 {
            anyhow::bail!("invalid ciphertext")
        }
        self.cipher
            .decrypt(Nonce::from_slice(&b[..12]), &b[12..])
            .map_err(|_| anyhow::anyhow!("decryption failed"))
    }
}

fn addon_key_path() -> std::path::PathBuf {
    crate::config::get_appdata_dir()
        .join("data")
        .join("stremio-addon.key")
}

fn load_or_create_key(path: &Path) -> anyhow::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => {
            let raw = raw.trim().to_owned();
            decode_key(&raw)?;
            Ok(raw)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut key = [0_u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            let raw = hex(&key);
            let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return load_or_create_key(path)
                }
                Err(error) => return Err(error.into()),
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(raw.as_bytes())?;
            file.sync_all()?;
            Ok(raw)
        }
        Err(error) => Err(error.into()),
    }
}

fn decode_key(s: &str) -> anyhow::Result<Vec<u8>> {
    let b = unhex(s)?;
    if b.len() != 32 {
        anyhow::bail!("key must be 32-byte hex")
    }
    Ok(b)
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn unhex(s: &str) -> anyhow::Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(anyhow::anyhow!("invalid hex"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(Into::into))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stored(stream_types: Option<Vec<&str>>) -> StoredAddon {
        StoredAddon {
            public: AddonRecord {
                id: "addon".into(),
                name: "Addon".into(),
                version: "1.0.0".into(),
                description: "Addon description".into(),
                logo: None,
                origin: "https://addon.example".into(),
                credential_present: false,
                enabled: true,
                sort_order: 0,
                stream_types: stream_types
                    .map(|types| types.into_iter().map(str::to_owned).collect()),
            },
            encrypted_config_url: "ciphertext".into(),
        }
    }

    #[test]
    fn unknown_capabilities_fail_open_but_known_capabilities_filter_strictly() {
        let unknown = stored(None);
        assert!(unknown.supports("movie"));
        assert!(unknown.supports("series"));

        let series = stored(Some(vec!["series"]));
        assert!(!series.supports("movie"));
        assert!(series.supports("series"));

        let movie = stored(Some(vec!["movie"]));
        assert!(movie.supports("movie"));
        assert!(!movie.supports("series"));

        let unsupported = stored(Some(vec![]));
        assert!(!unsupported.supports("movie"));
        assert!(!unsupported.supports("series"));
    }
    #[test]
    fn encrypted_config_url_roundtrips_without_plaintext() {
        let r = AddonRegistry {
            cipher: Aes256Gcm::new_from_slice(&[7; 32]).unwrap(),
        };
        let u = Url::parse("https://addon.example/secret/manifest.json").unwrap();
        let e = r.encrypt(&u).unwrap();
        assert!(!e.contains("secret"));
        assert_eq!(r.decrypt(&e).unwrap(), u);
    }

    #[test]
    fn decrypt_rejects_tamper_wrong_key_and_malformed_ciphertext_without_panicking() {
        let r = AddonRegistry::from_key(Some(&"07".repeat(32))).unwrap();
        let other = AddonRegistry::from_key(Some(&"08".repeat(32))).unwrap();
        let url = Url::parse("https://addon.example/secret/manifest.json").unwrap();
        let encrypted = r.encrypt(&url).unwrap();
        let mut tampered = encrypted.clone().into_bytes();
        let last = tampered.last_mut().unwrap();
        *last = if *last == b'0' { b'1' } else { b'0' };

        assert!(r.decrypt(std::str::from_utf8(&tampered).unwrap()).is_err());
        assert!(other.decrypt(&encrypted).is_err());
        for malformed in ["", "00", "abc", "zz", &"00".repeat(12)] {
            assert!(r.decrypt(malformed).is_err(), "accepted {malformed:?}");
        }
    }

    #[test]
    fn key_rejects_short_odd_and_nonhex_values() {
        for malformed in ["", "00", "abc", &"zz".repeat(32)] {
            assert!(
                AddonRegistry::from_key(Some(malformed)).is_err(),
                "accepted {malformed:?}"
            );
        }
        assert!(AddonRegistry::from_key(None).is_err());
    }

    #[test]
    fn scoped_secrets_roundtrip_and_reject_cross_scope_decryption() {
        let registry = AddonRegistry::from_key(Some(&"07".repeat(32))).unwrap();
        let ciphertext = registry
            .encrypt_secret("metadata:tmdb", "secret-key")
            .unwrap();
        assert!(!ciphertext.contains("secret-key"));
        assert_eq!(
            registry
                .decrypt_secret("metadata:tmdb", &ciphertext)
                .unwrap(),
            "secret-key"
        );
        assert!(registry
            .decrypt_secret("metadata:tvdb", &ciphertext)
            .is_err());
    }

    #[test]
    fn generated_key_is_persisted_and_reused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("stremio-addon.key");
        let first = AddonRegistry::load_or_create_at(&path).unwrap();
        let url = Url::parse("https://addon.example/private/manifest.json").unwrap();
        let encrypted = first.encrypt(&url).unwrap();
        let second = AddonRegistry::load_or_create_at(&path).unwrap();
        assert_eq!(second.decrypt(&encrypted).unwrap(), url);
        assert_eq!(std::fs::read_to_string(path).unwrap().len(), 64);
    }
}
