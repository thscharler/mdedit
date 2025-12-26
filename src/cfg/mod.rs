use crate::split_tab::SplitTabState;
use anyhow::{anyhow, Error};
use dirs::config_dir;
use ini::Ini;
use log::warn;
use rat_widget::text::{upos_type, Locale};
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::str::FromStr;
use sys_locale::get_locale;

#[derive(Debug)]
pub struct MDConfig {
    // system
    pub loc: Locale,

    // ui cfg
    pub theme: String,
    pub text_width: u16,
    pub font: String,
    pub font_size: f64,

    // startup
    pub load_file: Vec<PathBuf>,
    pub globs: Vec<String>,

    // auto/tmp
    pub file_split_at: u16,
    pub show_ctrl: bool,
    pub show_break: bool,
    pub wrap_text: bool,
    pub show_linenr: bool,
    pub log_level: String,

    pub edit_split_at: Vec<u16>,
    pub tab_state: Vec<(usize, usize, PathBuf)>,
    pub tab_cursor: Vec<(usize, usize, upos_type, upos_type)>,
    pub tab_offset: Vec<(usize, usize, upos_type, upos_type, upos_type)>,
    pub tab_selected: (usize, usize),
}

const DEFAULT_FILE_SPLIT_AT: u16 = 15;
const DEFAULT_TEXT_WIDTH: u16 = 65;

impl Default for MDConfig {
    fn default() -> Self {
        let loc = get_locale().unwrap_or("en-US".into()).replace('-', "_");
        let locale = Locale::from_str(&loc).unwrap_or(Locale::POSIX);

        MDConfig {
            loc: locale,
            theme: "Imperial".to_string(),
            show_ctrl: false,
            show_break: false,
            wrap_text: false,
            file_split_at: DEFAULT_FILE_SPLIT_AT,
            text_width: DEFAULT_TEXT_WIDTH,
            font: "".to_string(),
            font_size: 20.0,
            load_file: Default::default(),
            globs: vec!["*.md".to_string()],
            log_level: "debug".to_string(),
            show_linenr: true,
            tab_state: Default::default(),
            tab_cursor: Default::default(),
            tab_offset: Default::default(),
            tab_selected: (0, 0),
            edit_split_at: Default::default(),
        }
    }
}

