use crate::event::MDEvent;
use crate::file_list::{FileList, FileListState};
use crate::global::GlobalState;
use crate::md_file::MDFileState;
use crate::split_tab::{SplitTab, SplitTabState};
use crate::AppContext;
use crate::FocusFlag;
use anyhow::Error;
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{try_flow, ConsumedEvent, HandleEvent, Regular};
use rat_widget::focus::{impl_has_focus, FocusBuilder, HasFocus};
use rat_widget::splitter::{Split, SplitState, SplitType};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::StatefulWidget;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct MDEdit;

#[derive(Debug, Default)]
pub struct MDEditState {
    pub window_cmd: bool,

    pub hidden_files: bool,
    pub split_files: SplitState,
    pub file_list: FileListState,
    pub split_tab: SplitTabState,
}

impl AppWidget<GlobalState, MDEvent, Error> for MDEdit {
    type State = MDEditState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let theme = &ctx.g.theme;

        let (split, split_overlay) = Split::horizontal()
            .styles(theme.split_style())
            .mark_offset(1)
            .constraints([Constraint::Length(15), Constraint::Fill(1)])
            .split_type(SplitType::FullEmpty)
            .into_widgets();

        split.render(area, buf, &mut state.split_files);

        FileList.render(
            state.split_files.widget_areas[0],
            buf,
            &mut state.file_list,
            ctx,
        )?;

        SplitTab.render(
            state.split_files.widget_areas[1],
            buf,
            &mut state.split_tab,
            ctx,
        )?;

        split_overlay.render(area, buf, &mut state.split_files);

        Ok(())
    }
}

impl_has_focus!(file_list, split_files, split_tab for MDEditState);

impl AppState<GlobalState, MDEvent, Error> for MDEditState {
    fn init(&mut self, _ctx: &mut AppContext<'_>) -> Result<(), Error> {
        self.file_list.load(&Path::new("."))?;
        Ok(())
    }

    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r = match event {
            MDEvent::Event(event) => {
                try_flow!(self.split_files.handle(event, Regular));
                Control::Continue
            }
            MDEvent::New(p) => self.new(p, ctx)?,
            MDEvent::SelectOrOpen(p) => self.select_or_open(p, ctx)?,
            MDEvent::SelectOrOpenSplit(p) => self.select_or_open_split(p, ctx)?,
            MDEvent::Open(p) => self.open(p, ctx)?,
            MDEvent::Save => self.save(ctx)?,
            MDEvent::SaveAs(p) => self.save_as(p, ctx)?,
            MDEvent::Close => self.close_selected_tab(ctx)?,
            MDEvent::CloseAt(idx_split, idx_tab) => self.close_tab_at(*idx_split, *idx_tab, ctx)?,
            MDEvent::SelectAt(idx_split, idx_tab) => {
                self.select_tab_at(*idx_split, *idx_tab, ctx)?
            }
            MDEvent::Split => self.split(ctx)?,
            MDEvent::JumpToTree => self.jump_to_tree(ctx)?,
            MDEvent::JumpToFiles => self.jump_to_file(ctx)?,
            MDEvent::JumpToTabs => self.jump_to_tabs(ctx)?,
            MDEvent::JumpToFileSplit => self.jump_to_filesplit(ctx)?,
            MDEvent::JumpToEditSplit => self.jump_to_edit_split(ctx)?,
            MDEvent::PrevEditSplit => self.split_tab.select_prev(ctx).into(),
            MDEvent::NextEditSplit => self.split_tab.select_next(ctx).into(),
            MDEvent::HideFiles => self.hide_files(ctx)?,
            MDEvent::SyncEdit => self.sync_edit(ctx)?,
            _ => Control::Continue,
        };

        r = r.or_else_try(|| self.file_list.event(event, ctx))?;
        r = r.or_else_try(|| self.split_tab.event(event, ctx))?;

        Ok(r)
    }
}

impl MDEditState {
    // Open new file.
    pub fn new(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let pos = if let Some(pos) = self.split_tab.selected_pos() {
            (pos.0, pos.1 + 1)
        } else {
            (0, 0)
        };

        let new = MDFileState::new_file(&path, ctx);
        self.split_tab.open(pos, new, ctx);
        self.split_tab.select(pos, ctx);

        Ok(Control::Changed)
    }

