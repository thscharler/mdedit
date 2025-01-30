use crate::editor_file::{MDFile, MDFileState};
use crate::event::MDEvent;
use crate::global::GlobalState;
use crate::AppContext;
use anyhow::Error;
use rat_salsa::timer::TimerDef;
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{try_flow, ConsumedEvent, HandleEvent, Regular, TabbedOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::splitter::{Split, SplitState, SplitType};
use rat_widget::tabbed::{TabType, Tabbed, TabbedState};
use rat_widget::text::undo_buffer::UndoEntry;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::symbols;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct SplitTab;

#[derive(Debug)]
pub struct SplitTabState {
    pub container: FocusFlag,
    pub splitter: SplitState,
    pub sel_split: Option<usize>,
    pub sel_tab: Option<usize>,
    pub tabbed: Vec<TabbedState>,
    pub tabs: Vec<Vec<MDFileState>>,
}

impl Default for SplitTabState {
    fn default() -> Self {
        Self {
            container: FocusFlag::named("split_tab"),
            splitter: SplitState::named("splitter"),
            sel_split: None,
            sel_tab: None,
            tabbed: vec![],
            tabs: vec![],
        }
    }
}

impl AppWidget<GlobalState, MDEvent, Error> for SplitTab {
    type State = SplitTabState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let theme = ctx.g.theme.clone();

        let (split, split_overlay) = Split::horizontal()
            .constraints(vec![Constraint::Fill(1); state.tabbed.len()])
            .mark_offset(0)
            .split_type(SplitType::Scroll)
            .styles(theme.split_style())
            .into_widgets();

        split.render(area, buf, &mut state.splitter);

        let max_idx_split = state.splitter.widget_areas.len().saturating_sub(1);
        for (idx_split, edit_area) in state.splitter.widget_areas.iter().enumerate() {
            let select_style = if let Some((sel_pos, md)) = state.selected() {
                if sel_pos.0 == idx_split {
                    if state.tabbed[idx_split].is_focused() {
                        theme.tabbed_style().focus.expect("style")
                    } else if md.is_focused() {
                        theme.s().primary(1)
                    } else {
                        theme.tabbed_style().select.expect("style")
                    }
                } else {
                    theme.tabbed_style().select.expect("style")
                }
            } else {
                theme.tabbed_style().select.expect("style")
            };

            Tabbed::new()
                .tab_type(TabType::Glued)
                .closeable(true)
                .styles(theme.tabbed_style())
                .select_style(select_style)
                .tabs(state.tabs[idx_split].iter().map(|v| {
                    let title = format!(
                        "{}{}",
                        v.path.file_name().unwrap_or_default().to_string_lossy(),
                        if v.changed { " \u{1F5AB}" } else { "" }
                    );
                    Line::from(title)
                }))
                .render(*edit_area, buf, &mut state.tabbed[idx_split]);

            // fix block rendering
            let fix_area = state.tabbed[idx_split].block_area;
            if let Some(cell) = buf.cell_mut((fix_area.right() - 1, fix_area.y)) {
                cell.set_symbol(symbols::line::ROUNDED_TOP_RIGHT);
            }

            if let Some(idx_tab) = state.tabbed[idx_split].selected() {
                MDFile {
                    start_margin: if max_idx_split == idx_split { 0 } else { 1 },
                }
                .render(
                    state.tabbed[idx_split].widget_area,
                    buf,
                    &mut state.tabs[idx_split][idx_tab],
                    ctx,
                )?;
            }
        }

        split_overlay.render(area, buf, &mut state.splitter);

        Ok(())
    }
}

