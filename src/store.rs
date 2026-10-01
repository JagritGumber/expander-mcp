use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct PromptSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PromptStore {
    root: PathBuf,
}

impl PromptStore {
    pub fn from_environment() -> Result<Self, String> {
        if let Some(root) = env::var_os("EXPANDER_MCP_DIR").filter(|value| !value.is_empty()) {
            return Ok(Self::new(PathBuf::from(root)));
        }

        let root = if cfg!(windows) {
            env::var_os("APPDATA")
                .map(PathBuf::from)
                .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
                .ok_or_else(|| "could not determine the user data directory".to_string())?
                .join("expander-mcp")
                .join("prompts")
        } else if let Some(data_home) =
            env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty())
        {
            PathBuf::from(data_home)
                .join("expander-mcp")
                .join("prompts")
        } else {
            env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| "HOME is not set".to_string())?
                .join(".local")
                .join("share")
                .join("expander-mcp")
                .join("prompts")
        };

        Ok(Self::new(root))
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn set(&self, name: &str, content: &str) -> Result<(), String> {
        validate_name(name)?;
        if content.trim().is_empty() {
            return Err("prompt content cannot be empty".into());
        }

        fs::create_dir_all(&self.root)
            .map_err(|error| format!("could not create {}: {error}", self.root.display()))?;

        let destination = self.path_for(name);
        let temporary = self
            .root
            .join(format!(".{name}.{}.tmp", std::process::id()));
        fs::write(&temporary, content)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;

        if let Err(first_error) = fs::rename(&temporary, &destination) {
            if destination.exists() {
                fs::remove_file(&destination).map_err(|error| {
                    format!("could not replace {}: {error}", destination.display())
                })?;
                fs::rename(&temporary, &destination).map_err(|error| {
                    format!("could not move prompt into place after {first_error}: {error}")
                })?;
            } else {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "could not save {}: {first_error}",
                    destination.display()
                ));
            }
        }

        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<String, String> {
        validate_name(name)?;
        let path = self.path_for(name);
        if path.is_symlink() {
            return Err(format!("prompt '{name}' cannot be a symbolic link"));
        }
        fs::read_to_string(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("prompt '{name}' was not found")
            } else {
                format!("could not read {}: {error}", path.display())
            }
        })
    }

    pub fn list(&self) -> Result<Vec<PromptSummary>, String> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut prompts = Vec::new();
        let entries = fs::read_dir(&self.root)
            .map_err(|error| format!("could not read {}: {error}", self.root.display()))?;

        for entry in entries {
            let entry = entry.map_err(|error| format!("could not read prompt entry: {error}"))?;
            let file_type = entry.file_type().map_err(|error| {
                format!("could not inspect {}: {error}", entry.path().display())
            })?;
            if !file_type.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("md")
            {
                continue;
            }
            let Some(name) = entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            if validate_name(&name).is_err() {
                continue;
            }
            let content = fs::read_to_string(entry.path())
                .map_err(|error| format!("could not read prompt '{name}': {error}"))?;
            prompts.push(PromptSummary {
                name,
                description: first_line(&content),
            });
        }

        prompts.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(prompts)
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        validate_name(name)?;
        let path = self.path_for(name);
        if path.is_symlink() {
            return Err(format!("prompt '{name}' cannot be a symbolic link"));
        }
        fs::remove_file(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("prompt '{name}' was not found")
            } else {
                format!("could not delete {}: {error}", path.display())
            }
        })
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.md"))
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return Err("prompt name cannot be empty".into());
    };
    if !first.is_ascii_alphanumeric()
        || name.len() > 64
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(
            "prompt names must start with a letter or number and contain only letters, numbers, '.', '-' or '_' (64 characters max)"
                .into(),
        );
    }
    Ok(())
}

fn first_line(content: &str) -> String {
    let line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let mut description: String = line.chars().take(100).collect();
    if line.chars().count() > 100 {
        description.push('…');
    }
    description
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_store() -> PromptStore {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        PromptStore::new(env::temp_dir().join(format!("expander-mcp-test-{suffix}")))
    }

    #[test]
    fn saves_lists_reads_and_replaces_prompts() {
        let store = temporary_store();
        store
            .set("review-prompt", "Review this diff.\nBe strict.")
            .unwrap();
        assert_eq!(
            store.get("review-prompt").unwrap(),
            "Review this diff.\nBe strict."
        );
        assert_eq!(store.list().unwrap()[0].name, "review-prompt");
        store.set("review-prompt", "Review it again.").unwrap();
        assert_eq!(store.get("review-prompt").unwrap(), "Review it again.");
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn rejects_path_traversal() {
        let store = temporary_store();
        assert!(store.set("../escape", "nope").is_err());
        assert!(store.get("/tmp/escape").is_err());
    }
}