impl MDConfig {
    pub fn load() -> Result<MDConfig, Error> {
        let cfg = if let Some(config) = config_dir() {
            let config = config.join("mdedit").join("mdedit.ini");
            if config.exists() {
                let ini = Ini::load_from_file(config)?;
                let sec = ini.general_section();

                let theme = sec.get("theme").unwrap_or("Imperial");

                let text_width = sec
                    .get("text_width")
                    .unwrap_or(DEFAULT_TEXT_WIDTH.to_string().as_str())
                    .parse()
                    .unwrap_or(DEFAULT_TEXT_WIDTH);

                let font = sec.get("font")
                    .unwrap_or("").trim().to_string();
                let font_size = sec.get("font-size")
                    .unwrap_or("20")
                    .parse::<f64>()
                    .unwrap_or(20.0);

                let mut globs = sec
                    .get("file_pattern")
                    .unwrap_or("*.md")
                    .split([' ', ','])
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>();
                globs.sort();
                globs.dedup();

                let show_ctrl = sec
                    .get("show_ctrl")
                    .unwrap_or("false")
                    .parse()
                    .unwrap_or(false);
                let show_break = sec
                    .get("show_break")
                    .unwrap_or("false")
                    .parse()
                    .unwrap_or(false);
                let wrap_text = sec
                    .get("wrap_text")
                    .unwrap_or("false")
                    .parse()
                    .unwrap_or(false);
                let show_linenr = sec
                    .get("show_linenr")
                    .unwrap_or("true")
                    .parse()
                    .unwrap_or(true);

                let log = sec.get("log").unwrap_or("warn").trim().to_string();

                let file_split_at = DEFAULT_FILE_SPLIT_AT;
                if let Some(sec) = ini.section(Some("ui")) {
                    sec.get("file_split_at")
                        .unwrap_or(DEFAULT_FILE_SPLIT_AT.to_string().as_str())
                        .parse()
                        .unwrap_or(DEFAULT_FILE_SPLIT_AT);
                }

                let mut tab_state = Vec::new();
                let mut tab_cursor = Vec::new();
                let mut tab_offset = Vec::new();
                let mut tab_selected = (0, 0);
                let mut edit_split_at = Vec::new();
                if let Some(sec) = ini.section(Some("editor")) {
                    'f: {
                        for (k, v) in sec.iter() {
                            if k.starts_with("file.") {
                                let Some((s, t)) = Self::split_tab(k, v) else {
                                    break 'f;
                                };
                                let path = PathBuf::from(v);
                                if !path.exists() {
                                    warn!("file not found {}", path.to_string_lossy());
                                    break 'f;
                                }
                                tab_state.push((s, t, PathBuf::from(v)));
                            } else if k.starts_with("cursor.") {
                                let Some((s, t)) = Self::split_tab(k, v) else {
                                    break 'f;
                                };
                                let Some((x, y)) = Self::split_cursor(k, v) else {
                                    break 'f;
                                };
                                tab_cursor.push((s, t, x, y));
                            } else if k.starts_with("offset.") {
                                let Some((s, t)) = Self::split_tab(k, v) else {
                                    break 'f;
                                };
                                let Some((x, y, z)) = Self::split_offset(k, v) else {
                                    break 'f;
                                };
                                tab_offset.push((s, t, x, y, z));
                            }
                        }

                        if let Some(sel) = sec.get("selected") {
                            let mut sit = sel.split('.');
                            let Some(s) = sit.next() else {
                                warn!("no selected split in {}", sel);
                                break 'f;
                            };
                            let Ok(s) = s.parse::<usize>() else {
                                warn!("invalid split {} in {}", s, sel);
                                break 'f;
                            };
                            let Some(t) = sit.next() else {
                                warn!("no selected tab in {}", sel);
                                break 'f;
                            };
                            let Ok(t) = t.parse::<usize>() else {
                                warn!("invalid split {} in {}", t, sel);
                                break 'f;
                            };
                            tab_selected = (s, t);
                        }

                        if let Some(split) = sec.get("editor_widths") {
                            for s in split.split(',') {
                                let Ok(s) = s.trim().parse::<u16>() else {
                                    warn!("invalid split {} in {}", s, split);
                                    break 'f;
                                };
                                edit_split_at.push(s);
                            }
                        }
                    }
                }

                Some(MDConfig {
                    theme: theme.into(),
                    file_split_at,
                    text_width,
                    font,
                    font_size,
                    globs,
                    show_ctrl,
                    show_break,
                    wrap_text,
                    show_linenr,
                    log_level: log,
                    tab_state,
                    tab_cursor,
                    tab_offset,
                    tab_selected,
                    edit_split_at,
                    ..Default::default()
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(cfg.unwrap_or(MDConfig::default()))
    }

    fn split_offset(k: &str, v: &str) -> Option<(upos_type, upos_type, upos_type)> {
        let mut v_it = v.split(',');
        let Some(x) = v_it.next() else {
            warn!("no offset.x in {} {}", k, v);
            return None;
        };
        let Ok(x) = x.trim().parse::<upos_type>() else {
            warn!("invalid offset.x {} in {} {}", x, k, v);
            return None;
        };
        let Some(y) = v_it.next() else {
            warn!("no offset.y in {} {}", k, v);
            return None;
        };
        let Ok(y) = y.trim().parse::<upos_type>() else {
            warn!("invalid offset.y {} in {} {}", y, k, v);
            return None;
        };
        let Some(z) = v_it.next() else {
            warn!("no offset.s in {} {}", k, v);
            return None;
        };
        let Ok(z) = z.trim().parse::<upos_type>() else {
            warn!("invalid offset.s {} in {} {}", z, k, v);
            return None;
        };
        Some((x, y, z))
    }

    fn split_cursor(k: &str, v: &str) -> Option<(upos_type, upos_type)> {
        let mut v_it = v.split(',');
        let Some(x) = v_it.next() else {
            warn!("no cursor.x in {} {}", k, v);
            return None;
        };
        let Ok(x) = x.trim().parse::<upos_type>() else {
            warn!("invalid cursor.x {} in {} {}", x, k, v);
            return None;
        };
        let Some(y) = v_it.next() else {
            warn!("no cursor.y in {} {}", k, v);
            return None;
        };
        let Ok(y) = y.trim().parse::<upos_type>() else {
            warn!("invalid cursor.y {} in {} {}", y, k, v);
            return None;
        };
        Some((x, y))
    }

    fn split_tab(k: &str, v: &str) -> Option<(usize, usize)> {
        let mut k_it = k.split('.');
        k_it.next();
        let Some(s) = k_it.next() else {
            warn!("no split-nr in {} {}", k, v);
            return None;
        };
        let Ok(s) = s.parse::<usize>() else {
            warn!("invalid split-nr {} in {} {}", s, k, v);
            return None;
        };
        let Some(t) = k_it.next() else {
            warn!("no tab-nr in {} {}", k, v);
            return None;
        };
        let Ok(t) = t.parse::<usize>() else {
            warn!("invalid tab-nr {} in {} {}", s, k, v);
            return None;
        };
        Some((s, t))
    }

    pub fn store_file_state(&mut self, split_tab: &SplitTabState) {
        if let Some(pos) = split_tab.selected_pos() {
            self.tab_selected = pos;
        } else {
            self.tab_selected = (0, 0);
        }

        self.edit_split_at.clear();
        self.edit_split_at
            .extend_from_slice(split_tab.split.area_lengths());

        self.tab_state.clear();
        for (sidx, s) in split_tab.split_tab_file.iter().enumerate() {
            for (tidx, t) in s.iter().enumerate() {
                let edit = &split_tab.split_tab_file[sidx][tidx].edit;
                let cursor = edit.cursor();
                let offset = edit.offset();
                let sub_offset = edit.sub_row_offset();

                self.tab_state.push((sidx, tidx, t.path.clone()));
                self.tab_cursor.push((sidx, tidx, cursor.x, cursor.y));
                self.tab_offset.push((
                    sidx,
                    tidx,
                    offset.0 as upos_type,
                    offset.1 as upos_type,
                    sub_offset,
                ));
            }
        }
    }

    pub fn store(&self) -> Result<(), Error> {
        if let Some(config_root) = config_dir() {
            let config_dir = config_root.join("mdedit");
            if !config_dir.exists() {
                create_dir_all(&config_dir)?;
            }

            let config = config_dir.join("mdedit.ini");
            let mut ini = Ini::new();
            let mut sec = ini.with_general_section();
            sec.set("theme", self.theme.clone());
            sec.set("text_width", self.text_width.to_string());
            sec.set("font", self.font.clone());
            sec.set("font-size", self.font_size.to_string());
            sec.set(
                "file_pattern",
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
            sec.set("log", self.log_level.clone());
            sec.set("show_ctrl", self.show_ctrl.to_string());
            sec.set("show_break", self.show_break.to_string());
            sec.set("wrap_text", self.wrap_text.to_string());
            sec.set("show_linenr", self.show_linenr.to_string());

            let mut sec = ini.with_section(Some("ui"));
            sec.set("file_split_at", self.file_split_at.to_string());

            let mut sec = ini.with_section(Some("editor"));
            sec.set(
                "selected",
                format!("{}.{}", self.tab_selected.0, self.tab_selected.1),
            );
            for (s, t, f) in &self.tab_state {
                sec.set(
                    format!("file.{}.{}", *s, *t),
                    format!("{}", f.to_string_lossy()),
                );
            }
            for (s, t, x, y) in &self.tab_cursor {
                sec.set(format!("cursor.{}.{}", *s, *t), format!("{},{}", *x, *y));
            }
            for (s, t, ox, oy, os) in &self.tab_offset {
                sec.set(
                    format!("offset.{}.{}", *s, *t),
                    format!("{},{},{}", *ox, *oy, *os),
                );
            }
            let mut file_split = String::new();
            for s in &self.edit_split_at {
                if !file_split.is_empty() {
                    file_split.push_str(",");
                }
                file_split.push_str(format!("{}", *s).as_str());
            }
            sec.set("editor_widths", file_split);

            ini.write_to_file(config)?;

            Ok(())
        } else {
            Err(anyhow!("Can't save cfg."))
        }
    }
}
