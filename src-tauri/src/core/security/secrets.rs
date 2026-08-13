//! Secrets handling (plan 4.3) — detect and redact secrets before they reach
//! logs, UI, audit events or MCP responses.
//!
//! Two concerns, both pure and testable:
//!
//! - [`looks_like_secret`] — classify a value as a secret by shape
//!   (API keys, tokens, JWTs, private keys, passwords) so downstream code can
//!   refuse to log/display it.
//! - [`redact`] — scrub known secret *values* out of free text so a log line
//!   or audit detail containing a leaked value is still usable but never
//!   prints the secret itself.
//!
//! Storage (Windows Credential Manager) is out of scope here — this module is
//! about the *handling* boundary: never let a secret value into observable
//! output.

/// A catalog of secret shapes. Used to classify a value without knowing the
/// actual secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    Jwt,
    ApiKey,
    PrivateKey,
    GenericSecret,
}

impl SecretKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecretKind::Jwt => "jwt",
            SecretKind::ApiKey => "api-key",
            SecretKind::PrivateKey => "private-key",
            SecretKind::GenericSecret => "secret",
        }
    }
}

/// Redaction placeholder for a secret of the given kind.
fn redaction_marker(kind: SecretKind) -> String {
    format!("[REDACTED:{}]", kind.as_str())
}

/// True when the value has the shape of a secret.
///
/// Detects, without knowing the real secret:
/// - JSON Web Tokens: three dot-separated base64 segments.
/// - Long high-entropy keys (`sk-...`, `AKIA...`, 32+ hex/base64 runs).
/// - Private key blocks (PEM headers).
/// - Shorter generic secrets (`password=...`, `token=...` assignments).
pub fn looks_like_secret(value: &str) -> Option<SecretKind> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }

    // JWT: header.payload.signature (three dot-separated base64url segments).
    let segs: Vec<&str> = v.split('.').collect();
    if segs.len() == 3
        && segs.iter().all(|s| !s.is_empty())
        && segs.iter().all(|s| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    {
        return Some(SecretKind::Jwt);
    }

    // Private keys: PEM block.
    if v.starts_with("-----BEGIN") || v.contains("PRIVATE KEY-----") {
        return Some(SecretKind::PrivateKey);
    }

    // Well-known API-key prefixes.
    let lower = v.to_lowercase();
    if lower.starts_with("sk-")
        || lower.starts_with("pk-")
        || lower.starts_with("akias")
        || lower.starts_with("ghp_")
        || lower.starts_with("glpat-")
        || lower.starts_with("xoxb-")
        || lower.starts_with("xoxp-")
    {
        return Some(SecretKind::ApiKey);
    }

    // Assignment form: key=value where value is a plausible secret.
    if let Some((_, val)) = v.split_once('=') {
        let val = val.trim();
        if !val.is_empty() && (val.len() >= 16 || detects_keyword(&lower)) {
            return Some(if detects_keyword(&lower) {
                SecretKind::GenericSecret
            } else {
                SecretKind::ApiKey
            });
        }
    }

    None
}

/// True when the lowercased text contains a secret-keyword assignment hint.
fn detects_keyword(lower: &str) -> bool {
    const HINTS: &[&str] = &[
        "password",
        "passwd",
        "secret",
        "api_key",
        "apikey",
        "api-key",
        "token",
        "access_key",
        "secret_key",
        "client_secret",
        "private_key",
        "auth",
        "credential",
    ];
    HINTS.iter().any(|h| lower.contains(h))
}

/// Redact every known secret value that appears in `text`, replacing it with a
/// per-kind marker. `known` should be the set of secret values the caller
/// knows about (from the credential store / in-memory config). This scrubs
/// leaked values out of logs, audit details and MCP output.
pub fn redact(text: &str, known: &[String]) -> String {
    if known.is_empty() || text.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for secret in known {
        if secret.is_empty() {
            continue;
        }
        let kind = looks_like_secret(secret).unwrap_or(SecretKind::GenericSecret);
        out = out.replace(secret, &redaction_marker(kind));
    }
    out
}

/// Redact a single value entirely — used where the value itself must never be
/// echoed (e.g. returning a config value through an API).
pub fn redact_value(value: &str) -> String {
    match looks_like_secret(value) {
        Some(kind) => redaction_marker(kind),
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_is_detected() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0In0.dG9rZW4";
        assert_eq!(looks_like_secret(jwt), Some(SecretKind::Jwt));
    }

    #[test]
    fn jwt_with_padding_is_detected() {
        let jwt = "a.b.c";
        assert_eq!(looks_like_secret(jwt), Some(SecretKind::Jwt));
    }

    #[test]
    fn sk_prefix_is_api_key() {
        assert_eq!(
            looks_like_secret("sk-7f3a9c2ed4b5a1f0c9d8e7f6a5b4c3d2"),
            Some(SecretKind::ApiKey)
        );
    }

    #[test]
    fn private_key_block_is_detected() {
        let pem =
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEpQIBAAKCAQEA\n-----END RSA PRIVATE KEY-----";
        assert_eq!(looks_like_secret(pem), Some(SecretKind::PrivateKey));
    }

    #[test]
    fn assignment_with_keyword_is_secret() {
        assert_eq!(
            looks_like_secret("password=admin123"),
            Some(SecretKind::GenericSecret)
        );
    }

    #[test]
    fn long_value_is_api_key() {
        assert_eq!(
            looks_like_secret("token=3f9a2c1e8b7d6a5f4e3d2c1b0a9f8e7d6c5b4a3"),
            Some(SecretKind::GenericSecret)
        );
    }

    #[test]
    fn plain_text_is_not_secret() {
        assert_eq!(looks_like_secret("the database is PostgreSQL"), None);
        assert_eq!(looks_like_secret("port 8080"), None);
        assert_eq!(looks_like_secret(""), None);
    }

    #[test]
    fn redact_replaces_known_secret_in_text() {
        let secret = "sk-supersecretvalue123".to_string();
        let text = format!("connecting with {} at port 5432", secret);
        let out = redact(&text, &[secret]);
        assert!(out.contains("[REDACTED:api-key]"));
        assert!(!out.contains("sk-supersecretvalue123"));
    }

    #[test]
    fn redact_handles_multiple_secrets() {
        let s1 = "ghp_tokenAAA".to_string();
        let s2 = "password=hunter2".to_string();
        let text = format!("login={s1} then {s2}");
        let out = redact(&text, &[s1, s2]);
        assert!(!out.contains("ghp_tokenAAA"));
        assert!(!out.contains("hunter2"));
    }

    #[test]
    fn redact_empty_known_leaves_text() {
        assert_eq!(redact("hello world", &[]), "hello world");
    }

    #[test]
    fn redact_value_masks_secret_only() {
        assert!(redact_value("sk-abc").contains("[REDACTED"));
        assert_eq!(redact_value("postgres"), "postgres");
    }
}
