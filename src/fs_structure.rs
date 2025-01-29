use anyhow::Error;
use log::debug;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FileSysStructure {
    pub root: PathBuf,
    pub name: String,
    pub dirs: Vec<PathBuf>,
    pub display: Vec<String>,

    pub files_dir: PathBuf,
    pub files: Vec<PathBuf>,
}

impl FileSysStructure {
    pub fn load(&mut self, path: &Path) -> Result<(), Error> {
        self.load_current(path)?;
        self.load_filesys(path)?;
        Ok(())
    }

    pub fn load_filesys(&mut self, path: &Path) -> Result<(), Error> {
        let new_root = if let Some(v) = find_root(path) {
            v
        } else {
            path.to_path_buf()
        };

        if self.root == new_root {
            return Ok(());
        }

        self.name = String::default();
        self.root = path.to_path_buf();
        self.dirs.clear();
        self.display.clear();

        if let Some(v) = cargo_name(&self.root)? {
            self.name = v;
        } else if let Some(v) = mdbook_name(&self.root)? {
            self.name = v;
        } else if let Some(v) = self.root.file_name() {
            self.name = v.to_string_lossy().to_string();
        } else {
            self.name = "".to_string();
        }

        fs_recurse(&self.root, "", &mut self.dirs, &mut self.display)?;

        Ok(())
    }

    pub fn load_current(&mut self, path: &Path) -> Result<(), Error> {
        self.files_dir = path.into();
        self.files.clear();

        if let Ok(rd) = fs::read_dir(path) {
            for f in rd {
                let Ok(f) = f else {
                    continue;
                };
                let f = f.path();
                if let Some(ext) = f.extension() {
                    if ext == "md" {
                        self.files.push(f);
                    }
                }
            }
        }

        Ok(())
    }
}

fn mdbook_name(mdbook_dir: &Path) -> Result<Option<String>, Error> {
    let mdbook = mdbook_dir.join("book.toml");

    if !mdbook.exists() {
        return Ok(None);
    }

    let config_str = fs::read_to_string(mdbook)?;

    let toml = config_str.parse::<toml::Value>()?;
    if let Some(package) = toml.as_table().expect("book").get("package") {
        if let Some(table) = package.as_table() {
            for (key, val) in table.iter() {
                match key.as_str() {
                    "title" => return Ok(Some(val.as_str().unwrap_or("").into())),
                    _ => {}
                }
            }
        }
    }

    Ok(None)
}

fn cargo_name(cargo_dir: &Path) -> Result<Option<String>, Error> {
    let cargo = cargo_dir.join("Cargo.toml");

    if !cargo.exists() {
        return Ok(None);
    }

    let config_str = fs::read_to_string(cargo)?;

    let toml = config_str.parse::<toml::Value>()?;
    if let Some(package) = toml.as_table().expect("table").get("package") {
        if let Some(table) = package.as_table() {
            for (key, val) in table.iter() {
                match key.as_str() {
                    "name" => return Ok(Some(val.as_str().unwrap_or("").into())),
                    _ => {}
                }
            }
        }
    }
    Ok(None)
}

fn fs_recurse(
    dir: &Path,
    prefix: &str,
    dirs: &mut Vec<PathBuf>,
    display: &mut Vec<String>,
) -> Result<(), Error> {
    let mut tmp = Vec::new();
    for f in fs::read_dir(&dir)? {
        let f = f?;
        if let Some(f_name) = f.path().file_name() {
            let f_name = f_name.to_string_lossy();
            if f_name == ".git" {
                continue;
            }
        }

        if f.path().is_dir() {
            tmp.push(f.path().to_path_buf());
        }
    }

    tmp.sort();

    let len = tmp.len();
    for (i, f) in tmp.into_iter().enumerate() {
        dirs.push(f.clone());

        let name = if let Some(name) = f.file_name() {
            name.to_string_lossy().to_string()
        } else {
            "???".to_string()
        };

        let f_display = if i + 1 == len {
            format!("{}└{}", prefix, name)
        } else {
            format!("{}├{}", prefix, name)
        };
        display.push(f_display);

        let next_prefix = if i + 1 == len {
            format!("{} ", prefix)
        } else {
            format!("{}│", prefix)
        };

        fs_recurse(&f, &next_prefix, dirs, display)?
    }

    Ok(())
}

fn find_root(path: &Path) -> Option<PathBuf> {
    debug!("find root for {:?}", path);

    let mut parent = if path.is_relative() {
        PathBuf::from(".").join(path)
    } else {
        path.to_path_buf()
    };

    debug!("find root2 for {:?}", path);

    loop {
        let cargo_toml = parent.join("Cargo.toml");
        if cargo_toml.exists() {
            debug!("found cargo");
            return Some(parent);
        }
        let book_toml = parent.join("book.toml");
        if book_toml.exists() {
            debug!("found book");
            return Some(parent);
        }

        if let Some(v) = parent.parent() {
            debug!("to parent {:?}", v);
            parent = v.to_path_buf();
        } else {
            return None;
        }
    }
}
