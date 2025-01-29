use crate::event::MDEvent;
use crate::fs_structure::FileSysStructure;
use crate::global::GlobalState;
use anyhow::Error;
use rat_salsa::{AppContext, AppState, AppWidget, Control, RenderContext};
use rat_widget::choice::{Choice, ChoiceClose, ChoiceSelect, ChoiceState};
use rat_widget::event::{
    ct_event, try_flow, ChoiceOutcome, HandleEvent, MenuOutcome, Popup, Regular,
};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::list::selection::RowSelection;
use rat_widget::list::{List, ListState};
use rat_widget::menu::{PopupMenu, PopupMenuState};
use rat_widget::popup::{Placement, PopupConstraint};
use rat_widget::scrolled::Scroll;
use rat_widget::util::{revert_style, union_non_empty};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::prelude::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, StatefulWidget, StatefulWidgetRef, Widget};
use std::cmp::{max, min};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FileList;

#[derive(Debug)]
pub struct FileListState {
    pub container: FocusFlag,

    pub sys: FileSysStructure,

    pub f_sys: ChoiceState<PathBuf>,
    pub file_list: ListState<RowSelection>,

    pub popup_rect: Rect,
    pub popup: PopupMenuState,
}

impl Default for FileListState {
    fn default() -> Self {
        Self {
            container: Default::default(),
            sys: Default::default(),
            f_sys: Default::default(),
            file_list: ListState::named("file_list"),
            popup_rect: Default::default(),
            popup: Default::default(),
        }
    }
}

impl AppWidget<GlobalState, MDEvent, Error> for FileList {
    type State = FileListState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let theme = &ctx.g.theme;
        let scheme = &ctx.g.theme.s();

        let l_file_list = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);

        buf.set_style(l_file_list[0], theme.container_base());

        Line::from(state.sys.name.as_str())
            .style(theme.container_base().fg(scheme.green[2]))
            .render(l_file_list[1], buf);

        let popup_len = min(l_file_list[4].height, state.sys.dirs.len() as u16);

        let (choice, choice_popup) = Choice::new()
            .styles(theme.choice_style_tools())
            .items(
                state
                    .sys
                    .dirs
                    .iter()
                    .cloned()
                    .zip(state.sys.display.iter().cloned()),
            )
            .popup_scroll(Scroll::new())
            .popup_placement(Placement::Below)
            .popup_len(popup_len)
            .behave_select(ChoiceSelect::MouseClick)
            .behave_close(ChoiceClose::SingleClick)
            .into_widgets();
        choice.render_ref(l_file_list[2], buf, &mut state.f_sys);

        buf.set_style(l_file_list[3], theme.container_base());

        List::default()
            .scroll(Scroll::new().styles(theme.scroll_style()))
            .items(state.sys.files.iter().map(|v| {
                if let Some(name) = v.file_name() {
                    Line::from(name.to_string_lossy().to_string())
                } else {
                    Line::from("???")
                }
            }))
            .styles(theme.list_style())
            .render(l_file_list[4], buf, &mut state.file_list);

        if state.file_list.is_focused() && !state.popup.is_active() {
            if let Some(selected) = state.file_list.selected() {
                let idx = selected - state.file_list.offset();

                let focus_style = theme
                    .list_style()
                    .focus
                    .unwrap_or(revert_style(theme.list_style().style));

                let line = state.sys.files.iter().nth(idx).map(|v| {
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
                        .render(area, ctx.g.hover.buffer_mut(area));
                }
            }
        }

        choice_popup.render(l_file_list[2], buf, &mut state.f_sys);

        if state.popup.is_active() {
            PopupMenu::new()
                .block(Block::bordered())
                .constraint(PopupConstraint::AboveOrBelow(
                    Alignment::Left,
                    state.popup_rect,
                ))
                .boundary(state.file_list.area)
                .item_parsed("_New")
                .item_parsed("_Open")
                .item_parsed("_Delete")
                .styles(theme.menu_style())
                .render(Rect::default(), buf, &mut state.popup);
        }

        Ok(())
    }
}

impl HasFocus for FileListState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.f_sys);
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

