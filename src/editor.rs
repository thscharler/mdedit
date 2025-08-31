use crate::editor_file::MDFileState;
use crate::event::{MDEvent, MDImmediate};
use crate::file_list::{FileList, FileListState};
use crate::fs_structure::FileSysStructure;
use crate::global::GlobalState;
use crate::split_tab::{SplitTab, SplitTabState};
use crate::AppContext;
use anyhow::Error;
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{try_flow, ConsumedEvent, HandleEvent, Outcome, Regular};
use rat_widget::focus::{impl_has_focus, HasFocus};
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

        let (split, split_layout) = Split::horizontal()
            .styles(theme.split_style())
            .mark_offset(1)
            .constraints([
                Constraint::Length(ctx.g.cfg.file_split_at),
                Constraint::Fill(1),
            ])
            .split_type(SplitType::FullEmpty)
            .into_widget_layout(area, &mut state.split_files);

        FileList.render(split_layout[0], buf, &mut state.file_list, ctx)?;
        SplitTab.render(split_layout[1], buf, &mut state.split_tab, ctx)?;

        split.render(area, buf, &mut state.split_files);

        Ok(())
    }
}

impl_has_focus!(file_list, split_files, split_tab for MDEditState);

impl AppState<GlobalState, MDEvent, Error> for MDEditState {
    fn init(&mut self, ctx: &mut AppContext<'_>) -> Result<(), Error> {
        self.file_list.init(ctx)?;
        self.split_tab.init(ctx)?;
        Ok(())
    }

    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        // let et = SystemTime::now();

        let old_selected = self.split_tab.selected_pos();
        let mut sync_files = false;

        let mut r = match event {
            MDEvent::Event(event) => {
                // main split between file-list and editors
                try_flow!(match self.split_files.handle(event, Regular) {
                    Outcome::Changed => {
                        if !self.split_files.is_hidden(0) {
                            ctx.g.cfg.file_split_at = self.split_files.area_len(0);
                            ctx.queue(Control::Event(MDEvent::StoreConfig));
                        }
                        Control::Changed
                    }
                    r => r.into(),
                });
                Control::Continue
            }
            MDEvent::New(p) => self.new(p, ctx)?,
            MDEvent::SelectOrOpen(p) => self.select_or_open(p, ctx)?,
            MDEvent::SelectOrOpenSplit(p) => self.select_or_open_split(p, ctx)?,
            MDEvent::Open(p) => self.open(p, ctx)?,
            MDEvent::Save => {
                sync_files = true;
                self.save(ctx)?
            }
            MDEvent::SaveAs(p) => self.save_as(p, ctx)?,
            MDEvent::Close => self.close_selected_tab(ctx)?,
            MDEvent::CloseAll => self.close_all(ctx)?,
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
            MDEvent::SyncEdit => self.roll_forward_edit(ctx)?,
            MDEvent::SyncFileList => {
                sync_files = true;
                Control::Continue
            }
            MDEvent::FileSys(fs) => {
                self.file_list.replace_fs(fs.take());
                self.file_list.init(ctx)?;
                self.jump_to_file(ctx)?
            }
            _ => Control::Continue,
        };

        r = r.or_else_try(|| self.file_list.event(event, ctx))?;
        r = r.or_else_try(|| match self.split_tab.event(event, ctx) {
            Ok(Control::Event(MDEvent::Immediate(MDImmediate::TabClosed))) => {
                if self.split_tab.sel_split.is_none() {
                    self.file_list.focus_files(ctx);
                }
                Ok(Control::Changed)
            }
            r => r,
        })?;

        // global auto sync
        self.auto_hide_files();
        self.split_tab.assert_selection();

        let selected = self.split_tab.selected_pos();
        if selected != old_selected || sync_files {
            let f = self.sync_file_list(sync_files, ctx)?;
            ctx.queue(f);
        }

        // debug!("et {:?}", et.elapsed());

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
        self.split_tab.focus_selected(ctx);

        Ok(Control::Changed)
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

