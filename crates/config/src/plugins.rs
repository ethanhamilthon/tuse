use crate::modules::resolve_path;
use std::path::{Path, PathBuf};

pub fn tuse_home() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join(".tuse")
}

pub fn plugins_root() -> PathBuf {
    tuse_home().join("plugins")
}

pub fn normalize_url(url: &str) -> String {
    let s = url.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    s.trim_end_matches('/').to_string()
}

pub const BUNDLED_DEFAULT: &str = "tuse/default";

#[derive(Default)]
pub struct Spec {
    pub dir: Option<String>,
    pub path: Option<String>,
    pub git_ref: Option<String>,
    pub enabled: bool,
}

impl Spec {
    pub fn enabled() -> Spec {
        Spec {
            enabled: true,
            ..Spec::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    name: String,
    url: Option<String>,
    root: PathBuf,
    sub: String,
    git_ref: Option<String>,
    enabled: bool,
    bundled: bool,
}

pub struct Repo<'a> {
    url: &'a str,
    root: &'a Path,
    git_ref: Option<&'a str>,
}

impl<'a> Repo<'a> {
    pub fn url(&self) -> &'a str {
        self.url
    }

    pub fn root(&self) -> &'a Path {
        self.root
    }

    pub fn git_ref(&self) -> Option<&'a str> {
        self.git_ref
    }
}

impl Plugin {
    pub fn new(name: &str, spec: Spec, base: &Path) -> Result<Plugin, String> {
        let Spec {
            dir,
            path,
            git_ref,
            enabled,
        } = spec;
        let dir = dir.as_deref();
        let path = path.as_deref();
        if name == BUNDLED_DEFAULT {
            if dir.is_some() || path.is_some() || git_ref.is_some() {
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
                enabled,
                bundled: true,
            });
        }
        let bad = || format!("plugin `{name}`: expected owner/repo[/path], a git URL or `dir`");
        let mut p = Plugin {
            name: name.to_string(),
            url: None,
            root: PathBuf::new(),
            sub: String::new(),
            git_ref,
            enabled,
            bundled: false,
        };
        if let Some(d) = dir {
            p.root = resolve_path(d, base);
        } else if name.contains("://") || name.starts_with("git@") {
            let trimmed = name.trim_end_matches('/');
            let tail = trimmed.rsplit(['/', ':']).take(2).collect::<Vec<_>>();
            let [repo, owner] = tail[..] else {
                return Err(bad());
            };
            let repo = repo.trim_end_matches(".git");
            if repo.is_empty() || owner.is_empty() {
                return Err(bad());
            }
            let host = trimmed.split("://").nth(1).unwrap_or(trimmed);
            let host = host
                .trim_start_matches("git@")
                .split(['/', ':'])
                .next()
                .unwrap_or("git");
            let host = if host.is_empty() { "file" } else { host };
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_bundled(&self) -> bool {
        self.bundled
    }

    pub fn repo(&self) -> Option<Repo<'_>> {
        self.url.as_deref().map(|url| Repo {
            url,
            root: &self.root,
            git_ref: self.git_ref.as_deref(),
        })
    }

    pub fn conflict_with(&self, later: &Plugin) -> Result<(), String> {
        if self.bundled || later.bundled || self.root != later.root {
            return Ok(());
        }
        if let (Some(a), Some(b)) = (&self.url, &later.url)
            && normalize_url(a) != normalize_url(b)
        {
            return Err(format!(
                "plugins: {} and {} map to the same checkout with different URLs",
                self.name, later.name
            ));
        }
        if self.git_ref.is_some() && later.git_ref.is_some() && self.git_ref != later.git_ref {
            return Err(format!(
                "plugins: {} and {} have conflicting ref {:?} vs {:?} for one repository",
                self.name,
                later.name,
                self.git_ref.as_deref().unwrap_or(""),
                later.git_ref.as_deref().unwrap_or("")
            ));
        }
        Ok(())
    }

    pub fn entry_file(&self) -> Result<PathBuf, String> {
        if let Some(file) = self.entry() {
            return Ok(file);
        }
        Err(if self.url.is_some() && !self.root.exists() {
            format!(
                "not checked out; run `git clone {} {}`{}, see docs(\"plugins\")",
                self.url.as_deref().unwrap_or_default(),
                self.root.display(),
                self.git_ref
                    .as_deref()
                    .map(|r| format!(" and `git -C {} checkout {r}`", self.root.display()))
                    .unwrap_or_default()
            )
        } else {
            format!("no init.lua or tuse.lua in {}", self.dir().display())
        })
    }

    fn dir(&self) -> PathBuf {
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

    fn commit(&self) -> Option<String> {
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

    fn entry(&self) -> Option<PathBuf> {
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
        let p = Plugin::new("me/tuse/plugins/rex", Spec::enabled(), b).unwrap();
        assert_eq!(p.url.as_deref(), Some("https://github.com/me/tuse.git"));
        assert_eq!(
            (p.root.clone(), p.sub.as_str()),
            (root.join("me/tuse"), "plugins/rex")
        );
        assert_eq!(p.dir(), root.join("me/tuse/plugins/rex"));
        let p = Plugin::new(
            "https://gitlab.com/a/b.git",
            Spec {
                path: Some("x".into()),
                ..Spec::enabled()
            },
            b,
        )
        .unwrap();
        assert_eq!((p.root, p.sub.as_str()), (root.join("gitlab.com/a/b"), "x"));
        let p = Plugin::new("git@github.com:a/b.git", Spec::enabled(), b).unwrap();
        assert_eq!(p.root, root.join("github.com/a/b"));
        let p = Plugin::new(
            "dev",
            Spec {
                dir: Some("w".into()),
                ..Spec::enabled()
            },
            b,
        )
        .unwrap();
        assert_eq!((p.url, p.root), (None, PathBuf::from("/cfg/w")));
        for bad in ["rex", "a//b", "a/../b"] {
            assert!(Plugin::new(bad, Spec::enabled(), b).is_err(), "{bad}");
        }
        assert!(
            Plugin::new(
                "a/b",
                Spec {
                    path: Some("../x".into()),
                    ..Spec::enabled()
                },
                b
            )
            .is_err()
        );
    }
}
