use crate::config::MDConfig;
use rat_theme::dark_theme::DarkTheme;
use rat_theme::Scheme;
use std::rc::Rc;

#[derive(Debug)]
pub struct GlobalState {
    pub cfg: MDConfig,
    pub theme: Rc<DarkTheme>, //todo: rc??
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self {
            cfg,
            theme: Rc::new(theme),
        }
    }

    pub fn scheme(&self) -> &Scheme {
        &self.theme.scheme()
    }
}
