use crate::editor_file::MDFileState;
use crate::file_list::FileListState;
use crate::fsys::FileSysStructure;
use crate::global::event::{MDEvent, MDImmediate};
use crate::global::GlobalState;
use crate::split_tab::SplitTabState;
use crate::{file_list, split_tab};
use anyhow::Error;
use crate::rat_salsa::{Control, SalsaContext};
use rat_theme4::WidgetStyle;
use rat_widget::event::{break_flow, HandleEvent, Outcome, Regular};
use rat_widget::focus::{impl_has_focus, HasFocus};
use rat_widget::splitter::{Split, SplitState, SplitType};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::StatefulWidget;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct MDEditState {
    pub window_cmd: bool,

    pub hidden_files: bool,
    pub split_files: SplitState,
    pub file_list: FileListState,
    pub split_tab: SplitTabState,
}

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut MDEditState,
    ctx: &mut GlobalState,
) -> Result<(), Error> {
    let theme = &ctx.theme;

    let (split_layout, split) = Split::horizontal()
        .styles(theme.style(WidgetStyle::SPLIT))
        .mark_offset(1)
        .constraints([
            Constraint::Length(ctx.cfg.file_split_at),
            Constraint::Fill(1),
        ])
        .split_type(SplitType::FullPlain)
        .into_widgets();
    split_layout.render(area, buf, &mut state.split_files);

    file_list::render(
        state.split_files.widget_areas[0],
        buf,
        &mut state.file_list,
        ctx,
    )?;
    split_tab::render(
        state.split_files.widget_areas[1],
        buf,
        &mut state.split_tab,
        ctx,
    )?;

    split.render(area, buf, &mut state.split_files);

    Ok(())
}

impl_has_focus!(file_list, split_files, split_tab for MDEditState);

pub fn init(state: &mut MDEditState, ctx: &mut GlobalState) -> Result<(), Error> {
    file_list::init(&mut state.file_list, ctx)?;
    split_tab::init(&mut state.split_tab, ctx)?;
    Ok(())
}

pub fn event(
    event: &MDEvent,
    state: &mut MDEditState,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    // let et = SystemTime::now();

    let old_selected = state.split_tab.selected_pos();
    let mut sync_files = false;

    let r = 'f: {
        let rr = match event {
            MDEvent::Event(event) => {
                // main split between file-list and editors
                match state.split_files.handle(event, Regular) {
                    Outcome::Changed => {
                        if !state.split_files.is_hidden(0) {
                            ctx.cfg.file_split_at = state.split_files.area_len(0);
                            ctx.queue(Control::Event(MDEvent::StoreConfig));
                        }
                        Control::Changed
                    }
                    r => r.into(),
                }
            }
            MDEvent::New(p) => state.new(p, ctx)?,
            MDEvent::SelectOrOpen(p) => state.select_or_open(p, ctx)?,
            MDEvent::SelectOrOpenSplit(p) => state.select_or_open_split(p, ctx)?,
            MDEvent::Open(p) => state.open(p, ctx)?,
            MDEvent::Save => {
                sync_files = true;
                state.save(ctx)?
            }
            MDEvent::SaveAs(p) => state.save_as(p, ctx)?,
            MDEvent::Close => state.close_selected_tab(ctx)?,
            MDEvent::CloseAll => state.close_all(ctx)?,
            MDEvent::CloseAt(idx_split, idx_tab) => {
                state.close_tab_at(*idx_split, *idx_tab, ctx)?
            }
            MDEvent::SelectAt(idx_split, idx_tab) => {
                state.select_tab_at(*idx_split, *idx_tab, ctx)?
            }
            MDEvent::Split => state.split(ctx)?,
            MDEvent::JumpToTree => state.jump_to_tree(ctx)?,
            MDEvent::JumpToFiles => state.jump_to_file(ctx)?,
            MDEvent::JumpToTabs => state.jump_to_tabs(ctx)?,
            MDEvent::JumpToFileSplit => state.jump_to_filesplit(ctx)?,
            MDEvent::JumpToEditSplit => state.jump_to_edit_split(ctx)?,
            MDEvent::PrevEditSplit => state.split_tab.select_prev(ctx).into(),
            MDEvent::NextEditSplit => state.split_tab.select_next(ctx).into(),
            MDEvent::HideFiles => state.hide_files(ctx)?,
            MDEvent::SyncEdit => state.roll_forward_edit(ctx)?,
            MDEvent::SyncFileList => {
                sync_files = true;
                Control::Continue
            }
            MDEvent::FileSysChanged(fs) => {
                state.file_list.replace_fs(fs.take());
                file_list::init(&mut state.file_list, ctx)?;
                state.jump_to_file(ctx)?
            }
            MDEvent::FileSysReloaded(fs) => {
                state.file_list.replace_fs(fs.take());
                file_list::init(&mut state.file_list, ctx)?;
                if !state.split_files.is_hidden(0) {
                    Control::Changed
                } else {
                    Control::Continue
                }
            }
            _ => Control::Continue,
        };
        break_flow!('f: rr);

        break_flow!('f: file_list::event(&mut state.file_list, event, ctx)?);

        break_flow!('f: match split_tab::event(&mut state.split_tab, event, ctx)? {
            Control::Event(MDEvent::Immediate(MDImmediate::TabClosed)) => {
                if state.split_tab.sel_split.is_none() {
                    state.file_list.focus_files(ctx);
                }
                Control::Changed
            }
            r => r,
        });

        Control::Continue
    };

    // global auto sync
    state.auto_hide_files();
    state.split_tab.assert_selection();

    let selected = state.split_tab.selected_pos();
    if selected != old_selected || sync_files {
        let f = state.sync_file_list(sync_files, ctx)?;
        ctx.queue(f);
    }

    Ok(r)
}

