use crate::event::MDEvent;
use anyhow::Error;
use crossterm::event::Event;
use rat_salsa::Control;
use rat_widget::event::{try_flow, Dialog, FileOutcome, HandleEvent};
use rat_widget::file_dialog::{FileDialog, FileDialogState, FileDialogStyle};
use rat_widget::text::HasScreenCursor;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;
use std::path::PathBuf;

/// Multi purpose facility.
pub trait Facility<T, O, A, E> {
    /// Engage with the facility.
    /// Setup its current workings and set a handler for any possible outcomes.
    fn engage(
        &mut self,
        init: impl FnOnce(&mut T) -> Result<Control<A>, E>,
        out: fn(O) -> Result<Control<A>, E>,
    ) -> Result<Control<A>, E>;

    /// Handle crossterm events for the facility.
    fn handle(&mut self, event: &Event) -> Result<Control<A>, E>;
}

#[derive(Debug, Default)]
pub struct MDFileDialog {
    style: FileDialogStyle,
}

impl MDFileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn style(mut self, style: FileDialogStyle) -> Self {
        self.style = style;
        self
    }
}

impl StatefulWidget for MDFileDialog {
    type State = MDFileDialogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        FileDialog::new()
            .styles(self.style)
            .render(area, buf, &mut state.file_dlg);
    }
}

#[derive(Debug, Default)]
pub struct MDFileDialogState {
    pub file_dlg: FileDialogState,
    pub handle: Option<fn(PathBuf) -> Result<Control<MDEvent>, Error>>,
}

impl Facility<FileDialogState, PathBuf, MDEvent, Error> for MDFileDialogState {
    fn engage(
        &mut self,
        prepare: impl FnOnce(&mut FileDialogState) -> Result<Control<MDEvent>, Error>,
        handle: fn(PathBuf) -> Result<Control<MDEvent>, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let r = prepare(&mut self.file_dlg);
        if r.is_ok() {
            self.handle = Some(handle);
        }
        r
    }

    fn handle(&mut self, event: &Event) -> Result<Control<MDEvent>, Error> {
        try_flow!(match self.file_dlg.handle(event, Dialog)? {
            FileOutcome::Ok(path) => {
                if let Some(handle) = self.handle.take() {
                    handle(path)?
                } else {
                    Control::Changed
                }
            }
            FileOutcome::Cancel => {
                Control::Changed
            }
            r => r.into(),
        });
        Ok(Control::Continue)
    }
}

impl HasScreenCursor for MDFileDialogState {
    fn screen_cursor(&self) -> Option<(u16, u16)> {
        self.file_dlg.screen_cursor()
    }
}