impl HasFocus for SplitTabState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.splitter);
        for (idx_split, tabbed) in self.tabbed.iter().enumerate() {
            builder.widget(&self.tabbed[idx_split]);
            if let Some(idx_tab) = tabbed.selected() {
                builder.widget(&self.tabs[idx_split][idx_tab]);
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

impl AppState<GlobalState, MDEvent, Error> for SplitTabState {
    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r = match event {
            MDEvent::Event(event) => {
                try_flow!(self.splitter.handle(event, Regular));
                for (idx_split, tabbed) in self.tabbed.iter_mut().enumerate() {
                    try_flow!(match tabbed.handle(event, Regular) {
                        TabbedOutcome::Close(n) => {
                            Control::Event(MDEvent::CloseAt(idx_split, n))
                        }
                        TabbedOutcome::Select(n) => {
                            Control::Event(MDEvent::SelectAt(idx_split, n))
                        }
                        r => r.into(),
                    });
                }
                Control::Continue
            }
            _ => Control::Continue,
        };

        r = r.or_else_try(|| {
            match event {
                MDEvent::Event(_) => {
                    // forward only to the selected tab.
                    for (idx_split, tabbed) in self.tabbed.iter_mut().enumerate() {
                        if let Some(idx_tab) = tabbed.selected() {
                            try_flow!(self.tabs[idx_split][idx_tab].event(event, ctx)?);
                        }
                    }
                }
                _ => {
                    // application events go everywhere
                    for tab in &mut self.tabs {
                        for ed in tab {
                            try_flow!(ed.event(event, ctx)?);
                        }
                    }
                }
            }
            Ok::<_, Error>(Control::Continue)
        })?;

        Ok(r)
    }
}

impl SplitTabState {
    // Establish the active split+tab using the currently focused tab.
    pub fn establish_active_split(&mut self) -> bool {
        // Find which split contains the current focus.
        let old_split = self.sel_split;
        let old_tab = self.sel_tab;

        for (idx_split, tabbed) in self.tabbed.iter().enumerate() {
            if let Some(idx_tab) = tabbed.selected() {
                if self.tabs[idx_split][idx_tab].is_focused() {
                    self.sel_split = Some(idx_split);
                    self.sel_tab = Some(idx_tab);
                    break;
                }
            }
        }

        old_split != self.sel_split || old_tab != self.sel_tab
    }

    // Add file at position (split-idx, tab-idx).
    pub fn open(&mut self, pos: (usize, usize), new: MDFileState, ctx: &mut AppContext<'_>) {
        if pos.0 == self.tabs.len() {
            self.tabs.push(Vec::new());
            self.tabbed
                .push(TabbedState::named(format!("tabbed-{}", pos.0).as_str()));
        }
        if let Some(sel_tab) = self.tabbed[pos.0].selected() {
            if sel_tab >= pos.1 {
                self.tabbed[pos.0].select(Some(sel_tab + 1));
            }
        } else {
            self.tabbed[pos.0].select(Some(0));
        }
        self.tabs[pos.0].insert(pos.1, new);

        ctx.focus_mut().update_container(self);
    }

    // Close tab (split-idx, tab-idx).
    pub fn close(&mut self, pos: (usize, usize), ctx: &mut AppContext<'_>) -> Result<(), Error> {
        if pos.0 < self.tabs.len() {
            if pos.1 < self.tabs[pos.0].len() {
                self.tabs[pos.0][pos.1].save()?;

                // remove tab
                self.tabs[pos.0].remove(pos.1);
                if let Some(sel_tab) = self.tabbed[pos.0].selected() {
                    let new_tab = if sel_tab >= pos.1 {
                        if sel_tab > 0 {
                            Some(sel_tab - 1)
                        } else if self.tabs[pos.0].len() > 0 {
                            Some(0)
                        } else {
                            None
                        }
                    } else {
                        if sel_tab == 0 {
                            if self.tabs[pos.0].len() > 0 {
                                Some(0)
                            } else {
                                None
                            }
                        } else {
                            Some(sel_tab)
                        }
                    };
                    self.tabbed[pos.0].select(new_tab);
                }

                // maybe remove split
                if self.tabs[pos.0].len() == 0 {
                    self.tabs.remove(pos.0);
                    self.tabbed.remove(pos.0);
                    if let Some(sel_split) = self.sel_split {
                        let new_split = if sel_split >= pos.0 {
                            if sel_split > 0 {
                                Some(sel_split - 1)
                            } else if self.tabbed.len() > 0 {
                                Some(0)
                            } else {
                                None
                            }
                        } else {
                            if sel_split == 0 {
                                if self.tabbed.len() > 0 {
                                    Some(0)
                                } else {
                                    None
                                }
                            } else {
                                Some(sel_split)
                            }
                        };
                        self.sel_split = new_split;
                    }
                }

                ctx.focus_mut().update_container(self);
            }
        }
        Ok(())
    }

    // Select by (split-idx, tab-idx)
    pub fn select(&mut self, pos: (usize, usize), ctx: &mut AppContext<'_>) {
        if pos.0 < self.tabs.len() {
            if pos.1 < self.tabs[pos.0].len() {
                self.sel_split = Some(pos.0);
                self.tabbed[pos.0].select(Some(pos.1));

                ctx.focus_mut().update_container(self);
                ctx.focus().focus(&self.tabs[pos.0][pos.1]);
            }
        }
    }

    // Select next split
    pub fn select_next(&mut self, ctx: &mut AppContext<'_>) -> bool {
        if let Some(idx_split) = self.sel_split {
            if idx_split + 1 < self.tabs.len() {
                let new_split = idx_split + 1;
                let new_tab = self.tabbed[new_split].selected().unwrap_or_default();
                self.select((new_split, new_tab), ctx);
                return true;
            }
        }
        false
    }

    // Select prev split
    pub fn select_prev(&mut self, ctx: &mut AppContext<'_>) -> bool {
        if let Some(idx_split) = self.sel_split {
            if idx_split > 0 {
                let new_split = idx_split - 1;
                let new_tab = self.tabbed[new_split].selected().unwrap_or_default();
                self.select((new_split, new_tab), ctx);
                return true;
            }
        }
        false
    }

    // Position of the current focus.
    pub fn selected_pos(&self) -> Option<(usize, usize)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.tabbed[idx_split].selected() {
                return Some((idx_split, idx_tab));
            }
        }
        None
    }

    // Last known focus and position.
    pub fn selected(&self) -> Option<((usize, usize), &MDFileState)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.tabbed[idx_split].selected() {
                return Some(((idx_split, idx_tab), &self.tabs[idx_split][idx_tab]));
            }
        }
        None
    }

    // Last known focus and position.
    pub fn selected_mut(&mut self) -> Option<((usize, usize), &mut MDFileState)> {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.tabbed[idx_split].selected() {
                return Some(((idx_split, idx_tab), &mut self.tabs[idx_split][idx_tab]));
            }
        }
        None
    }

    // Find the editor for the path.
    pub fn for_path(&self, path: &Path) -> Option<((usize, usize), &MDFileState)> {
        for (idx_split, tabs) in self.tabs.iter().enumerate() {
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
        for (idx_split, tabs) in self.tabs.iter_mut().enumerate() {
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
        for (_idx_split, tabs) in self.tabs.iter_mut().enumerate() {
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
        ctx: &mut AppContext<'_>,
    ) {
        for (idx_split, tabs) in self.tabs.iter_mut().enumerate() {
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