impl MDEditState {
    // Open new file.
    pub fn new(&mut self, path: &Path, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
    pub fn open(&mut self, path: &Path, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        let pos = if let Some(pos) = self.split_tab.selected_pos() {
            (pos.0, pos.1 + 1)
        } else {
            (0, 0)
        };

        self.open_in(pos, path, ctx)
    }

    // Open path as new split.
    fn _open_split(
        &mut self,
        path: &Path,
        ctx: &mut GlobalState,
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

        self.open_in(pos, path, ctx)
    }

    /// Open in split/tab.
    pub fn open_in(
        &mut self,
        pos: (usize, usize),
        path: &Path,
        ctx: &mut GlobalState,
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
        ctx: &mut GlobalState,
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
                    self.file_list.load(parent, &ctx.cfg.globs)?;
                    self.file_list.select(&path)?;
                    Control::Changed
                } else if self.file_list.current_dir() != parent || refresh {
                    self.file_list.load_current(parent, &ctx.cfg.globs)?;
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
    fn roll_forward_edit(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
        ctx: &mut GlobalState,
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
        ctx: &mut GlobalState,
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
    pub fn save(&mut self, _ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        self.split_tab.save()?;
        Ok(Control::Changed)
    }

    // Editor in tab
    pub fn editor_at(&mut self, idx_split: usize, idx_tab: usize) -> Option<&mut MDFileState> {
        if idx_split < self.split_tab.split_tab_file.len() {
            if idx_tab < self.split_tab.split_tab_file[idx_split].len() {
                return Some(&mut self.split_tab.split_tab_file[idx_split][idx_tab]);
            }
        }
        None
    }

    // Select tab
    pub fn select_tab_at(
        &mut self,
        idx_split: usize,
        idx_tab: usize,
        ctx: &mut GlobalState,
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
        ctx: &mut GlobalState,
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
    pub fn close_selected_tab(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
    pub fn close_all(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
        _ctx: &mut GlobalState,
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
    pub fn hide_files(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
    pub fn jump_to_tree(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
    pub fn jump_to_file(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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

    pub fn jump_to_tabs(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        if let Some((pos, sel)) = self.split_tab.selected() {
            if sel.is_focused() {
                ctx.focus().focus(&self.split_tab.split_tab[pos.0]);
            } else {
                ctx.focus().focus(sel);
            }
        }
        Ok(Control::Changed)
    }

    pub fn jump_to_edit_split(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        if self.split_tab.split.is_focused() {
            ctx.focus().next();
        } else {
            ctx.focus().focus(&self.split_tab);
        }
        Ok(Control::Changed)
    }

    // Jump to Split
    pub fn jump_to_filesplit(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        if self.split_files.is_focused() {
            ctx.focus().next();
        } else {
            ctx.focus().focus(&self.split_files);
        }
        Ok(Control::Changed)
    }

    // Split current buffer.
    pub fn split(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
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
