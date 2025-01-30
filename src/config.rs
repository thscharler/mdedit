use anyhow::{anyhow, Error};
use dirs::config_dir;
use ini::Ini;
use std::fs::create_dir_all;
use std::path::PathBuf;

#[derive(Debug)]
pub struct MDConfig {
    pub theme: String,
    pub show_ctrl: bool,
    pub file_split_at: u16,
    pub new_line: String,
    pub load_file: Vec<PathBuf>,
    pub globs: Vec<String>,
    pub log: String,
}

#[cfg(windows)]
const LINE_ENDING: &str = "\r\n";

#[cfg(not(windows))]
const LINE_ENDING: &str = "\n";

const DEFAULT_FILE_SPLIT_AT: u16 = 15;

impl MDConfig {
    pub fn load() -> Result<MDConfig, Error> {
        let cfg = if let Some(config) = config_dir() {
            let config = config.join("mdedit").join("mdedit.ini");
            if config.exists() {
                let ini = Ini::load_from_file(config)?;

                let section: Option<String> = None;
                let theme = ini.get_from_or(section.clone(), "theme", "Imperial");
                let file_split_at = ini
                    .get_from_or(
                        section.clone(),
                        "file_split_at",
                        DEFAULT_FILE_SPLIT_AT.to_string().as_str(),
                    )
                    .parse()
                    .unwrap_or(DEFAULT_FILE_SPLIT_AT);
                let mut globs = ini
                    .get_from_or(section.clone(), "file_pattern", "*.md")
                    .split([' ', ','])
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>();
                globs.sort();
                globs.dedup();
                let log = ini
                    .get_from_or(section.clone(), "log", "warn")
                    .trim()
                    .to_string();

                Some(MDConfig {
                    theme: theme.into(),
                    show_ctrl: false,
                    file_split_at,
                    new_line: LINE_ENDING.into(),
                    load_file: Default::default(),
                    globs,
                    log,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(cfg.unwrap_or(MDConfig {
            theme: "Imperial".to_string(),
            show_ctrl: false,
            file_split_at: DEFAULT_FILE_SPLIT_AT,
            new_line: LINE_ENDING.into(),
            load_file: Default::default(),
            globs: vec!["*.md".to_string()],
            log: "debug".to_string(),
        }))
    }

    pub fn store(&self) -> Result<(), Error> {
        if let Some(config_root) = config_dir() {
            let config_dir = config_root.join("mdedit");
            if !config_dir.exists() {
                create_dir_all(&config_dir)?;
            }

            let config = config_dir.join("mdedit.ini");
            let mut ini = Ini::new();
            let section: Option<String> = None;
            ini.set_to(section.clone(), "theme".into(), self.theme.clone());
            ini.set_to(
                section.clone(),
                "file_split_at".into(),
                self.file_split_at.to_string(),
            );
            ini.set_to(
                section.clone(),
                "file_pattern".into(),
                self.globs
                    .iter()
                    .cloned()
                    .reduce(|mut v, w| {
                        v.push(',');
                        v.push(' ');
                        v.push_str(&w);
                        v
                    })
                    .unwrap_or("*.md".to_string()),
            );
            ini.set_to(section.clone(), "log".into(), self.log.clone());

            ini.write_to_file(config)?;

            Ok(())
        } else {
            Err(anyhow!("Can't save config."))
        }
    }
}
