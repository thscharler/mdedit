use crate::editor_file;
use crate::editor_file::MDFileState;
use crate::global::event::{MDEvent, MDImmediate};
use crate::global::theme::MDWidgets;
use crate::global::GlobalState;
use anyhow::Error;
use log::error;
use rat_salsa::timer::TimerDef;
use rat_salsa::{Control, SalsaContext};
use rat_theme4::WidgetStyle;
use rat_widget::event::{ct_event, try_flow, ConsumedEvent, HandleEvent, Regular, TabbedOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::splitter::{Split, SplitState, SplitType};
use rat_widget::tabbed::{TabType, Tabbed, TabbedState};
use rat_widget::text::undo_buffer::UndoEntry;
use rat_widget::text::TextStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, StatefulWidget};
use std::cmp::max;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SplitTabState {
    pub container: FocusFlag,

    pub sel_split: Option<usize>,
    pub sel_tab: Option<usize>,

    pub split: SplitState,
    pub split_tab: Vec<TabbedState>,
    pub split_tab_file: Vec<Vec<MDFileState>>,
}

impl Default for SplitTabState {
    fn default() -> Self {
        Self {
            container: FocusFlag::new().with_name("split_tab"),
            sel_split: Default::default(),
            sel_tab: Default::default(),
            split: SplitState::named("splitter"),
            split_tab: Default::default(),
            split_tab_file: Default::default(),
        }
    }
}

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut SplitTabState,
    ctx: &mut GlobalState,
) -> Result<(), Error> {
    let (split_layout, split) = Split::horizontal()
        .constraints(vec![Constraint::Fill(1); state.split_tab.len()])
        .mark_offset(2)
        .split_type(SplitType::Scroll)
        .styles(ctx.theme.style(WidgetStyle::SPLIT))
        .into_widgets();
    split_layout.render(area, buf, &mut state.split);

    if state.split.widget_areas.is_empty() {
        buf.set_style(
            area,
            ctx.theme
                .style::<TextStyle>(WidgetStyle::TEXT_DOCUMENT)
                .style,
        );
    }

    let max_idx_split = state.split.widget_areas.len().saturating_sub(1);
    for (idx_split, edit_area) in state.split.widget_areas.iter().enumerate() {
        Tabbed::new()
            .tab_type(TabType::Attached)
            .closeable(true)
            .block(Block::bordered().borders(Borders::TOP | Borders::RIGHT))
            .styles(ctx.theme.style(WidgetStyle::TABBED))
            .tabs(state.split_tab_file[idx_split].iter().map(|v| {
                let title = format!(
                    "{}{}",
                    v.path.file_name().unwrap_or_default().to_string_lossy(),
                    if v.changed { " \u{1F5AB}" } else { "" }
                );
                Line::from(title)
            }))
            .render(*edit_area, buf, &mut state.split_tab[idx_split]);

        if let Some(idx_tab) = state.split_tab[idx_split].selected() {
            editor_file::render(
                0, // if max_idx_split == idx_split { 0 } else { 1 },
                state.split_tab[idx_split].widget_area,
                buf,
                &mut state.split_tab_file[idx_split][idx_tab],
                ctx,
            )?;
        } else {
            // should not occur?
            buf.set_style(
                state.split_tab[idx_split].widget_area,
                Style::new().on_red(),
            );
        }
    }

    split.render(area, buf, &mut state.split);

    Ok(())
}

