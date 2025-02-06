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

    pub is_cargo: bool,
    pub is_mdbook: bool,

    pub files_dir: PathBuf,
    pub files: Vec<PathBuf>,
}

// only needed for MDEvent ...
impl PartialEq for FileSysStructure {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for FileSysStructure {}

impl FileSysStructure {
    pub fn new() -> Self {
        Self {
            root: Default::default(),
            name: Default::default(),
            dirs: Default::default(),
            display: Default::default(),
            is_cargo: Default::default(),
            is_mdbook: Default::default(),
            files_dir: Default::default(),
            files: Default::default(),
        }
    }

    pub fn load(&mut self, path: &Path, globs: &[String]) -> Result<(), Error> {
        debug!("** load {:?} {:?}", path, globs);
        self.load_current(path, globs)?;
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

        debug!("** change root {:?}", new_root);

        self.name = String::default();
        self.root = new_root;
        self.dirs.clear();
        self.display.clear();

        if let Some(v) = cargo_name(&self.root)? {
            self.name = v;
            self.is_cargo = true;
            self.is_mdbook = false;
        } else if let Some(v) = mdbook_name(&self.root)? {
            self.name = v;
            self.is_cargo = false;
            self.is_mdbook = true;
        } else if let Some(v) = self.root.file_name() {
            self.name = v.to_string_lossy().to_string();
            self.is_cargo = false;
            self.is_mdbook = false;
        } else {
            self.name = ".".to_string();
            self.is_cargo = false;
            self.is_mdbook = false;
        }

        self.dirs.push(self.root.clone());
        self.display.push(self.name.clone());

        // fs_recurse(&self.root, "", &mut self.dirs, &mut self.display)?;
        fs_recurse3(&self.root, "", &mut self.dirs, &mut self.display)?;

        Ok(())
    }

    pub fn load_current(&mut self, path: &Path, globs: &[String]) -> Result<(), Error> {
        debug!("load current {:?} {:?}", path, globs);

        self.files_dir = path.into();
        self.files.clear();

        for pat in globs {
            let pat = path.join(pat);
            let pat = pat.to_string_lossy();

            let rd = glob::glob(pat.as_ref())?;

            for f in rd {
                let Ok(f) = f else {
                    continue;
                };
                if f.is_file() {
                    debug!("    found {:?} -> {:?}", pat, f);
                    self.files.push(f);
                }
            }
        }

        self.files.sort();
        self.files.dedup();

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
