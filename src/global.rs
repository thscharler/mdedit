use crate::config::MDConfig;
use crate::theme::DarkTheme;
use rat_theme::Scheme;

#[derive(Debug)]
pub struct GlobalState {
    pub cfg: MDConfig,
    pub theme: DarkTheme,
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self { cfg, theme }
    }

    pub fn scheme(&self) -> &Scheme {
        &self.theme.scheme()
    }
}
