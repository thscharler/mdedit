use crate::config::MDConfig;
use crate::event::MDEvent;
use crate::theme::DarkTheme;
use anyhow::Error;
use rat_dialog::DialogStackState;
use rat_theme2::Palette;
use rat_widget::hover::HoverState;
use std::rc::Rc;

#[derive(Debug)]
pub struct GlobalState {
    pub cfg: MDConfig,
    pub theme: Rc<DarkTheme>,
    pub hover: HoverState,
    pub dialogs: DialogStackState<GlobalState, MDEvent, Error>,
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self {
            cfg,
            theme: Rc::new(theme),
            hover: Default::default(),
            dialogs: Default::default(),
        }
    }

    pub fn scheme(&self) -> &Palette {
        &self.theme.scheme()
    }
}
