use std::{
    collections::HashSet,
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use rand::{RngCore, rngs::OsRng};

use crate::StartupError;

pub const ACCESS_TTL: i64 = 900;
pub const SESSION_TTL: i64 = 7 * 24 * 60 * 60;

#[derive(Clone)]
pub struct AuthConfig {
    pub(crate) secret: Arc<Vec<u8>>,
    pub(crate) trusted_origins: Arc<Vec<String>>,
    pub(crate) cookie_secure: bool,
}

impl AuthConfig {
    pub fn new(
        secret: impl AsRef<[u8]>,
        trusted_origins: Vec<String>,
        cookie_secure: bool,
    ) -> Result<Self, StartupError> {
        let secret = secret.as_ref();
        validate_secret(secret)?;
        if trusted_origins.is_empty() || trusted_origins.iter().any(|origin| !valid_origin(origin))
        {
            return Err(
                "TRUSTED_ORIGINS must contain exact http(s) origins without paths or wildcards"
                    .into(),
            );
        }
        if !cookie_secure && trusted_origins.iter().any(|origin| !local_origin(origin)) {
            return Err(
                "COOKIE_SECURE=false is allowed only with loopback development origins".into(),
            );
        }
        Ok(Self {
            secret: Arc::new(secret.to_vec()),
            trusted_origins: Arc::new(trusted_origins),
            cookie_secure,
        })
    }

    pub fn from_env() -> Result<Self, StartupError> {
        let secret = match (env::var_os("JWT_SECRET"), env::var_os("JWT_SECRET_FILE")) {
            (Some(_), Some(_)) => {
                return Err("configure JWT_SECRET or JWT_SECRET_FILE, not both".into());
            }
            (Some(value), None) => value
                .into_string()
                .map_err(|_| "JWT_SECRET must be UTF-8")?
                .into_bytes(),
            (None, path) => load_or_create_secret(
                &path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(".data/jwt.key")),
            )?,
        };
        let secure = match env::var("COOKIE_SECURE").as_deref() {
            Ok("false") => false,
            Ok("true") | Err(env::VarError::NotPresent) => true,
            _ => return Err("COOKIE_SECURE must be true or false".into()),
        };
        let origins = env::var("TRUSTED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173,http://localhost:8080,http://127.0.0.1:8080".into())
            .split(',').map(|value| value.trim().to_owned()).collect();
        Self::new(secret, origins, secure)
    }

    /// A fresh key for compatibility with the original synthetic-route tests.
    /// Production startup always uses `from_env`, which persists its key.
    pub(crate) fn ephemeral() -> Self {
        let mut secret = [0; 32];
        OsRng.fill_bytes(&mut secret);
        Self::new(secret, vec!["http://localhost:5173".into()], true).expect("random test key")
    }
}

fn valid_origin(origin: &str) -> bool {
    let Some(authority) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    !authority.is_empty()
        && !authority
            .chars()
            .any(|c| c.is_whitespace() || "/?#@*\\".contains(c))
}

fn local_origin(origin: &str) -> bool {
    let Some(authority) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
        authority == *host
            || authority
                .strip_prefix(host)
                .is_some_and(|rest| rest.starts_with(':') && rest[1..].parse::<u16>().is_ok())
    })
}

fn validate_secret(secret: &[u8]) -> Result<(), StartupError> {
    if !(32..=4096).contains(&secret.len())
        || secret.iter().copied().collect::<HashSet<_>>().len() < 8
    {
        return Err(
            "JWT key must contain at least 32 bytes and must not be a weak/repeated value".into(),
        );
    }
    Ok(())
}

fn load_or_create_secret(path: &Path) -> Result<Vec<u8>, StartupError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        crate::create_private_directory(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            let mut secret = vec![0; 32];
            OsRng.fill_bytes(&mut secret);
            file.write_all(&secret)?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err("JWT key must be a regular file of at most 4096 bytes".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(
                "JWT key file must not be readable or writable by group/others; use chmod 600"
                    .into(),
            );
        }
    }
    let mut secret = Vec::new();
    fs::File::open(path)?.take(4097).read_to_end(&mut secret)?;
    validate_secret(&secret)?;
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_persisted_private_and_weak_keys_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("keys/jwt.key");
        let first = load_or_create_secret(&path).unwrap();
        assert_eq!(first, load_or_create_secret(&path).unwrap());
        assert_eq!(first.len(), 32);
        assert!(
            AuthConfig::new(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                vec!["https://example.com".into()],
                true
            )
            .is_err()
        );
        assert!(AuthConfig::new(first, vec!["http://example.com".into()], false).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
