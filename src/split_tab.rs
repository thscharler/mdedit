use crate::editor_file::{MDFile, MDFileState};
use crate::event::{MDEvent, MDImmediate};
use crate::global::GlobalState;
use crate::AppContext;
use anyhow::Error;
use rat_salsa::timer::TimerDef;
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_theme2::Contrast;
use rat_widget::event::{try_flow, ConsumedEvent, HandleEvent, Regular, TabbedOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::splitter::{Split, SplitState, SplitType};
use rat_widget::tabbed::{TabType, Tabbed, TabbedState};
use rat_widget::text::undo_buffer::UndoEntry;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct SplitTab;

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
            container: FocusFlag::named("split_tab"),
            sel_split: Default::default(),
            sel_tab: Default::default(),
            split: SplitState::named("splitter"),
            split_tab: Default::default(),
            split_tab_file: Default::default(),
        }
    }
}

#[allow(deprecated)]
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
        let scheme = theme.scheme();

        let (split, split_areas) = Split::horizontal()
            .constraints(vec![Constraint::Fill(1); state.split_tab.len()])
            .mark_offset(0)
            .split_type(SplitType::Scroll)
            .styles(theme.split_style())
            .into_widget_layout(area, &mut state.split);

        if split_areas.is_empty() {
            buf.set_style(area, theme.textarea_style_doc().style);
        }

        let max_idx_split = split_areas.len().saturating_sub(1);
        for (idx_split, edit_area) in split_areas.iter().enumerate() {
            let select_style = if let Some((sel_pos, md)) = state.selected() {
                if sel_pos.0 == idx_split {
                    if state.split_tab[idx_split].is_focused() {
                        theme.tabbed_style().focus.expect("style")
                    } else if md.is_focused() {
                        scheme.primary(1, Contrast::Normal)
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
                MDFile::new()
                    .start_margin(if max_idx_split == idx_split { 0 } else { 1 })
                    .render(
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

impl AppState<GlobalState, MDEvent, Error> for SplitTabState {
    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        // establish focus
        if ctx.focus().gained_focus().is_some() {
            let idx = 'sel: {
                for (idx_split, split) in self.split_tab_file.iter().enumerate() {
                    for (idx_tab, tab) in split.iter().enumerate() {
                        if tab.gained_focus() {
                            break 'sel Some((idx_split, idx_tab));
                        }
                    }
                }
                None
            };
            if let Some((idx_split, idx_tab)) = idx {
                self.select((idx_split, idx_tab), ctx);
            }
        }

        let mut r = match event {
            MDEvent::Event(event) => {
                try_flow!(self.split.handle(event, Regular));

                let (idx_split, r) = 'tab: {
                    for (idx_split, tabbed) in self.split_tab.iter_mut().enumerate() {
                        let r = tabbed.handle(event, Regular);
                        if r.is_consumed() {
                            break 'tab (idx_split, r);
                        }
                    }
                    (0, TabbedOutcome::Continue)
                };
                match r {
                    TabbedOutcome::Close(n) => {
                        self.close((idx_split, n), ctx)?;
                        self.focus_selected(ctx);
                        Control::Event(MDEvent::Immediate(MDImmediate::TabClosed))
                    }
                    TabbedOutcome::Select(n) => {
                        self.select((idx_split, n), ctx);
                        self.focus_selected(ctx);
                        Control::Changed
                    }
                    r => r.into(),
                }
            }
            _ => Control::Continue,
        };

        r = r.or_else_try(|| {
            match event {
                MDEvent::Event(_) => {
                    // forward only to the selected tab.
                    for (idx_split, tabbed) in self.split_tab.iter_mut().enumerate() {
                        if let Some(idx_tab) = tabbed.selected() {
                            try_flow!(self.split_tab_file[idx_split][idx_tab].event(event, ctx)?);
                        }
                    }
                }
                _ => {
                    // application events go everywhere
                    for tab in &mut self.split_tab_file {
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

        assert!(self.sel_split == new_split && self.sel_tab == new_tab);
    }

    // Add file at position (split-idx, tab-idx).
    pub fn open(&mut self, pos: (usize, usize), new: MDFileState, _ctx: &mut AppContext<'_>) {
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
        self.split_tab_file[pos.0].insert(pos.1, new);
    }

    // Close tab (split-idx, tab-idx).
    pub fn close(&mut self, pos: (usize, usize), _ctx: &mut AppContext<'_>) -> Result<(), Error> {
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
                        if new_split.is_some() {
                            self.sel_tab = Some(0);
                            self.split_tab[pos.0].select(Some(0));
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
    pub fn select(&mut self, pos: (usize, usize), _ctx: &mut AppContext<'_>) {
        if pos.0 < self.split_tab_file.len() {
            if pos.1 < self.split_tab_file[pos.0].len() {
                self.sel_split = Some(pos.0);
                self.sel_tab = Some(pos.1);
                self.split_tab[pos.0].select(Some(pos.1));
            }
        }
    }

    // Rebuild focus and focus selected
    pub fn focus_selected(&mut self, ctx: &mut AppContext<'_>) {
        if let Some(idx_split) = self.sel_split {
            if let Some(idx_tab) = self.sel_tab {
                ctx.focus_mut().update_container(self);
                ctx.focus().focus(&self.split_tab_file[idx_split][idx_tab]);
            }
        }
    }

    // Select next split
    pub fn select_next(&mut self, ctx: &mut AppContext<'_>) -> bool {
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
    pub fn select_prev(&mut self, ctx: &mut AppContext<'_>) -> bool {
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
        ctx: &mut AppContext<'_>,
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
