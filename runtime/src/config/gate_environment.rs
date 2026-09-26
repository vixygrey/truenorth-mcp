//! Environment inheritance policy for bounded gate execution.
//!
//! Gate subprocesses inherit only a small set of exact variable names. Operators can add
//! safe names through [`GATE_ENV_ALLOWLIST_ENV`]. Credential-like and TrueNorth runtime
//! configuration names are rejected before a subprocess starts.

use std::collections::BTreeSet;
use std::fmt;

/// Environment variable that adds comma-separated names to the gate environment allowlist.
pub const GATE_ENV_ALLOWLIST_ENV: &str = "TRUENORTH_GATE_ENV_ALLOWLIST";

/// Environment names inherited by every gate subprocess when present.
pub const DEFAULT_GATE_ENV_NAMES: [&str; 5] = ["PATH", "HOME", "TMPDIR", "TMP", "TEMP"];

/// Validated environment inheritance policy for a gate subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateEnvironmentPolicy {
    allowed_names: Vec<String>,
    configured_names: Vec<String>,
}

impl GateEnvironmentPolicy {
    /// Resolve the policy from the current process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured value is not Unicode, contains an invalid
    /// environment name, or requests a credential-like or reserved name.
    pub fn from_process() -> Result<Self, GateEnvironmentError> {
        let configured = match std::env::var(GATE_ENV_ALLOWLIST_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(GateEnvironmentError::non_unicode());
            }
        };
        Self::parse(configured.as_deref())
    }

    /// Parse an optional comma-separated list of additional environment names.
    ///
    /// # Errors
    ///
    /// Returns every invalid and credential-like name without inspecting any associated
    /// environment value.
    pub fn parse(configured: Option<&str>) -> Result<Self, GateEnvironmentError> {
        let configured_names: Vec<String> = configured
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let invalid_names: Vec<String> = configured_names
            .iter()
            .filter(|name| !is_portable_environment_name(name))
            .cloned()
            .collect();
        let credential_names: Vec<String> = configured_names
            .iter()
            .filter(|name| is_portable_environment_name(name))
            .filter(|name| is_credential_like_name(name))
            .cloned()
            .collect();

        if !invalid_names.is_empty() || !credential_names.is_empty() {
            return Err(GateEnvironmentError {
                invalid_names,
                credential_names,
                non_unicode: false,
            });
        }

        let mut allowed_names: Vec<String> = DEFAULT_GATE_ENV_NAMES
            .into_iter()
            .map(str::to_string)
            .collect();
        for name in &configured_names {
            if !allowed_names.contains(name) {
                allowed_names.push(name.clone());
            }
        }

        Ok(Self {
            allowed_names,
            configured_names,
        })
    }

    /// Effective inherited names, including defaults and explicit additions.
    pub fn allowed_names(&self) -> &[String] {
        &self.allowed_names
    }

    /// Explicit operator additions, excluding built-in defaults.
    pub fn configured_names(&self) -> &[String] {
        &self.configured_names
    }
}

/// Invalid bounded-executor environment configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateEnvironmentError {
    invalid_names: Vec<String>,
    credential_names: Vec<String>,
    non_unicode: bool,
}

impl GateEnvironmentError {
    fn non_unicode() -> Self {
        Self {
            invalid_names: Vec::new(),
            credential_names: Vec::new(),
            non_unicode: true,
        }
    }

    /// Names that do not use the portable environment-name syntax.
    pub fn invalid_names(&self) -> &[String] {
        &self.invalid_names
    }

    /// Names rejected as credentials or reserved runtime configuration.
    pub fn credential_names(&self) -> &[String] {
        &self.credential_names
    }
}

impl fmt::Display for GateEnvironmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.non_unicode {
            return write!(
                formatter,
                "{GATE_ENV_ALLOWLIST_ENV} must contain a comma-separated UTF-8 list of environment variable names"
            );
        }

        let mut problems = Vec::new();
        if !self.invalid_names.is_empty() {
            problems.push(format!("invalid names: {}", self.invalid_names.join(", ")));
        }
        if !self.credential_names.is_empty() {
            problems.push(format!(
                "credential-like or reserved names: {}",
                self.credential_names.join(", ")
            ));
        }
        write!(
            formatter,
            "{GATE_ENV_ALLOWLIST_ENV} contains {}; use portable non-credential names only",
            problems.join("; ")
        )
    }
}

impl std::error::Error for GateEnvironmentError {}

fn is_portable_environment_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }
    bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn is_credential_like_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if upper.starts_with("TRUENORTH_") {
        return true;
    }
    if matches!(
        upper.as_str(),
        "AWS_ACCESS_KEY_ID"
            | "GOOGLE_APPLICATION_CREDENTIALS"
            | "DOCKER_AUTH_CONFIG"
            | "SSH_AUTH_SOCK"
            | "SSH_AGENT_PID"
            | "DATABASE_URL"
    ) {
        return true;
    }

    let segments: Vec<&str> = upper.split('_').collect();
    segments.iter().any(|segment| {
        matches!(
            *segment,
            "TOKEN" | "SECRET" | "PASSWORD" | "CREDENTIAL" | "CREDENTIALS" | "JWT" | "AUTH"
        )
    }) || upper.contains("API_KEY")
        || upper.contains("ACCESS_KEY")
        || upper.contains("PRIVATE_KEY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_minimal_and_exact() {
        let policy = GateEnvironmentPolicy::parse(None).expect("default policy");
        assert_eq!(
            policy.allowed_names(),
            ["PATH", "HOME", "TMPDIR", "TMP", "TEMP"]
        );
        assert!(policy.configured_names().is_empty());
    }

    #[test]
    fn safe_names_are_trimmed_sorted_and_deduplicated() {
        let policy = GateEnvironmentPolicy::parse(Some(" JAVA_HOME,CARGO_HOME,JAVA_HOME "))
            .expect("safe additions");
        assert_eq!(policy.configured_names(), ["CARGO_HOME", "JAVA_HOME"]);
        assert!(policy.allowed_names().contains(&"CARGO_HOME".to_string()));
        assert!(policy.allowed_names().contains(&"JAVA_HOME".to_string()));
    }

    #[test]
    fn invalid_names_are_rejected_together() {
        let error = GateEnvironmentPolicy::parse(Some("GOOD,bad-name,9START"))
            .expect_err("invalid names fail");
        assert_eq!(error.invalid_names(), ["9START", "bad-name"]);
        assert!(error.credential_names().is_empty());
        assert!(!error.to_string().contains("GOOD"));
    }

    #[test]
    fn common_credential_classes_are_rejected() {
        for name in [
            "GITHUB_TOKEN",
            "NPM_TOKEN",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "CI_JOB_TOKEN",
            "SSH_AUTH_SOCK",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "TRUENORTH_VERIFY_CMD",
        ] {
            let error = GateEnvironmentPolicy::parse(Some(name))
                .expect_err("credential-like name must fail");
            assert_eq!(error.credential_names(), [name]);
            assert!(!error.to_string().contains("placeholder-secret-value"));
        }
    }
}