    // Open path.
    pub fn open_split(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let pos = if let Some(pos) = self.split_tab.selected_pos() {
            if pos.0 + 1 >= self.split_tab.tabs.len() {
                (pos.0 + 1, 0)
            } else {
                if let Some(sel_tab) = self.split_tab.tabbed[pos.0 + 1].selected() {
                    (pos.0 + 1, sel_tab + 1)
                } else {
                    (pos.0 + 1, 0)
                }
            }
        } else {
            (0, 0)
        };

        self._open(pos, path, ctx)
    }

    // Open path.
    pub fn open(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let pos = if let Some(pos) = self.split_tab.selected_pos() {
            (pos.0, pos.1 + 1)
        } else {
            (0, 0)
        };

        self._open(pos, path, ctx)
    }

    fn _open(
        &mut self,
        pos: (usize, usize),
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let new = if let Some((_, md)) = self.split_tab.for_path_mut(path) {
            // enable replay and clone the buffer
            if let Some(undo) = md.edit.undo_buffer_mut() {
                undo.enable_replay_log(true);
            }
            md.clone()
        } else {
            MDFileState::open_file(path, ctx)?
        };
        self.split_tab.open(pos, new, ctx);
        self.split_tab.select(pos, ctx);

        if let Some(parent) = path.parent() {
            self.file_list.load(parent)?;
        }
        self.file_list.select(path)?;

        Ok(Control::Changed)
    }

