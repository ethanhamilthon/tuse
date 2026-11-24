use crate::load::resolve_path;
use std::path::{Path, PathBuf};

pub fn tuse_home() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join(".tuse")
}

pub fn plugins_root() -> PathBuf {
    tuse_home().join("plugins")
}

pub const BUNDLED_DEFAULT: &str = "tuse/default";

#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub name: String,
    pub url: Option<String>,
    pub root: PathBuf,
    pub sub: String,
    pub git_ref: Option<String>,
    pub enabled: bool,
    pub bundled: bool,
}

impl Plugin {
    pub fn new(
        name: &str,
        dir: Option<&str>,
        path: Option<&str>,
        base: &Path,
    ) -> Result<Plugin, String> {
        if name == BUNDLED_DEFAULT {
            if dir.is_some() || path.is_some() {
                return Err(format!(
                    "plugin {BUNDLED_DEFAULT}: bundled plugin does not accept ref, dir, or path"
                ));
            }
            return Ok(Plugin {
                name: BUNDLED_DEFAULT.to_string(),
                url: None,
                root: PathBuf::new(),
                sub: String::new(),
                git_ref: None,
                enabled: true,
                bundled: true,
            });
        }
        let bad = || format!("plugin `{name}`: expected owner/repo[/path], a git URL or `dir`");
        let mut p = Plugin {
            name: name.to_string(),
            url: None,
            root: PathBuf::new(),
            sub: String::new(),
            git_ref: None,
            enabled: true,
            bundled: false,
        };
        if let Some(d) = dir {
            p.root = resolve_path(d, base);
        } else if name.contains("://") || name.starts_with("git@") {
            let tail = name.rsplit(['/', ':']).take(2).collect::<Vec<_>>();
            let [repo, owner] = tail[..] else {
                return Err(bad());
            };
            let repo = repo.trim_end_matches(".git");
            let host = name.split("://").nth(1).unwrap_or(name);
            let host = host
                .trim_start_matches("git@")
                .split(['/', ':'])
                .next()
                .unwrap_or("git");
            p.url = Some(name.to_string());
            p.root = plugins_root().join(host).join(owner).join(repo);
        } else {
            let parts: Vec<&str> = name.split('/').collect();
            if parts.len() < 2
                || parts
                    .iter()
                    .any(|s| s.is_empty() || *s == "." || *s == "..")
            {
                return Err(bad());
            }
            p.url = Some(format!("https://github.com/{}/{}.git", parts[0], parts[1]));
            p.root = plugins_root().join(parts[0]).join(parts[1]);
            p.sub = parts[2..].join("/");
        }
        if let Some(extra) = path.filter(|s| !s.is_empty()) {
            if extra.split('/').any(|s| s == "..") {
                return Err(format!(
                    "plugin `{name}`: path must stay inside the repository"
                ));
            }
            p.sub = if p.sub.is_empty() {
                extra.into()
            } else {
                format!("{}/{extra}", p.sub)
            };
        }
        Ok(p)
    }

    pub fn dir(&self) -> PathBuf {
        if self.sub.is_empty() {
            self.root.clone()
        } else {
            self.root.join(&self.sub)
        }
    }

    pub fn info(&self) -> crate::PluginInfo {
        if self.bundled {
            return crate::PluginInfo {
                name: self.name.clone(),
                dir: "bundled".into(),
                git_ref: None,
                commit: Some("bundled".into()),
            };
        }
        crate::PluginInfo {
            name: self.name.clone(),
            dir: self.dir().display().to_string(),
            git_ref: self.git_ref.clone(),
            commit: self.commit(),
        }
    }

    pub fn commit(&self) -> Option<String> {
        let git = self.root.join(".git");
        let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
        let head = head.trim();
        let full = match head.strip_prefix("ref: ") {
            None => head.to_string(),
            Some(r) => match std::fs::read_to_string(git.join(r)) {
                Ok(s) => s.trim().to_string(),
                Err(_) => std::fs::read_to_string(git.join("packed-refs"))
                    .ok()?
                    .lines()
                    .find_map(|l| l.strip_suffix(r)?.strip_suffix(' ').map(str::to_string))?,
            },
        };
        Some(full.get(..7)?.to_string())
    }

    pub fn entry(&self) -> Option<PathBuf> {
        let d = self.dir();
        [d.join("init.lua"), d.join("tuse.lua")]
            .into_iter()
            .find(|p| p.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_clones() {
        let b = Path::new("/cfg");
        let root = plugins_root();
        let p = Plugin::new("me/tuse/plugins/rex", None, None, b).unwrap();
        assert_eq!(p.url.as_deref(), Some("https://github.com/me/tuse.git"));
        assert_eq!(
            (p.root.clone(), p.sub.as_str()),
            (root.join("me/tuse"), "plugins/rex")
        );
        assert_eq!(p.dir(), root.join("me/tuse/plugins/rex"));
        let p = Plugin::new("https://gitlab.com/a/b.git", None, Some("x"), b).unwrap();
        assert_eq!((p.root, p.sub.as_str()), (root.join("gitlab.com/a/b"), "x"));
        let p = Plugin::new("git@github.com:a/b.git", None, None, b).unwrap();
        assert_eq!(p.root, root.join("github.com/a/b"));
        let p = Plugin::new("dev", Some("w"), None, b).unwrap();
        assert_eq!((p.url, p.root), (None, PathBuf::from("/cfg/w")));
        for bad in ["rex", "a//b", "a/../b"] {
            assert!(Plugin::new(bad, None, None, b).is_err(), "{bad}");
        }
        assert!(Plugin::new("a/b", None, Some("../x"), b).is_err());
    }
}