    // Open path as new split.
    fn _open_split(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let pos = if let Some(pos) = self.split_tab.selected_pos() {
            if pos.0 + 1 >= self.split_tab.split_tab_file.len() {
                (pos.0 + 1, 0)
            } else {
                if let Some(sel_tab) = self.split_tab.split_tab[pos.0 + 1].selected() {
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
        self.split_tab.focus_selected(ctx);

        Ok(Control::Changed)
    }

    // Sync views.
    pub fn sync_file_list(
        &mut self,
        refresh: bool,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let path = if let Some((_, md)) = self.split_tab.selected() {
            Some(md.path.clone())
        } else {
            None
        };

        Ok(if let Some(path) = path {
            if let Some(parent) = path.parent() {
                let root = FileSysStructure::find_root(parent);
                let root = root.as_deref();

                if Some(self.file_list.root()) != root {
                    self.file_list.load(parent, &ctx.g.cfg.globs)?;
                    self.file_list.select(&path)?;
                    Control::Changed
                } else if self.file_list.current_dir() != parent || refresh {
                    self.file_list.load_current(parent, &ctx.g.cfg.globs)?;
                    self.file_list.select(&path)?;
                    Control::Changed
                } else if self.file_list.current_file() != Some(&path) {
                    self.file_list.select(&path)?;
                    Control::Changed
                } else {
                    Control::Unchanged
                }
            } else {
                Control::Unchanged
            }
        } else {
            Control::Continue
        })
    }

    /// Synchronize all editor instances of one file.
    fn roll_forward_edit(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
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

    // Focus path or open file.
    pub fn select_or_open(
        &mut self,
        path: &Path,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, _md)) = self.split_tab.for_path(path) {
            self.split_tab.select(pos, ctx);
            self.split_tab.focus_selected(ctx);
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
            self.split_tab.focus_selected(ctx);
            Ok(Control::Changed)
        } else {
            self._open_split(path, ctx)
        }
    }

    // Save all.
    pub fn save(&mut self, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.split_tab.save()?;
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
        self.split_tab.focus_selected(ctx);
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
        if self.split_tab.sel_split.is_none() {
            self.file_list.focus_files(ctx);
        } else {
            self.split_tab.focus_selected(ctx);
        }
        Ok(Control::Changed)
    }

    // Close selected
    pub fn close_selected_tab(
        &mut self,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        if let Some(pos) = self.split_tab.selected_pos() {
            self.split_tab.close((pos.0, pos.1), ctx)?;
            if self.split_tab.sel_split.is_none() {
                self.file_list.focus_files(ctx);
            } else {
                self.split_tab.focus_selected(ctx);
            }
            Ok(Control::Changed)
        } else {
            Ok(Control::Continue)
        }
    }

    // Close all
    pub fn close_all(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if let Some(pos) = self.split_tab.selected_pos() {
            for i in (0..self.split_tab.split_tab_file[pos.0].len()).rev() {
                self.split_tab.close((pos.0, i), ctx)?;
            }
            if self.split_tab.sel_split.is_none() {
                self.file_list.focus_files(ctx);
            } else {
                // noop
            }
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
        if !self.file_list.focus_tree(ctx) {
            if let Some((_, last_edit)) = self.split_tab.selected() {
                ctx.focus().focus(last_edit);
                Ok(Control::Changed)
            } else {
                Ok(Control::Continue)
            }
        } else {
            if self.split_files.is_hidden(0) {
                self.split_files.show_split(0);
            }
            Ok(Control::Changed)
        }
    }

    // Select Files
    pub fn jump_to_file(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if !self.file_list.focus_files(ctx) {
            if let Some((_, last_edit)) = self.split_tab.selected() {
                ctx.focus().focus(last_edit);
                Ok(Control::Changed)
            } else {
                Ok(Control::Continue)
            }
        } else {
            if self.split_files.is_hidden(0) {
                self.split_files.show_split(0);
            }
            Ok(Control::Changed)
        }
    }

    pub fn jump_to_tabs(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, sel)) = self.split_tab.selected() {
            if sel.is_focused() {
                ctx.focus().focus(&self.split_tab.split_tab[pos.0]);
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
        if self.split_tab.split.is_focused() {
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
        let Some((pos, sel)) = self.split_tab.selected() else {
            return Ok(Control::Continue);
        };

        let new_split = pos.0 + 1;

        // already open in next split?
        let open_as_tab = if new_split < self.split_tab.split_tab_file.len() {
            self.split_tab.split_tab_file[new_split]
                .iter()
                .enumerate()
                .find(|(_, v)| v.path == sel.path)
        } else {
            None
        };

        if let Some((new_tab, _)) = open_as_tab {
            self.split_tab.select((new_split, new_tab), ctx);
            self.split_tab.focus_selected(ctx);
        } else {
            let Some((_, sel)) = self.split_tab.selected_mut() else {
                return Ok(Control::Continue);
            };
            // enable replay and clone the buffer
            if let Some(undo) = sel.edit.undo_buffer_mut() {
                undo.enable_replay_log(true);
            }
            let new = sel.clone();

            let new_tab = if new_split < self.split_tab.split_tab_file.len() {
                self.split_tab.split_tab_file[new_split].len()
            } else {
                0
            };

            self.split_tab.open((new_split, new_tab), new, ctx);
            self.split_tab.select((new_split, new_tab), ctx);
            self.split_tab.focus_selected(ctx);
        }

        Ok(Control::Changed)
    }
}
