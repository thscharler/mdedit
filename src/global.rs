use crate::config::MDConfig;
use crate::theme::DarkTheme;
use rat_theme2::Scheme;
use rat_widget::hover::HoverState;
use std::rc::Rc;

#[derive(Debug)]
pub struct GlobalState {
    pub cfg: MDConfig,
    pub theme: Rc<DarkTheme>,
    pub hover: HoverState,
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self {
            cfg,
            theme: Rc::new(theme),
            hover: HoverState::default(),
        }
    }

    pub fn scheme(&self) -> &Scheme {
        &self.theme.scheme()
    }
}
