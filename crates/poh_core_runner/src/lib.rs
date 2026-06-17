use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use poh_core::{Redactor, RuntimeConfig, RuntimeSecurityPolicy};
use thiserror::Error;

pub trait SecretResolver {
    fn resolve(&self, key: &str) -> Option<String>;
}

#[derive(Clone, Debug, Default)]
pub struct MapSecretResolver {
    secrets: BTreeMap<String, String>,
}

impl MapSecretResolver {
    pub fn new(secrets: BTreeMap<String, String>) -> Self {
        Self { secrets }
    }
}

impl SecretResolver for MapSecretResolver {
    fn resolve(&self, key: &str) -> Option<String> {
        self.secrets.get(key).cloned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedFile {
    pub relative_path: String,
    pub content: String,
    pub sensitive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedRuntime {
    pub files: Vec<MaterializedFile>,
    pub command_args: Vec<String>,
    pub environment: BTreeMap<String, String>,
}

impl MaterializedRuntime {
    pub fn redacted_preview(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push_str("# ");
            output.push_str(&file.relative_path);
            output.push('\n');
            if file.sensitive {
                output.push_str(&Redactor::redact(&file.content));
            } else {
                output.push_str(&file.content);
            }
            output.push('\n');
        }

        output
    }

    pub fn write_to(&self, base_dir: &Path) -> Result<Vec<PathBuf>, RunnerError> {
        fs::create_dir_all(base_dir)?;
        let base_dir = base_dir.canonicalize()?;
        let mut written = Vec::new();
        for file in &self.files {
            let target = base_dir.join(&file.relative_path);
            let parent = target
                .parent()
                .ok_or_else(|| RunnerError::UnsafePath(file.relative_path.clone()))?;
            fs::create_dir_all(parent)?;
            let parent = parent.canonicalize()?;
            if !parent.starts_with(&base_dir) {
                return Err(RunnerError::UnsafePath(file.relative_path.clone()));
            }

            fs::write(&target, &file.content)?;
            written.push(target);
        }

        Ok(written)
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeMaterializer {
    runtime_policy: RuntimeSecurityPolicy,
}

impl RuntimeMaterializer {
    pub fn materialize<R>(
        &self,
        config: &RuntimeConfig,
        resolver: &R,
    ) -> Result<MaterializedRuntime, RunnerError>
    where
        R: SecretResolver,
    {
        self.runtime_policy.validate_config(config)?;

        let files = config
            .files
            .iter()
            .map(|file| {
                Ok(MaterializedFile {
                    relative_path: file.relative_path.clone(),
                    content: replace_placeholders(&file.content, resolver)?,
                    sensitive: file.sensitive,
                })
            })
            .collect::<Result<Vec<_>, RunnerError>>()?;
        let command_args = config
            .command_args
            .iter()
            .map(|arg| replace_placeholders(arg, resolver))
            .collect::<Result<Vec<_>, RunnerError>>()?;
        let environment = config
            .environment
            .iter()
            .map(|(key, value)| Ok((key.clone(), replace_placeholders(value, resolver)?)))
            .collect::<Result<BTreeMap<_, _>, RunnerError>>()?;

        let materialized = MaterializedRuntime {
            files,
            command_args,
            environment,
        };
        self.runtime_policy.validate_config(&RuntimeConfig {
            files: materialized
                .files
                .iter()
                .map(|file| poh_core::RuntimeFile {
                    relative_path: file.relative_path.clone(),
                    content: file.content.clone(),
                    sensitive: file.sensitive,
                })
                .collect(),
            command_args: materialized.command_args.clone(),
            environment: materialized.environment.clone(),
        })?;

        Ok(materialized)
    }
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error(transparent)]
    Security(#[from] poh_core::SecurityError),
    #[error("missing secret: {0}")]
    MissingSecret(String),
    #[error("unsafe materialized path: {0}")]
    UnsafePath(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn replace_placeholders<R>(input: &str, resolver: &R) -> Result<String, RunnerError>
where
    R: SecretResolver,
{
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('>') else {
            output.push('<');
            rest = after_start;
            continue;
        };

        let key = &after_start[..end];
        if is_secret_key(key) {
            let secret = resolver
                .resolve(key)
                .ok_or_else(|| RunnerError::MissingSecret(key.to_string()))?;
            output.push_str(&secret);
        } else {
            output.push('<');
            output.push_str(key);
            output.push('>');
        }

        rest = &after_start[end + 1..];
    }
    output.push_str(rest);

    Ok(output)
}

fn is_secret_key(key: &str) -> bool {
    matches!(
        key,
        "endpoint.password" | "endpoint.client_random" | "listener.socks.password"
    )
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use poh_core::{RuntimeFile, RuntimeSecurityPolicy};

    use super::*;

    #[test]
    fn materializer_substitutes_known_secret_placeholders() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "config.toml".to_string(),
                content: "password = \"<endpoint.password>\"".to_string(),
                sensitive: true,
            }],
            command_args: vec!["--config".to_string(), "config.toml".to_string()],
            environment: BTreeMap::new(),
        };
        let resolver = MapSecretResolver::new(BTreeMap::from([(
            "endpoint.password".to_string(),
            "real-secret".to_string(),
        )]));

        let materialized = RuntimeMaterializer::default()
            .materialize(&config, &resolver)
            .unwrap();

        assert!(materialized.files[0].content.contains("real-secret"));
        assert!(!materialized.redacted_preview().contains("real-secret"));
    }

    #[test]
    fn materializer_requires_missing_secret() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "config.toml".to_string(),
                content: "password = \"<endpoint.password>\"".to_string(),
                sensitive: true,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        let error = RuntimeMaterializer::default()
            .materialize(&config, &MapSecretResolver::default())
            .unwrap_err();
        assert!(matches!(error, RunnerError::MissingSecret(_)));
    }

    #[test]
    fn materializer_rejects_unsafe_runtime_path() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "../config.toml".to_string(),
                content: "test".to_string(),
                sensitive: false,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        let error = RuntimeMaterializer::default()
            .materialize(&config, &MapSecretResolver::default())
            .unwrap_err();
        assert!(matches!(error, RunnerError::Security(_)));
    }

    #[test]
    fn materialized_runtime_writes_inside_base_directory() {
        let base = std::env::temp_dir().join(format!(
            "poh-runner-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let runtime = MaterializedRuntime {
            files: vec![MaterializedFile {
                relative_path: "runtime/config.toml".to_string(),
                content: "loglevel = \"info\"".to_string(),
                sensitive: false,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        let written = runtime.write_to(&base).unwrap();
        assert_eq!(written.len(), 1);
        assert!(written[0].starts_with(base.canonicalize().unwrap()));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn runtime_security_policy_still_rejects_absolute_paths() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "C:/escape.toml".to_string(),
                content: String::new(),
                sensitive: false,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        assert!(RuntimeSecurityPolicy::default()
            .validate_config(&config)
            .is_err());
    }
}
