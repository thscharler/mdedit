use crate::fsys::FileSysStructure;
use crate::global::event::MDEvent;
use crate::global::GlobalState;
use anyhow::Error;
use rat_salsa::{Control, SalsaContext};
use rat_widget::choice::{Choice, ChoiceClose, ChoiceSelect, ChoiceState};
use rat_widget::event::{ct_event, try_flow, ChoiceOutcome, HandleEvent, Popup, Regular};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::list::selection::RowSelection;
use rat_widget::list::{List, ListState};
use rat_widget::popup::Placement;
use rat_widget::scrolled::Scroll;
use rat_widget::util::revert_style;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{StatefulWidget, Widget};
use std::cmp::min;
use std::path::{Path, PathBuf};

/// File list widget.
#[derive(Debug)]
pub struct FileListState {
    container: FocusFlag,
    sys: FileSysStructure,

    pub file_system: ChoiceState<PathBuf>,
    pub file_list: ListState<RowSelection>,
}

impl Default for FileListState {
    fn default() -> Self {
        Self {
            container: Default::default(),
            sys: Default::default(),
            file_system: Default::default(),
            file_list: ListState::named("file_list"),
        }
    }
}

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut FileListState,
    ctx: &mut GlobalState,
) -> Result<(), Error> {
    let theme = &ctx.theme;
    let scheme = &ctx.scheme();

    let l_file_list = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(area);

    buf.set_style(l_file_list[0], theme.container_base());

    Line::from(state.sys.name())
        .style(theme.container_base().fg(scheme.green[2]))
        .render(l_file_list[1], buf);

    let popup_len = min(l_file_list[4].height, state.sys.dirs_len() as u16);

    let (choice, choice_popup) = Choice::new()
        .styles(theme.choice_style_tools())
        .items(
            state
                .sys
                .dirs()
                .iter()
                .cloned()
                .zip(state.sys.display().iter().cloned()),
        )
        .popup_scroll(Scroll::new())
        .popup_placement(Placement::Below)
        .popup_len(popup_len)
        .behave_select(ChoiceSelect::MouseClick)
        .behave_close(ChoiceClose::SingleClick)
        .into_widgets();
    choice.render(l_file_list[2], buf, &mut state.file_system);

    buf.set_style(l_file_list[3], theme.container_base());

    List::default()
        .scroll(Scroll::new().styles(theme.scroll_style()))
        .items(state.sys.files().iter().map(|v| {
            if let Some(name) = v.file_name() {
                Line::from(name.to_string_lossy().to_string())
            } else {
                Line::from("???")
            }
        }))
        .styles(theme.list_style())
        .render(l_file_list[4], buf, &mut state.file_list);

    // render hover for overlong file names
    if state.file_list.is_focused() {
        if let Some(selected) = state.file_list.selected() {
            let idx = selected - state.file_list.offset();

            let focus_style = theme
                .list_style()
                .focus
                .unwrap_or(revert_style(theme.list_style().style));

            let line = state.sys.files().iter().nth(idx).map(|v| {
                if let Some(name) = v.file_name() {
                    Line::from(name.to_string_lossy().to_string()).style(focus_style)
                } else {
                    Line::from("???").style(focus_style)
                }
            });
            if let Some(line) = line {
                let mut area = state.file_list.row_areas[idx];
                area.width = line.width() as u16 + 1;

                line.style(focus_style)
                    .render(area, ctx.hover.buffer_mut(area));
            }
        }
    }

    choice_popup.render(l_file_list[2], buf, &mut state.file_system);

    Ok(())
}

impl HasFocus for FileListState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.file_system);
        builder.widget(&self.file_list);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }

    fn area(&self) -> Rect {
        self.file_list.area()
    }
}

pub fn init(state: &mut FileListState, _ctx: &mut GlobalState) -> Result<(), Error> {
    if !state.sys.files_is_empty() {
        state
            .file_system
            .set_value(state.sys.files_dir().to_path_buf());
        state.file_list.select(Some(0));
    }
    Ok(())
}

