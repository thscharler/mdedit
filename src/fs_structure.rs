use anyhow::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
        let et = SystemTime::now();

        let new_root = if let Some(v) = find_root(path) {
            v
        } else {
            path.to_path_buf()
        };

        if self.root == new_root {
            return Ok(());
        }

        self.name = String::default();
        self.root = new_root;
        self.dirs.clear();
        self.display.clear();

        if let Some(v) = cargo_name(&self.root)? {
            self.name = v;
        } else if let Some(v) = mdbook_name(&self.root)? {
            self.name = v;
        } else if let Some(v) = self.root.file_name() {
            self.name = v.to_string_lossy().to_string();
        } else {
            self.name = ".".to_string();
        }

        self.dirs.push(self.root.clone());
        self.display.push(self.name.clone());

        // fs_recurse(&self.root, "", &mut self.dirs, &mut self.display)?;
        fs_recurse3(&self.root, "", &mut self.dirs, &mut self.display)?;

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
    if let Some(package) = toml.as_table().expect("table").get("book") {
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

fn fs_recurse3(
    dir: &Path,
    _prefix: &str,
    dirs: &mut Vec<PathBuf>,
    display: &mut Vec<String>,
) -> Result<(), Error> {
    #[derive(Debug, Default)]
    struct Tree {
        path: PathBuf,
        name: String,
        items: Vec<Tree>,
    }

    let mut tree = Tree::default();

    let walk = ignore::WalkBuilder::new(dir)
        .standard_filters(true) //
        .build();
    for w in walk {
        let w = w?;
        let next = w.path().strip_prefix(dir)?;

        let Some(parent) = next.parent() else {
            continue;
        };

        let mut branch = &mut tree;
        for c in parent.components() {
            let c_str = c.as_os_str().to_string_lossy();
            let c_str = c_str.as_ref();

            let found = branch.items.iter().position(|v| v.name == c_str);
            if let Some(found) = found {
                branch = &mut branch.items[found];
            } else {
                let new = Tree {
                    path: parent.to_path_buf(),
                    name: c_str.to_string(),
                    items: Vec::new(),
                };
                branch.items.push(new);
                branch = branch.items.last_mut().expect("last");
            }
        }
    }

    let mut stack = Vec::new();

    #[derive(Debug)]
    struct TreeStack<'a> {
        branch: &'a Tree,
        idx: usize,
        prefix: String,
    }

    stack.push(TreeStack {
        branch: &tree,
        idx: 0,
        prefix: "".to_string(),
    });

    loop {
        let Some(mut v) = stack.pop() else {
            break;
        };

        if v.idx >= v.branch.items.len() {
            continue;
        }

        if v.idx + 1 == v.branch.items.len() {
            let b = &v.branch.items[v.idx];
            dirs.push(dir.join(&b.path));
            display.push(format!("{}└{}", v.prefix, b.name));
        } else {
            let b = &v.branch.items[v.idx];
            dirs.push(dir.join(&b.path));
            display.push(format!("{}├{}", v.prefix, b.name));
        }

        let next = if v.idx + 1 == v.branch.items.len() {
            TreeStack {
                branch: &v.branch.items[v.idx],
                idx: 0,
                prefix: format!("{} ", v.prefix),
            }
        } else {
            TreeStack {
                branch: &v.branch.items[v.idx],
                idx: 0,
                prefix: format!("{}│", v.prefix),
            }
        };

        v.idx += 1;
        if v.idx < v.branch.items.len() {
            stack.push(v);
        }
        if next.branch.items.len() > 0 {
            stack.push(next);
        }
    }

    Ok(())
}

fn find_root(path: &Path) -> Option<PathBuf> {
    let mut path = path.to_path_buf();

    loop {
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() {
            return Some(path);
        }
        let book_toml = path.join("book.toml");
        if book_toml.exists() {
            return Some(path);
        }

        if let Some(v) = path.parent() {
            path = v.to_path_buf();
        } else {
            return None;
        }
    }
}
