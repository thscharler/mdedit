use crate::event::MDEvent;
use anyhow::Error;
use rat_salsa::Control;
use rat_widget::event::{try_flow, Dialog, FileOutcome, HandleEvent};
use rat_widget::file_dialog::{FileDialog, FileDialogState, FileDialogStyle};
use rat_widget::text::HasScreenCursor;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;
use std::path::PathBuf;

/// Multi-purpose facilities.
///
/// Primary use-case is to reuse the same file-dialog for different
/// scenarios (Open, Save, ...).
///
/// This brings the need to configure the file-dialog and to
/// map its outcomes to some specific action.
///
pub trait Facility<T, O, A, E>
where
    Self: HandleEvent<A, Dialog, Result<Control<A>, E>>,
{
    /// Engage with the facility.
    /// Set up its current config and set a handler for any possible outcomes.
    fn engage(
        &mut self,
        init: impl FnOnce(&mut T) -> Result<Control<A>, E>,
        out: fn(O) -> Result<Control<A>, E>,
    ) -> Result<Control<A>, E>;
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
    file_dlg: FileDialogState,
    handle: Option<fn(PathBuf) -> Result<Control<MDEvent>, Error>>,
}

impl MDFileDialogState {
    pub fn active(&self) -> bool {
        self.file_dlg.active
    }
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
}

impl HandleEvent<MDEvent, Dialog, Result<Control<MDEvent>, Error>> for MDFileDialogState {
    fn handle(&mut self, event: &MDEvent, _qualifier: Dialog) -> Result<Control<MDEvent>, Error> {
        if let MDEvent::Event(event) = event {
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
        }
        Ok(Control::Continue)
    }
}

impl HasScreenCursor for MDFileDialogState {
    fn screen_cursor(&self) -> Option<(u16, u16)> {
        self.file_dlg.screen_cursor()
    }
}
