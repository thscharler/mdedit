use crate::config::MDConfig;
use crate::theme::DarkTheme;
use rat_theme::Scheme;
use rat_widget::hover::HoverState;

#[derive(Debug)]
pub struct GlobalState {
    pub cfg: MDConfig,
    pub theme: DarkTheme,
    pub hover: HoverState,
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self {
            cfg,
            theme,
            hover: HoverState::default(),
        }
    }

    pub fn scheme(&self) -> &Scheme {
        &self.theme.s()
    }
}
