use crate::event::MDEvent;
use crate::global::GlobalState;
use anyhow::Error;
use rat_salsa::{AppContext, AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{ct_event, try_flow, HandleEvent, MenuOutcome, Popup, Regular};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::list::selection::RowSelection;
use rat_widget::list::{List, ListState};
use rat_widget::menu::{PopupMenu, PopupMenuState};
use rat_widget::popup::PopupConstraint;
use rat_widget::scrolled::Scroll;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, StatefulWidget};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FileList;

#[derive(Debug)]
pub struct FileListState {
    pub container: FocusFlag,
    pub files_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub file_list: ListState<RowSelection>,

    pub popup_pos: (u16, u16),
    pub popup: PopupMenuState,
}

impl Default for FileListState {
    fn default() -> Self {
        Self {
            container: Default::default(),
            files_dir: Default::default(),
            files: vec![],
            file_list: ListState::named("file_list"),
            popup_pos: (0, 0),
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

        let l_file_list =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);

        buf.set_style(l_file_list[0], theme.container_base());

        List::default()
            .scroll(Scroll::new().styles(theme.scroll_style()))
            .items(state.files.iter().map(|v| {
                if let Some(name) = v.file_name() {
                    Line::from(name.to_string_lossy().to_string())
                } else {
                    Line::from("???")
                }
            }))
            .styles(theme.list_style())
            .render(l_file_list[1], buf, &mut state.file_list);

        if state.popup.is_active() {
            PopupMenu::new()
                .block(Block::bordered())
                .constraint(PopupConstraint::Right(
                    Alignment::Left,
                    Rect::new(state.popup_pos.0, state.popup_pos.1, 0, 0),
                ))
                .offset((-1, -1))
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
        _ctx: &mut AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        match event {
            MDEvent::Event(event) => {
                try_flow!(match self.popup.handle(event, Popup) {
                    MenuOutcome::Activated(0) => {
                        Control::Event(MDEvent::MenuNew)
                    }
                    MenuOutcome::Activated(1) => {
                        if let Some(pos) = self.file_list.row_at_clicked(self.popup_pos) {
                            Control::Event(MDEvent::Open(self.files[pos].clone()))
                        } else {
                            Control::Changed
                        }
                    }
                    MenuOutcome::Activated(2) => {
                        Control::Event(MDEvent::Message("buh".into()))
                    }
                    MenuOutcome::Hide => {
                        self.popup.set_active(false);
                        Control::Changed
                    }
                    r => r.into(),
                });

                if self.file_list.is_focused() {
                    try_flow!(match event {
                        ct_event!(keycode press Enter) => {
                            if let Some(row) = self.file_list.selected() {
                                Control::Event(MDEvent::SelectOrOpen(self.files[row].clone()))
                            } else {
                                Control::Continue
                            }
                        }
                        ct_event!(key press '+') => {
                            if let Some(row) = self.file_list.selected() {
                                Control::Event(MDEvent::SelectOrOpenSplit(self.files[row].clone()))
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
                        self.popup_pos = (*x, *y);
                        self.popup.set_active(true);
                        Control::Changed
                    }
                    ct_event!(mouse any for m)
                        if self.file_list.mouse.doubleclick(self.file_list.area, m) =>
                    {
                        if let Some(row) = self.file_list.row_at_clicked((m.column, m.row)) {
                            Control::Event(MDEvent::SelectOrOpen(self.files[row].clone()))
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
        &self.files_dir
    }

    /// Current file
    pub fn current_file(&self) -> Option<&Path> {
        if let Some(selected) = self.file_list.selected() {
            if selected < self.files.len() {
                Some(&self.files[selected])
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Read directory.
    pub fn load(&mut self, dir: &Path) -> Result<(), Error> {
        self.files_dir = dir.into();
        self.files.clear();
        if let Ok(rd) = fs::read_dir(dir) {
            for f in rd {
                let Ok(f) = f else {
                    continue;
                };
                let f = f.path();
                if let Some(ext) = f.extension() {
                    if ext == "md" {
                        self.files.push(f);
                    }
                }
            }
        }
        if self.files.len() > 0 {
            if let Some(sel) = self.file_list.selected() {
                if sel > self.files.len() {
                    self.file_list.select(Some(self.files.len() - 1));
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
        for (i, f) in self.files.iter().enumerate() {
            if file == f {
                self.file_list.select(Some(i));
                break;
            }
        }
        Ok(())
    }
}
