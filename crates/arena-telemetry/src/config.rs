//! Where telemetry goes: the OTLP endpoint and its credentials, from the environment or a file, never from the repo.
//!
//! - `ARENA_OTLP_ENDPOINT`: the OTLP/HTTP base, e.g. `http://127.0.0.1:5080/api/default` (OpenObserve's organisation
//!   `default`); `/v1/logs`, `/v1/traces` and `/v1/metrics` are added to it. Unset or empty: telemetry stays local.
//! - `ARENA_OTLP_AUTH`: the credentials, either a whole header value (`Basic …`) or `user:password`.
//! - `ARENA_OTLP_AUTH_FILE`: a `KEY=VALUE` file holding `ARENA_OTLP_AUTH`, or OpenObserve's own `ZO_ROOT_USER_EMAIL`
//!   and `ZO_ROOT_USER_PASSWORD` (the store's `observe.env`). Read when `ARENA_OTLP_AUTH` is unset.

/// An OTLP/HTTP endpoint and the `Authorization` header it takes.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The base URL, without a trailing slash.
    pub url: String,
    /// The `Authorization` header's value, if any.
    pub auth: Option<String>,
}

impl std::fmt::Debug for Endpoint {
    // the credentials never reach a log
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("url", &self.url)
            .field("auth", &self.auth.as_ref().map(|_| "<set>"))
            .finish()
    }
}

impl Endpoint {
    /// The endpoint the environment names (`ARENA_OTLP_ENDPOINT` and its credentials), or `None` when unset.
    pub fn from_env() -> Result<Option<Endpoint>, String> {
        Endpoint::from_vars(&|k| std::env::var(k).ok(), &|p| {
            std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
        })
    }

    /// As `from_env`, over any variables and file reader (tests).
    pub fn from_vars(
        var: &dyn Fn(&str) -> Option<String>,
        read: &dyn Fn(&str) -> Result<String, String>,
    ) -> Result<Option<Endpoint>, String> {
        let Some(url) = var("ARENA_OTLP_ENDPOINT").filter(|u| !u.trim().is_empty()) else {
            return Ok(None);
        };
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(format!("ARENA_OTLP_ENDPOINT is not an http(s) URL: {url}"));
        }
        let auth = match var("ARENA_OTLP_AUTH").filter(|a| !a.trim().is_empty()) {
            Some(a) => Some(header(&a)),
            None => match var("ARENA_OTLP_AUTH_FILE").filter(|p| !p.trim().is_empty()) {
                Some(path) => Some(auth_from_file(&read(path.trim())?).ok_or_else(|| {
                    format!(
                        "{path}: no ARENA_OTLP_AUTH, nor ZO_ROOT_USER_EMAIL and ZO_ROOT_USER_PASSWORD"
                    )
                })?),
                None => None,
            },
        };
        Ok(Some(Endpoint { url, auth }))
    }

    /// The URL of one signal (`logs`, `traces`, `metrics`).
    pub fn signal(&self, signal: &str) -> String {
        format!("{}/v1/{signal}", self.url)
    }
}

/// The `Authorization` header for `auth`: kept as it is when it already names a scheme, else `user:password` as Basic.
pub fn header(auth: &str) -> String {
    let auth = auth.trim();
    if auth.starts_with("Basic ") || auth.starts_with("Bearer ") {
        auth.to_string()
    } else {
        format!("Basic {}", base64(auth.as_bytes()))
    }
}

/// The header from a `KEY=VALUE` file: `ARENA_OTLP_AUTH`, else OpenObserve's root user.
pub fn auth_from_file(text: &str) -> Option<String> {
    let value = |key: &str| {
        text.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| k.trim().trim_start_matches("export ").trim() == key)
            .map(|(_, v)| v.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|v| !v.is_empty())
    };
    if let Some(a) = value("ARENA_OTLP_AUTH") {
        return Some(header(&a));
    }
    let (user, pass) = (
        value("ZO_ROOT_USER_EMAIL")?,
        value("ZO_ROOT_USER_PASSWORD")?,
    );
    Some(header(&format!("{user}:{pass}")))
}

/// Standard base64 with padding (RFC 4648), for a Basic header.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(A[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
    }

    #[test]
    fn a_header_is_kept_and_a_pair_becomes_basic() {
        assert_eq!(header("Basic abc"), "Basic abc");
        assert_eq!(header("a@b.c:pw"), format!("Basic {}", base64(b"a@b.c:pw")));
    }

    #[test]
    fn unset_is_off_and_the_file_gives_the_root_user() -> Result<(), String> {
        let none = |_: &str| None;
        let no_file = |p: &str| Err(format!("no file {p}"));
        assert_eq!(Endpoint::from_vars(&none, &no_file)?, None);

        let vars = |k: &str| match k {
            "ARENA_OTLP_ENDPOINT" => Some("http://127.0.0.1:5080/api/default/".to_string()),
            "ARENA_OTLP_AUTH_FILE" => Some("/x/observe.env".to_string()),
            _ => None,
        };
        let file = |_: &str| {
            Ok(
                "# the store\nZO_ROOT_USER_EMAIL=root@x.io\nZO_ROOT_USER_PASSWORD=\"s3cret\"\n"
                    .to_string(),
            )
        };
        let e = Endpoint::from_vars(&vars, &file)?.ok_or("configured")?;
        assert_eq!(e.url, "http://127.0.0.1:5080/api/default");
        assert_eq!(
            e.signal("logs"),
            "http://127.0.0.1:5080/api/default/v1/logs"
        );
        assert_eq!(
            e.auth,
            Some(format!("Basic {}", base64(b"root@x.io:s3cret")))
        );
        assert!(
            !format!("{e:?}").contains("Basic"),
            "credentials never in Debug"
        );
        Ok(())
    }

    #[test]
    fn a_bad_url_or_an_empty_file_is_an_error() {
        let bad = |k: &str| (k == "ARENA_OTLP_ENDPOINT").then(|| "127.0.0.1:5080".to_string());
        assert!(Endpoint::from_vars(&bad, &|_| Ok(String::new())).is_err());
        let vars = |k: &str| match k {
            "ARENA_OTLP_ENDPOINT" => Some("http://h".to_string()),
            "ARENA_OTLP_AUTH_FILE" => Some("f".to_string()),
            _ => None,
        };
        assert!(Endpoint::from_vars(&vars, &|_| Ok("NOTHING=1".to_string())).is_err());
    }
}
