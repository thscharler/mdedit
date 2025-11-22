//!
//! Implements a dark theme.
//!

use crate::cfg::MDConfig;
use crate::global::event::MDEvent;
use crate::global::theme::DarkTheme;
use anyhow::Error;
use rat_dialog::DialogStack;
use rat_salsa::{SalsaAppContext, SalsaContext};
use rat_theme4::palette::Palette;
use rat_widget::hover::HoverState;

#[derive(Debug)]
pub struct GlobalState {
    ctx: SalsaAppContext<MDEvent, Error>,
    pub cfg: MDConfig,
    pub theme: DarkTheme,
    pub hover: HoverState,
    pub dialogs: DialogStack<MDEvent, GlobalState, Error>,
}

impl SalsaContext<MDEvent, Error> for GlobalState {
    fn set_salsa_ctx(&mut self, app_ctx: SalsaAppContext<MDEvent, Error>) {
        self.ctx = app_ctx;
    }

    fn salsa_ctx(&self) -> &SalsaAppContext<MDEvent, Error> {
        &self.ctx
    }
}

impl GlobalState {
    pub fn new(cfg: MDConfig, theme: DarkTheme) -> Self {
        Self {
            ctx: Default::default(),
            cfg,
            theme,
            hover: Default::default(),
            dialogs: Default::default(),
        }
    }

    pub fn palette(&self) -> &Palette {
        &self.theme.palette()
    }
}

pub mod event;
pub mod theme;