impl HasFocus for SplitTabState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.split);
        for (idx_split, tabbed) in self.split_tab.iter().enumerate() {
            builder.widget(&self.split_tab[idx_split]);
            if let Some(idx_tab) = tabbed.selected() {
                builder.widget(&self.split_tab_file[idx_split][idx_tab]);
            }
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

pub fn init(_state: &mut SplitTabState, _ctx: &mut GlobalState) -> Result<(), Error> {
    Ok(())
}

pub fn event(
    state: &mut SplitTabState,
    event: &MDEvent,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    // establish focus
    if ctx.focus().gained_focus().is_some() {
        let idx = 'sel: {
            for (idx_split, split) in state.split_tab_file.iter().enumerate() {
                for (idx_tab, tab) in split.iter().enumerate() {
                    if tab.gained_focus() {
                        break 'sel Some((idx_split, idx_tab));
                    }
                }
            }
            None
        };
        if let Some((idx_split, idx_tab)) = idx {
            state.select((idx_split, idx_tab), ctx);
        }
    }

    if let MDEvent::Event(event) = event {
        try_flow!(state.split.handle(event, Regular));

        match event {
            ct_event!(keycode press ALT-Left) => try_flow! {
                if let Some(sel_split) = state.sel_split {
                    if sel_split > 0 {
                        state.sel_split = Some(sel_split - 1);
                        state.sel_tab = state.split_tab[sel_split - 1].selected();

                        let sel_tab = state.sel_tab.unwrap_or(0);
                        ctx.focus_mut().update_container(state);
                        ctx.focus().focus(&state.split_tab_file[sel_split - 1][sel_tab]);
                        Control::Changed
                    } else {
                        Control::Continue
                    }
                } else {
                    Control::Continue
                }
            },
            ct_event!(keycode press ALT-Right) => try_flow! {
                if let Some(sel_split) = state.sel_split {
                    if sel_split + 1 < state.split_tab.len() {
                        state.sel_split = Some(sel_split + 1);
                        state.sel_tab = state.split_tab[sel_split + 1].selected();

                        let sel_tab = state.sel_tab.unwrap_or(0);
                        ctx.focus_mut().update_container(state);
                        ctx.focus().focus(&state.split_tab_file[sel_split + 1][sel_tab]);
                        Control::Changed
                    } else {
                        Control::Continue
                    }
                } else {
                    Control::Continue
                }
            },
            _ => {}
        }

        let (idx_split, r) = 'tab: {
            for (idx_split, tabbed) in state.split_tab.iter_mut().enumerate() {
                let r = tabbed.handle(event, Regular);
                if r.is_consumed() {
                    break 'tab (idx_split, r);
                }
            }
            (0, TabbedOutcome::Continue)
        };
        try_flow!(match r {
            TabbedOutcome::Close(n) => {
                state.close((idx_split, n), ctx)?;
                state.focus_selected(ctx);
                Control::Event(MDEvent::Immediate(MDImmediate::TabClosed))
            }
            TabbedOutcome::Select(n) => {
                state.select((idx_split, n), ctx);
                state.focus_selected(ctx);
                Control::Changed
            }
            r => r.into(),
        });
    }

    if let MDEvent::Event(_) = event {
        // forward only to the selected tab.
        for (idx_split, tabbed) in state.split_tab.iter_mut().enumerate() {
            if let Some(idx_tab) = tabbed.selected() {
                try_flow!(editor_file::event(
                    event,
                    &mut state.split_tab_file[idx_split][idx_tab],
                    ctx
                )?);
            }
        }
    } else {
        // application events go everywhere
        try_flow!({
            let mut r = Control::Continue;
            for tab in &mut state.split_tab_file {
                for ed in tab {
                    r = max(r, editor_file::event(event, ed, ctx)?)
                }
            }
            r
        })
    }

    Ok(Control::Continue)
}

impl SplitTabState {
    // Assert that focus and selection are in sync.
    pub fn assert_selection(&mut self) {
        // Find which split contains the current focus.
        let mut new_split = self.sel_split;
        let mut new_tab = self.sel_tab;

        for (idx_split, tabbed) in self.split_tab.iter().enumerate() {
            if let Some(idx_tab) = tabbed.selected() {
                if self.split_tab_file[idx_split][idx_tab].is_focused() {
                    new_split = Some(idx_split);
                    new_tab = Some(idx_tab);
                    break;
                }
            }
        }

        assert_eq!(self.sel_split, new_split);
        assert_eq!(self.sel_tab, new_tab);
    }

    // Add file at position (split-idx, tab-idx).
    pub fn open(&mut self, pos: (usize, usize), new: MDFileState, _ctx: &mut GlobalState) {
        if pos.0 > self.split_tab_file.len() {
            error!("open split-offset {} invalid.", pos.0);
            return;
        }
        if pos.0 == self.split_tab_file.len() {
            self.split_tab_file.push(Vec::new());
            self.split_tab
                .push(TabbedState::named(format!("tabbed-{}", pos.0).as_str()));
        }
        if let Some(sel_tab) = self.split_tab[pos.0].selected() {
            if sel_tab >= pos.1 {
                self.split_tab[pos.0].select(Some(sel_tab + 1));
            }
        } else {
            self.split_tab[pos.0].select(Some(0));
        }

        if pos.1 > self.split_tab_file[pos.0].len() {
            error!("open tab-offset {} invalid.", pos.1);
            return;
        }
        self.split_tab_file[pos.0].insert(pos.1, new);
    }