impl AppState<GlobalState, MDEvent, Error> for FileListState {
    fn init(
        &mut self,
        _ctx: &mut AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<(), Error> {
        self.load(&Path::new("."))?;
        Ok(())
    }

    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        match event {
            MDEvent::Event(event) => {
                try_flow!(match self.popup.handle(event, Popup) {
                    MenuOutcome::Activated(0) => {
                        Control::Event(MDEvent::MenuNew)
                    }
                    MenuOutcome::Activated(1) => {
                        if let Some(pos) = self
                            .file_list
                            .row_at_clicked((self.popup_rect.x, self.popup_rect.y))
                        {
                            Control::Event(MDEvent::Open(self.sys.files[pos].clone()))
                        } else {
                            Control::Changed
                        }
                    }
                    MenuOutcome::Activated(2) => {
                        Control::Event(MDEvent::Message("buh".into()))
                    }
                    MenuOutcome::Hide => {
                        self.popup.set_active(false);
                        ctx.queue(Control::Changed);
                        Control::Continue
                    }
                    r => r.into(),
                });

                if !self.f_sys.is_focused() {
                    // TODO: why is this necessary??
                    self.f_sys.set_popup_active(false);
                }

                try_flow!(match self.f_sys.handle(event, Popup) {
                    ChoiceOutcome::Value => {
                        let sel_path = self.f_sys.value();
                        self.load_current(&sel_path)?;
                        Control::Changed
                    }
                    r => r.into(),
                });

                if self.file_list.is_focused() {
                    try_flow!(match event {
                        ct_event!(keycode press Enter) => {
                            if let Some(row) = self.file_list.selected() {
                                Control::Event(MDEvent::SelectOrOpen(self.sys.files[row].clone()))
                            } else {
                                Control::Continue
                            }
                        }
                        ct_event!(key press '+') => {
                            if let Some(row) = self.file_list.selected() {
                                Control::Event(MDEvent::SelectOrOpenSplit(
                                    self.sys.files[row].clone(),
                                ))
                            } else {
                                Control::Continue
                            }
                        }
                        _ => Control::Continue,
                    });
                }
                try_flow!(match event {
                    ct_event!(mouse down Right for x,y)
                        if self.file_list.area.contains(Position::new(*x, *y)) =>
                    {
                        if let Some(row) = self.file_list.row_at_clicked((*x, *y)) {
                            let row = row - self.file_list.offset();
                            self.popup_rect = self.file_list.row_areas[row];
                            self.popup.set_active(true);
                        }
                        Control::Changed
                    }
                    ct_event!(mouse any for m)
                        if self.file_list.mouse.doubleclick(self.file_list.area, m) =>
                    {
                        if let Some(row) = self.file_list.row_at_clicked((m.column, m.row)) {
                            Control::Event(MDEvent::SelectOrOpen(self.sys.files[row].clone()))
                        } else {
                            Control::Continue
                        }
                    }

                    _ => Control::Continue,
                });

                try_flow!(self.file_list.handle(event, Regular));

                Ok(Control::Continue)
            }
            _ => Ok(Control::Continue),
        }
    }
}

impl FileListState {
    /// Current directory.
    pub fn current_dir(&self) -> &Path {
        &self.sys.files_dir
    }

    /// Current file
    pub fn current_file(&self) -> Option<&Path> {
        if let Some(selected) = self.file_list.selected() {
            if selected < self.sys.files.len() {
                Some(&self.sys.files[selected])
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Read directory.
    pub fn load_current(&mut self, dir: &Path) -> Result<(), Error> {
        self.sys.load_current(dir)?;

        self.f_sys.set_value(self.sys.files_dir.clone());
        self.f_sys.set_offset(0);

        if self.sys.files.len() > 0 {
            if let Some(sel) = self.file_list.selected() {
                if sel > self.sys.files.len() {
                    self.file_list.move_to(self.sys.files.len() - 1);
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
    pub fn load(&mut self, dir: &Path) -> Result<(), Error> {
        self.sys.load(dir)?;

        self.f_sys.set_value(self.sys.files_dir.clone());
        self.f_sys.set_offset(0);

        if self.sys.files.len() > 0 {
            if let Some(sel) = self.file_list.selected() {
                if sel > self.sys.files.len() {
                    self.file_list.select(Some(self.sys.files.len() - 1));
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
        for (i, f) in self.sys.files.iter().enumerate() {
            if file == f {
                self.file_list.select(Some(i));
                break;
            }
        }
        Ok(())
    }
}
