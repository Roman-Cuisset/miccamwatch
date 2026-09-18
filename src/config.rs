use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Conservative,
    #[default]
    Balanced,
    Strict,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrustPolicy {
    #[default]
    Offline,
    Online,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub trust_policy: TrustPolicy,
    #[serde(default)]
    pub applications: Vec<ApplicationRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRule {
    pub executable: String,
    #[serde(default)]
    pub publishers: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Policy {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read policy {}", path.display()))?;
        let policy: Self = toml::from_str(&contents)
            .with_context(|| format!("invalid policy {}", path.display()))?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<()> {
        let mut names = HashSet::new();
        for rule in &self.applications {
            let executable = rule.executable.trim().to_ascii_lowercase();
            if executable.is_empty() {
                bail!("application executable cannot be empty");
            }
            if !names.insert(executable.clone()) {
                bail!("duplicate application rule for {executable}");
            }
            if rule.publishers.iter().any(|value| value.trim().is_empty()) {
                bail!("publisher names cannot be empty for {executable}");
            }
            if rule.paths.iter().any(|value| value.trim().is_empty()) {
                bail!("expected paths cannot be empty for {executable}");
            }
        }
        Ok(())
    }

    pub fn application(&self, executable: &str) -> Option<&ApplicationRule> {
        self.applications
            .iter()
            .find(|rule| rule.executable.eq_ignore_ascii_case(executable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_and_duplicate_rules() {
        assert!(toml::from_str::<Policy>("unknown = true").is_err());
        let policy: Policy = toml::from_str(
            r#"
                [[applications]]
                executable = "browser.exe"
                [[applications]]
                executable = "BROWSER.EXE"
            "#,
        )
        .unwrap();
        assert!(policy.validate().is_err());
    }
}
