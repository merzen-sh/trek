use anyhow::Context;
use inquire::Text;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    workspace_dir: Option<String>,
}

impl Config {
    fn path() -> PathBuf {
        path_from(
            std::env::var("XDG_CONFIG_HOME").ok(),
            std::env::var("HOME").ok(),
            std::env::var("APPDATA").ok(),
        )
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).context("failed to read config file")?;
            let cfg: Config = toml::from_str(&content).context("failed to parse config file")?;
            Ok(cfg)
        } else {
            Ok(Self {
                workspace_dir: None,
            })
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create config directory")?;
        }
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        std::fs::write(&path, content).context("failed to write config file")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn workspace_dir(&self) -> Option<&str> {
        self.workspace_dir.as_deref()
    }

    pub fn ensure_workspace_dir(&mut self) -> anyhow::Result<String> {
        if let Some(ref dir) = self.workspace_dir {
            let extracted = extract_name(dir);
            if !extracted.is_empty() {
                return Ok(extracted);
            }
        }

        let default = std::env::current_dir()
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("[{name}]")
            })
            .unwrap_or_default();

        let input = Text::new("workspace_dir not set. Enter workspace name like [name]")
            .with_default(&default)
            .prompt()
            .context("failed to read input")?;

        let name = extract_name(&input);
        if name.is_empty() {
            anyhow::bail!("workspace name must be a single name, not a path");
        }

        self.workspace_dir = Some(name.clone());
        self.save()?;
        tracing::info!("saved workspace_dir to {}", Self::path().display());
        Ok(name)
    }
}

fn extract_name(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let name = s
        .rsplit(|c: char| c == '/' || c == '\\')
        .find(|part| !part.is_empty())
        .map(|part| part.trim().to_string())
        .unwrap_or_default();

    if name.starts_with('[') && name.ends_with(']') {
        name
    } else {
        String::new()
    }
}

fn path_from(
    xdg_config_home: Option<String>,
    home: Option<String>,
    appdata: Option<String>,
) -> PathBuf {
    if let Some(dir) = xdg_config_home {
        PathBuf::from(dir)
    } else if cfg!(windows) {
        let base = appdata
            .map(PathBuf::from)
            .or_else(|| home.map(|h| PathBuf::from(h).join("AppData").join("Roaming")))
            .unwrap_or_default();
        base
    } else {
        home.map(|h| PathBuf::from(h).join(".config"))
            .unwrap_or_default()
    }
    .join("trek")
    .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::{extract_name, path_from};
    use std::path::PathBuf;

    #[test]
    fn extract_plain_name() {
        assert_eq!(extract_name("[trek]"), "[trek]");
    }

    #[test]
    fn extract_rejects_unbracketed() {
        assert_eq!(extract_name("trek"), "");
    }

    #[test]
    fn extract_from_unix_path() {
        assert_eq!(extract_name("/home/user/[trek]"), "[trek]");
    }

    #[test]
    fn extract_from_windows_path() {
        assert_eq!(extract_name("C:\\Users\\user\\[myapp]"), "[myapp]");
    }

    #[test]
    fn extract_trailing_slash() {
        assert_eq!(extract_name("/home/user/"), "");
    }

    #[test]
    fn extract_empty() {
        assert_eq!(extract_name(""), "");
    }

    #[test]
    fn extract_whitespace() {
        assert_eq!(extract_name("  "), "");
    }

    #[test]
    fn unix_xdg_config_home() {
        let p = path_from(
            Some("/home/user/.config".into()),
            Some("/home/user".into()),
            None,
        );
        assert_eq!(p, PathBuf::from("/home/user/.config/trek/config.toml"));
    }

    #[test]
    fn unix_home_fallback() {
        let p = path_from(None, Some("/home/user".into()), None);
        assert_eq!(p, PathBuf::from("/home/user/.config/trek/config.toml"));
    }

    #[test]
    fn fallback_no_env() {
        let p = path_from(None, None, None);
        assert_eq!(p, PathBuf::from("trek/config.toml"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_appdata() {
        let p = path_from(
            None,
            Some("C:\\Users\\user".into()),
            Some("C:\\Users\\user\\AppData\\Roaming".into()),
        );
        assert_eq!(
            p,
            PathBuf::from("C:\\Users\\user\\AppData\\Roaming\\trek\\config.toml")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_home_fallback_when_no_appdata() {
        let p = path_from(None, Some("C:\\Users\\user".into()), None);
        assert_eq!(
            p,
            PathBuf::from("C:\\Users\\user\\AppData\\Roaming\\trek\\config.toml")
        );
    }
}
