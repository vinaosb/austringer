//! The provider seam as configuration: family parsing, validation, and
//! key resolution through the named environment variable.

use std::sync::Arc;
use std::time::Duration;

use harness_providers::auth::{CredentialSource, NoCredentials, StaticApiKey};

use super::compose::ResolvedProvider;

/// Provider wire families the daemon can compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFamily {
    /// Anthropic Messages API.
    Anthropic,
    /// OpenAI chat completions (public endpoint).
    OpenAi,
    /// Google Gemini.
    Google,
    /// xAI Grok (chat-completions wire).
    Xai,
    /// Any OpenAI-compatible base URL (Ollama/vLLM/OpenRouter/...).
    Compat,
}

impl ProviderFamily {
    /// The wire families `HARNESS_PROVIDER` accepts.
    pub const KNOWN: [&'static str; 5] = ["anthropic", "openai", "google", "xai", "compat"];

    fn parse(value: &str) -> Result<Self, ProviderSetupError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAi),
            "google" => Ok(Self::Google),
            "xai" => Ok(Self::Xai),
            "compat" => Ok(Self::Compat),
            _ => Err(ProviderSetupError::UnknownFamily {
                value: value.to_owned(),
            }),
        }
    }

    fn requires_api_key(self) -> bool {
        !matches!(self, Self::Compat)
    }

    pub(crate) fn canonical(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Google => "google",
            Self::Xai => "xai",
            Self::Compat => "compat",
        }
    }
}

/// Every provider-configuration failure, typed and actionable.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProviderSetupError {
    /// `HARNESS_PROVIDER` held an unknown family name.
    #[error(
        "unknown provider family {value:?} in HARNESS_PROVIDER; \
         expected one of {known}",
        known = ProviderFamily::KNOWN.join(", ")
    )]
    UnknownFamily {
        /// The rejected value.
        value: String,
    },
    /// `HARNESS_MODEL` was absent or blank.
    #[error("HARNESS_MODEL is required when HARNESS_PROVIDER is set (it must name the model id)")]
    MissingModel,
    /// A keyed family was configured without `HARNESS_PROVIDER_KEY_ENV`.
    #[error(
        "provider {family} requires HARNESS_PROVIDER_KEY_ENV naming the \
         environment variable that holds its API key (never the key itself)"
    )]
    MissingKeyEnvName {
        /// The family that demanded a key.
        family: &'static str,
    },
    /// The compat family was configured without a base URL.
    #[error(
        "provider compat requires HARNESS_PROVIDER_BASE_URL (everything up to, but excluding, /chat/completions)"
    )]
    MissingBaseUrl,
    /// The named key variable is not set in the environment.
    #[error(
        "environment variable {env_var} (named by HARNESS_PROVIDER_KEY_ENV) \
         is not set; export the API key under that name and restart"
    )]
    KeyEnvUnset {
        /// The environment variable that was expected to hold the key.
        env_var: String,
    },
}

/// The provider seam as parsed from configuration: family, model, and
/// the NAME of the environment variable holding the API key (the value
/// is read at resolve time and never persisted).
#[derive(Debug, Clone)]
pub struct ProviderSelection {
    /// Wire family to compose.
    pub family: ProviderFamily,
    /// Model id handed to the adapter.
    pub model: String,
    /// Environment variable name holding the API key (`None` = the
    /// no-credential source, legal only for local compat endpoints).
    pub api_key_env: Option<String>,
    /// Base-URL override (required for compat).
    pub base_url: Option<String>,
    /// Wall-clock budget for one inference call; a provider that
    /// exceeds it surfaces a typed timeout instead of hanging the turn.
    pub inference_timeout: Duration,
}

impl ProviderSelection {
    /// Parses and validates the raw config values (the binary feeds the
    /// environment strings; tests feed literals). Blank model, unknown
    /// families, and family/key/base-URL mismatches are rejected with
    /// actionable typed errors before anything is composed.
    pub fn from_parts(
        family: &str,
        model: Option<&str>,
        api_key_env: Option<&str>,
        base_url: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, ProviderSetupError> {
        let family_kind = ProviderFamily::parse(family)?;
        let model = model.map(str::trim).filter(|m| !m.is_empty());
        let Some(model) = model else {
            return Err(ProviderSetupError::MissingModel);
        };
        if family_kind.requires_api_key() && api_key_env.map(str::trim).is_none() {
            return Err(ProviderSetupError::MissingKeyEnvName {
                family: family_kind.canonical(),
            });
        }
        if matches!(family_kind, ProviderFamily::Compat) && base_url.map(str::trim).is_none() {
            return Err(ProviderSetupError::MissingBaseUrl);
        }
        Ok(Self {
            family: family_kind,
            model: model.to_owned(),
            api_key_env: trimmed(api_key_env),
            base_url: trimmed(base_url),
            inference_timeout: Duration::from_secs(timeout_secs.unwrap_or(300)),
        })
    }

    /// Resolves the key through the real environment.
    ///
    /// # Errors
    /// [`ProviderSetupError::KeyEnvUnset`] when the named variable is
    /// absent or blank.
    pub fn resolve(&self) -> Result<ResolvedProvider, ProviderSetupError> {
        self.resolve_with(|name| std::env::var(name).ok())
    }

    /// Resolves the key through an injected lookup (tests; the shipped
    /// binary uses [`Self::resolve`]).
    ///
    /// # Errors
    /// [`ProviderSetupError::KeyEnvUnset`] when the named variable
    /// yields no non-blank value.
    pub fn resolve_with(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<ResolvedProvider, ProviderSetupError> {
        let credentials = match &self.api_key_env {
            Some(env_var) => {
                let key = lookup(env_var)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty());
                match key {
                    Some(key) => self.static_key(&key),
                    None => {
                        return Err(ProviderSetupError::KeyEnvUnset {
                            env_var: env_var.clone(),
                        });
                    }
                }
            }
            None => Arc::new(NoCredentials),
        };
        Ok(ResolvedProvider::new(
            self.family,
            self.model.clone(),
            credentials,
            self.base_url.clone(),
            self.inference_timeout,
        ))
    }

    fn static_key(&self, key: &str) -> Arc<dyn CredentialSource> {
        match self.family {
            ProviderFamily::Anthropic => Arc::new(StaticApiKey::x_api_key(key)),
            ProviderFamily::Google => Arc::new(StaticApiKey::google_api_key(key)),
            ProviderFamily::OpenAi | ProviderFamily::Xai | ProviderFamily::Compat => {
                Arc::new(StaticApiKey::bearer(key))
            }
        }
    }
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