pub fn event(
    state: &mut FileListState,
    event: &MDEvent,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    if let MDEvent::Event(event) = event {
        try_flow!(match state.file_system.handle(event, Popup) {
            ChoiceOutcome::Value => {
                if matches!(event, ct_event!(keycode press Enter)) {
                    ctx.focus().next();
                }
                let sel_path = state.file_system.value();
                state.load_current(&sel_path, &ctx.cfg.globs)?;
                Control::Changed
            }
            ChoiceOutcome::Changed => {
                if matches!(event, ct_event!(keycode press Enter)) {
                    ctx.focus().next();
                }
                Control::Changed
            }
            r => r.into(),
        });

        if state.file_list.is_focused() {
            try_flow!(match event {
                ct_event!(keycode press Enter) => {
                    if let Some(row) = state.file_list.selected() {
                        Control::Event(MDEvent::SelectOrOpen(state.sys.file(row).into()))
                    } else {
                        Control::Continue
                    }
                }
                ct_event!(key press '+') => {
                    if let Some(row) = state.file_list.selected() {
                        Control::Event(MDEvent::SelectOrOpenSplit(state.sys.file(row).into()))
                    } else {
                        Control::Continue
                    }
                }
                _ => Control::Continue,
            });
        }
        try_flow!(match event {
            ct_event!(mouse any for m)
                if state.file_list.mouse.doubleclick(state.file_list.area, m) =>
            {
                if let Some(row) = state.file_list.row_at_clicked((m.column, m.row)) {
                    Control::Event(MDEvent::SelectOrOpen(state.sys.file(row).into()))
                } else {
                    Control::Continue
                }
            }

            _ => Control::Continue,
        });

        try_flow!(state.file_list.handle(event, Regular));

        Ok(Control::Continue)
    } else {
        Ok(Control::Continue)
    }
}

impl FileListState {
    /// Current root
    pub fn root(&self) -> &Path {
        self.sys.root()
    }

    /// Replace the file-system.
    pub fn replace_fs(&mut self, fs: FileSysStructure) {
        self.sys = fs;
    }

    /// Current directory.
    pub fn current_dir(&self) -> &Path {
        self.sys.files_dir()
    }

    /// Current file
    pub fn current_file(&self) -> Option<&Path> {
        if let Some(selected) = self.file_list.selected() {
            if selected < self.sys.files_len() {
                Some(self.sys.file(selected))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Read directory listing.
    pub fn load_current(&mut self, dir: &Path, globs: &[String]) -> Result<(), Error> {
        self.sys.load_current(dir, globs)?;

        self.file_system
            .set_value(self.sys.files_dir().to_path_buf());

        if self.sys.files_len() > 0 {
            if let Some(sel) = self.file_list.selected() {
                if sel > self.sys.files_len() {
                    self.file_list.move_to(self.sys.files_len() - 1);
                }
            } else {
                self.file_list.move_to(0);
            }
        } else {
            self.file_list.select(None);
        }

        Ok(())
    }

    /// Set directory, find roots.
    pub fn load(&mut self, dir: &Path, globs: &[String]) -> Result<(), Error> {
        self.sys.load(dir, globs)?;

        self.file_system
            .set_value(self.sys.files_dir().to_path_buf());
        self.file_system.set_offset(0);

        if self.sys.files_len() > 0 {
            if let Some(sel) = self.file_list.selected() {
                if sel > self.sys.files_len() {
                    self.file_list.select(Some(self.sys.files_len() - 1));
                }
            } else {
                self.file_list.select(Some(0));
            }
        } else {
            self.file_list.select(None);
        }

        Ok(())
    }

    /// Select this file.
    pub fn select(&mut self, file: &Path) -> Result<(), Error> {
        self.file_list.clear_selection();
        for (i, f) in self.sys.files().iter().enumerate() {
            if file == f {
                self.file_list.select(Some(i));
                break;
            }
        }
        Ok(())
    }

    /// Focus the files list.
    pub fn focus_files(&self, ctx: &GlobalState) -> bool {
        if !self.file_list.is_focused() {
            ctx.focus().focus(&self.file_list);
            true
        } else {
            false
        }
    }

    /// Focus the fs-tree.
    pub fn focus_tree(&mut self, ctx: &GlobalState) -> bool {
        if !self.file_system.is_focused() {
            self.file_system.set_popup_active(true);
            ctx.focus().focus(&self.file_system);
            true
        } else {
            self.file_system.set_popup_active(false);
            false
        }
    }
}