    // Close tab (split-idx, tab-idx).
    pub fn close(&mut self, pos: (usize, usize), _ctx: &mut GlobalState) -> Result<(), Error> {
        if pos.0 < self.split_tab_file.len() {
            if pos.1 < self.split_tab_file[pos.0].len() {
                self.split_tab_file[pos.0][pos.1].save()?;

                // remove tab
                self.split_tab_file[pos.0].remove(pos.1);

                if let Some(sel_tab) = self.split_tab[pos.0].selected() {
                    let new_tab = if sel_tab >= pos.1 {
                        if sel_tab < self.split_tab_file[pos.0].len() {
                            Some(sel_tab)
                        } else if self.split_tab_file[pos.0].len() > 0 {
                            Some(self.split_tab_file[pos.0].len() - 1)
                        } else {
                            None
                        }
                    } else {
                        Some(sel_tab)
                    };
                    self.sel_tab = new_tab;
                    self.split_tab[pos.0].select(new_tab);
                }

                // maybe remove split
                if self.split_tab_file[pos.0].len() == 0 {
                    self.split_tab_file.remove(pos.0);
                    self.split_tab.remove(pos.0);

                    if let Some(sel_split) = self.sel_split {
                        let new_split = if sel_split >= pos.0 {
                            if sel_split < self.split_tab_file.len() {
                                Some(sel_split)
                            } else if self.split_tab_file.len() > 0 {
                                Some(self.split_tab_file.len() - 1)
                            } else {
                                None
                            }
                        } else {
                            Some(sel_split)
                        };

                        self.sel_split = new_split;
                        if let Some(new_split) = new_split {
                            self.sel_tab = Some(0);
                            self.split_tab[new_split].select(Some(0));
                        } else {
                            self.sel_tab = None;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // Select by (split-idx, tab-idx)
    pub fn select(&mut self, pos: (usize, usize), _ctx: &mut GlobalState) {
        if pos.0 < self.split_tab_file.len() {
            if pos.1 < self.split_tab_file[pos.0].len() {
                self.sel_split = Some(pos.0);
                self.sel_tab = Some(pos.1);
                self.split_tab[pos.0].select(Some(pos.1));
            }
        }
    }

    // Rebuild focus and focus selected
    pub fn focus_selected(&mut self, ctx: &mut GlobalState) {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.sel_tab {
                ctx.focus_mut().update_container(self);
                ctx.focus().focus(&self.split_tab_file[idx_split][idx_tab]);
            }
        }
    }

    // Select next split
    pub fn select_next(&mut self, ctx: &mut GlobalState) -> bool {
        if let Some(idx_split) = self.sel_split {
            if idx_split + 1 < self.split_tab_file.len() {
                let new_split = idx_split + 1;
                let new_tab = self.split_tab[new_split].selected().unwrap_or_default();
                self.select((new_split, new_tab), ctx);
                self.focus_selected(ctx);
                return true;
            }
        }
        false
    }

    // Select prev split
    pub fn select_prev(&mut self, ctx: &mut GlobalState) -> bool {
        if let Some(idx_split) = self.sel_split {
            if idx_split > 0 {
                let new_split = idx_split - 1;
                let new_tab = self.split_tab[new_split].selected().unwrap_or_default();
                self.select((new_split, new_tab), ctx);
                self.focus_selected(ctx);
                return true;
            }
        }
        false
    }

    // Position of the current focus.
    pub fn selected_pos(&self) -> Option<(usize, usize)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.split_tab[idx_split].selected() {
                return Some((idx_split, idx_tab));
            }
        }
        None
    }

    // Last known focus and position.
    pub fn selected(&self) -> Option<((usize, usize), &MDFileState)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.split_tab[idx_split].selected() {
                return Some((
                    (idx_split, idx_tab),
                    &self.split_tab_file[idx_split][idx_tab],
                ));
            }
        }
        None
    }

    // Last known focus and position.
    pub fn selected_mut(&mut self) -> Option<((usize, usize), &mut MDFileState)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.split_tab[idx_split].selected() {
                return Some((
                    (idx_split, idx_tab),
                    &mut self.split_tab_file[idx_split][idx_tab],
                ));
            }
        }
        None
    }

    // Find the editor for the path.
    pub fn for_path(&self, path: &Path) -> Option<((usize, usize), &MDFileState)> {
        for (idx_split, tabs) in self.split_tab_file.iter().enumerate() {
            for (idx_tab, tab) in tabs.iter().enumerate() {
                if tab.path == path {
                    return Some(((idx_split, idx_tab), tab));
                }
            }
        }
        None
    }

    // Find the editor for the path.
    pub fn for_path_mut(&mut self, path: &Path) -> Option<((usize, usize), &mut MDFileState)> {
        for (idx_split, tabs) in self.split_tab_file.iter_mut().enumerate() {
            for (idx_tab, tab) in tabs.iter_mut().enumerate() {
                if tab.path == path {
                    return Some(((idx_split, idx_tab), tab));
                }
            }
        }
        None
    }

    // Save all files.
    pub fn save(&mut self) -> Result<(), Error> {
        for (_idx_split, tabs) in self.split_tab_file.iter_mut().enumerate() {
            for (_idx_tab, tab) in tabs.iter_mut().enumerate() {
                tab.save()?
            }
        }
        Ok(())
    }

    // Run the replay for the file at path.
    pub fn replay(
        &mut self,
        id: (usize, usize),
        path: &Path,
        replay: &[UndoEntry],
        ctx: &mut GlobalState,
    ) {
        for (idx_split, tabs) in self.split_tab_file.iter_mut().enumerate() {
            for (idx_tab, tab) in tabs.iter_mut().enumerate() {
                if id != (idx_split, idx_tab) && tab.path == path {
                    tab.edit.replay_log(replay);
                    // restart timer
                    tab.parse_timer = Some(ctx.replace_timer(
                        tab.parse_timer,
                        TimerDef::new().next(Instant::now() + Duration::from_millis(200)),
                    ));
                }
            }
        }
    }
}