    // Focus path or open file.
    pub fn select_or_open(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, _md)) = self.split_tab.for_path(path) {
            self.split_tab.select(pos, ctx);
            Ok(Control::Changed)
        } else {
            self.open(path, ctx)
        }
    }

    // Focus path or open file.
    pub fn select_or_open_split(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, _md)) = self.split_tab.for_path(path) {
            self.split_tab.select(pos, ctx);
            Ok(Control::Changed)
        } else {
            self.open_split(path, ctx)
        }
    }

    // Save all.
    pub fn save(&mut self, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.split_tab.save()?;

        self.file_list.load(&self.file_list.sys.files_dir.clone())?;
        if let Some((_, mdfile)) = self.split_tab.selected() {
            self.file_list.select(&mdfile.path)?;
        }

        Ok(Control::Changed)
    }

    // Select tab
    pub fn select_tab_at(
        &mut self,
        idx_split: usize,
        idx_tab: usize,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        self.split_tab.select((idx_split, idx_tab), ctx);
        Ok(Control::Changed)
    }

    // Close tab
    pub fn close_tab_at(
        &mut self,
        idx_split: usize,
        idx_tab: usize,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        self.split_tab.close((idx_split, idx_tab), ctx)?;
        Ok(Control::Changed)
    }

    // Close selected
    pub fn close_selected_tab(
        &mut self,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some(pos) = self.split_tab.selected_pos() {
            self.split_tab.close((pos.0, pos.1), ctx)?;
            Ok(Control::Changed)
        } else {
            Ok(Control::Continue)
        }
    }

    // Save selected as.
    pub fn save_as(
        &mut self,
        path: &Path,
        _ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("md");
        }

        if let Some((_pos, t)) = self.split_tab.selected_mut() {
            t.save_as(&path)?;
        }
        Ok(Control::Changed)
    }

    /// Autohide file-list if so
    pub fn auto_hide_files(&mut self) {
        if !self.file_list.is_focused() && self.hidden_files {
            self.split_files.hide_split(0);
        }
    }

    // Hide files
    pub fn hide_files(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if self.hidden_files {
            self.hidden_files = false;
            self.split_files.show_split(0);
        } else {
            self.hidden_files = true;
            self.split_files.hide_split(0);
            if self.file_list.is_focused() {
                ctx.focus().next();
            }
        }
        Ok(Control::Changed)
    }

    // Select tree
    pub fn jump_to_tree(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let mut r = Control::Continue;

        if self.split_files.is_hidden(0) {
            self.split_files.show_split(0);
            r = Control::Changed;
        }
        if !self.file_list.f_sys.is_focused() {
            ctx.focus().focus(&self.file_list.f_sys);
            r = Control::Changed;
        } else {
            if let Some((_, last_edit)) = self.split_tab.selected() {
                ctx.focus().focus(last_edit);
                r = Control::Changed;
            }
        }

        Ok(r)
    }

    // Select Files
    pub fn jump_to_file(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let mut r = Control::Continue;

        if self.split_files.is_hidden(0) {
            self.split_files.show_split(0);
            r = Control::Changed;
        }
        if !self.file_list.file_list.is_focused() {
            ctx.focus().focus(&self.file_list.file_list);
            r = Control::Changed;
        } else {
            if let Some((_, last_edit)) = self.split_tab.selected() {
                ctx.focus().focus(last_edit);
                r = Control::Changed;
            }
        }

        Ok(r)
    }

    pub fn jump_to_tabs(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, sel)) = self.split_tab.selected() {
            if sel.is_focused() {
                ctx.focus().focus(&self.split_tab.tabbed[pos.0]);
            } else {
                ctx.focus().focus(sel);
            }
        }
        Ok(Control::Changed)
    }

    pub fn jump_to_edit_split(
        &mut self,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if self.split_tab.splitter.is_focused() {
            ctx.focus().next();
        } else {
            ctx.focus().focus(&self.split_tab);
        }
        Ok(Control::Changed)
    }

    // Jump to Split
    pub fn jump_to_filesplit(
        &mut self,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if self.split_files.is_focused() {
            ctx.focus().next();
        } else {
            ctx.focus().focus(&self.split_files);
        }
        Ok(Control::Changed)
    }

    // Split current buffer.
    pub fn split(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let Some((pos, sel)) = self.split_tab.selected_mut() else {
            return Ok(Control::Continue);
        };

        // enable replay and clone the buffer
        if let Some(undo) = sel.edit.undo_buffer_mut() {
            undo.enable_replay_log(true);
        }
        let new = sel.clone();

        let new_pos = if pos.0 + 1 == self.split_tab.tabs.len() {
            (pos.0 + 1, 0)
        } else {
            (pos.0 + 1, self.split_tab.tabs[pos.0 + 1].len())
        };
        self.split_tab.open(new_pos, new, ctx);
        self.split_tab.select(pos, ctx);

        Ok(Control::Changed)
    }

    // Establish the currently focus split+tab as the active split.
    pub fn establish_active_split(&mut self) -> bool {
        self.split_tab.establish_active_split()
    }

    // Sync views.
    pub fn sync_file_list(&mut self, ctx: &mut AppContext<'_>) -> Result<(), Error> {
        let path = if let Some((_, md)) = self.split_tab.selected() {
            Some(md.path.clone())
        } else {
            None
        };
        if let Some(path) = path {
            if self.sync_files(&path, ctx)? == Control::Changed {
                ctx.queue(Control::Changed);
            }
        }
        Ok(())
    }

    // Sync file-list with the given file.
    fn sync_files(
        &mut self,
        file: &Path,
        _ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some(parent) = file.parent() {
            if self.file_list.current_dir() != parent {
                self.file_list.load(parent)?;
                self.file_list.select(file)?;
                Ok(Control::Changed)
            } else if self.file_list.current_file() != Some(file) {
                self.file_list.select(file)?;
                Ok(Control::Changed)
            } else {
                Ok(Control::Unchanged)
            }
        } else {
            Ok(Control::Unchanged)
        }
    }

    /// Synchronize all editor instances of one file.
    pub fn sync_edit(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        // synchronize instances
        let (id_sel, sel_path, replay) = if let Some((id_sel, sel)) = self.split_tab.selected_mut()
        {
            (id_sel, sel.path.clone(), sel.edit.recent_replay_log())
        } else {
            ((0, 0), PathBuf::default(), Vec::default())
        };
        if !replay.is_empty() {
            self.split_tab.replay(id_sel, &sel_path, &replay, ctx);
        }
        Ok(Control::Changed)
    }
}
