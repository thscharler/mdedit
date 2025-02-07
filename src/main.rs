use crate::config::MDConfig;
use crate::config_dlg::{ConfigDialog, ConfigDialogState};
use crate::editor::{MDEdit, MDEditState};
use crate::event::MDEvent;
use crate::facilities::{Facility, MDFileDialog, MDFileDialogState};
use crate::fs_structure::FileSysStructure;
use crate::global::GlobalState;
use crate::theme::{dark_themes, DarkTheme};
use anyhow::Error;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::SendError;
use dirs::cache_dir;
use log::{error, set_max_level};
use rat_salsa::poll::{PollCrossterm, PollRendered, PollTasks, PollTimers};
use rat_salsa::thread_pool::Cancel;
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{run_tui, AppState, AppWidget, Control, RenderContext, RunConfig};
use rat_theme2::schemes::IMPERIAL;
use rat_widget::event::{
    ct_event, try_flow, ConsumedEvent, Dialog, HandleEvent, MenuOutcome, Popup,
};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::hover::Hover;
use rat_widget::layout::layout_middle;
use rat_widget::menu::{MenuBuilder, MenuStructure, Menubar, MenubarState, Separator};
use rat_widget::msgdialog::{MsgDialog, MsgDialogState};
use rat_widget::popup::Placement;
use rat_widget::statusline::{StatusLine, StatusLineState};
use rat_widget::text::HasScreenCursor;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::StatefulWidget;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Padding};
use std::cmp::max;
use std::env::args;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::str::from_utf8;
use std::time::Duration;
use std::{env, fs, mem};

type AppContext<'a> = rat_salsa::AppContext<'a, GlobalState, MDEvent, Error>;

mod config;
mod config_dlg;
mod doc_type;
mod editor;
mod editor_file;
mod event;
mod facilities;
mod file_list;
mod fs_structure;
mod global;
mod split_tab;
mod theme;

fn main() -> Result<(), Error> {
    setup_logging()?;

    let mut config = MDConfig::load()?;
    set_max_level(config.log_level.parse()?);

    let mut args = args();
    args.next();
    config.load_file = {
        let mut load = Vec::new();
        for arg1 in args {
            for path in glob::glob(&arg1)? {
                let mut path = path?;
                // need __some__ parent directory
                if path.parent().is_none() || path.parent() == Some(&PathBuf::from("")) {
                    path = PathBuf::from(".").join(path);
                }
                load.push(path);
            }
        }
        load
    };

    let theme = dark_themes()
        .iter()
        .find(|v| v.name() == config.theme)
        .cloned()
        .unwrap_or(DarkTheme::new("Imperial".into(), IMPERIAL));

    let mut global = GlobalState::new(config, theme);

    let app = MDApp;
    let mut state = MDAppState::default();

    run_tui(
        app,
        &mut global,
        &mut state,
        RunConfig::default()?
            .poll(PollCrossterm)
            .poll(PollTasks::default())
            .poll(PollTimers::default())
            .poll(PollRendered),
    )?;

    Ok(())
}

#[derive(Debug)]
struct Menu {
    show_ctrl: bool,
}

impl<'a> MenuStructure<'a> for Menu {
    fn menus(&'a self, menu: &mut MenuBuilder<'a>) {
        menu.item_parsed("_File")
            .item_parsed("_Edit")
            .item_parsed("_View")
            .item_parsed("_Quit");
    }

    fn submenu(&'a self, n: usize, submenu: &mut MenuBuilder<'a>) {
        match n {
            0 => {
                submenu.item_parsed("_New..|Ctrl-N");
                submenu.item_parsed("_Open..|Ctrl-O");
                submenu.item_parsed("_Save..|Ctrl-S");
                submenu.item_parsed("Save _as..");
                submenu.item_parsed("\\___");
                submenu.item_parsed("_Configure");
            }
            1 => {
                submenu.item_parsed("Format Item|F8");
                submenu.item_parsed("Alt-Format Item|F7");
            }
            2 => {
                if self.show_ctrl {
                    submenu.item_parsed("\u{2611} Control chars");
                } else {
                    submenu.item_parsed("\u{2610} Control chars");
                }
                submenu.separator(Separator::Dotted);
                submenu.item_parsed("_Split view|Ctrl-W D");
                submenu.item_parsed("_Jump to Tree|F4");
                submenu.item_parsed("_Jump to File|F5");
                submenu.item_parsed("_Hide files|F6");
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub struct MDApp;

#[derive(Debug)]
pub struct MDAppState {
    pub editor: MDEditState,
    pub menu: MenubarState,
    pub status: StatusLineState,
    pub clear_status: TimerHandle,

    pub window_cmd: bool,

    pub message_dlg: MsgDialogState,
    pub error_dlg: MsgDialogState,
    pub file_dlg: MDFileDialogState,
    pub cfg_dlg: ConfigDialogState,
}

impl Default for MDAppState {
    fn default() -> Self {
        let s = Self {
            editor: MDEditState::default(),
            menu: MenubarState::named("menu"),
            status: Default::default(),
            clear_status: Default::default(),
            window_cmd: false,
            message_dlg: Default::default(),
            error_dlg: Default::default(),
            file_dlg: Default::default(),
            cfg_dlg: Default::default(),
        };
        s
    }
}

impl AppWidget<GlobalState, MDEvent, Error> for MDApp {
    type State = MDAppState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let theme = ctx.g.theme.clone();
        let scheme = theme.scheme();

        let r = Layout::vertical([
            Constraint::Fill(1), //
            Constraint::Length(1),
        ])
        .split(area);
        let s = Layout::horizontal([
            Constraint::Percentage(61), //
            Constraint::Percentage(39),
        ])
        .split(r[1]);

        MDEdit.render(r[0], buf, &mut state.editor, ctx)?;

        let menu_struct = Menu {
            show_ctrl: ctx.g.cfg.show_ctrl,
        };
        let (menu, menu_popup) = Menubar::new(&menu_struct)
            .title("^^°n°^^")
            .popup_width(25)
            .popup_block(Block::bordered())
            .popup_placement(Placement::Above)
            .styles(if state.menu.is_focused() {
                theme.menu_style()
            } else {
                theme.menu_style_hidden()
            })
            .into_widgets();
        menu.render(s[0], buf, &mut state.menu);

        let status = StatusLine::new()
            .layout([Constraint::Fill(1), Constraint::Length(14)])
            .styles(vec![theme.status_base(), theme.status_base()]);
        status.render(s[1], buf, &mut state.status);

        // some overlays
        Hover::new().render(Rect::default(), buf, &mut ctx.g.hover);
        menu_popup.render(s[0], buf, &mut state.menu);

        // dialogs
        if state.file_dlg.active() {
            let l_fd = layout_middle(
                r[0],
                Constraint::Length(state.menu.bar.item_areas[0].x),
                Constraint::Percentage(39),
                Constraint::Percentage(39),
                Constraint::Length(0),
            );
            MDFileDialog::new()
                .style(theme.file_dialog_style()) //
                .render(l_fd, buf, &mut state.file_dlg);
            ctx.set_screen_cursor(state.file_dlg.screen_cursor());
        }

        if state.cfg_dlg.active() {
            let l_fd = layout_middle(
                r[0],
                Constraint::Percentage(19),
                Constraint::Percentage(19),
                Constraint::Percentage(19),
                Constraint::Percentage(19),
            );
            ConfigDialog.render(l_fd, buf, &mut state.cfg_dlg, ctx)?;
        }

        if state.error_dlg.active() {
            let l_msg = layout_middle(
                r[0],
                Constraint::Percentage(19),
                Constraint::Percentage(19),
                Constraint::Percentage(19),
                Constraint::Percentage(19),
            );
            let err = MsgDialog::new()
                .block(
                    Block::bordered()
                        .style(theme.dialog_base())
                        .border_type(BorderType::Rounded)
                        .title_style(Style::new().fg(ctx.g.scheme().red[0]))
                        .padding(Padding::new(1, 1, 1, 1)),
                )
                .styles(theme.msg_dialog_style());
            err.render(l_msg, buf, &mut state.error_dlg);
        }

        if state.message_dlg.active() {
            let l_msg = layout_middle(
                r[0],
                Constraint::Percentage(4),
                Constraint::Percentage(4),
                Constraint::Percentage(4),
                Constraint::Percentage(4),
            );
            let err = MsgDialog::new()
                .block(
                    Block::bordered()
                        .style(
                            Style::default() //
                                .fg(scheme.white[2])
                                .bg(scheme.deepblue[0]),
                        )
                        .border_type(BorderType::Rounded)
                        .title_style(Style::new().fg(ctx.g.scheme().bluegreen[0]))
                        .padding(Padding::new(1, 1, 1, 1)),
                )
                .styles(theme.msg_dialog_style());
            err.render(l_msg, buf, &mut state.message_dlg);
        }

        Ok(())
    }
}

impl HasFocus for MDAppState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.widget(&self.menu);
        builder.widget(&self.editor);
    }

    fn focus(&self) -> FocusFlag {
        unimplemented!("don't use this")
    }

    fn area(&self) -> Rect {
        unimplemented!("don't use this")
    }
}

impl AppState<GlobalState, MDEvent, Error> for MDAppState {
    fn init(&mut self, ctx: &mut AppContext<'_>) -> Result<(), Error> {
        ctx.focus = Some(FocusBuilder::build_for(self));
        ctx.focus().enable_log();

        self.editor.init(ctx)?;

        self.menu.bar.select(Some(0));
        self.status
            .status(0, format!("mdedit {}", env!("CARGO_PKG_VERSION")));
        self.clear_status = ctx.add_timer(TimerDef::new().timer(Duration::from_secs(1)));

        fn spawn_load_dir(
            path: PathBuf,
            ctx: &mut AppContext<'_>,
        ) -> Result<Cancel, SendError<()>> {
            let cfg = ctx.g.cfg.globs.clone();
            ctx.spawn(move |_can, _send| {
                let mut sys = FileSysStructure::new();
                sys.load_filesys(&path)?;

                if sys.is_mdbook {
                    let src_path = path.join("src");
                    sys.load_current(&src_path, &cfg)?;
                } else {
                    sys.load_current(&path, &cfg)?;
                }

                Ok(Control::Event(MDEvent::FileSys(
                    Box::new(AtomicCell::new(sys)), //
                )))
            })
        }

        if ctx.g.cfg.load_file.is_empty() {
            let cwd = env::current_dir()?;
            spawn_load_dir(cwd, ctx)?;
        } else {
            for load in mem::take(&mut ctx.g.cfg.load_file) {
                if load.is_dir() {
                    spawn_load_dir(load, ctx)?;
                } else {
                    _ = self.editor.open(&load, ctx)?;
                }
            }
            _ = self.editor.select_tab_at(0, 0, ctx)?;
            _ = self.editor.sync_file_list(ctx)?;
        }

        Ok(())
    }

    fn event(
        &mut self,
        mdevent: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r = match mdevent {
            MDEvent::Event(event) => {
                try_flow!(self.error_dlg.handle(event, Dialog));
                try_flow!(self.message_dlg.handle(event, Dialog));
                try_flow!(self.file_dlg.handle(mdevent, Dialog)?);
                try_flow!(self.cfg_dlg.event(mdevent, ctx)?);

                // ^W window commands
                if self.window_cmd {
                    try_flow!(self.window_cmd(event, ctx)?);
                }

                ctx.focus_event(event);

                // regular global
                let mut r = match &event {
                    ct_event!(resized) => Control::Changed,
                    ct_event!(key press CONTROL-'q') => Control::Quit,
                    ct_event!(key press CONTROL-'e') => Control::Event(MDEvent::Close),
                    ct_event!(keycode press CONTROL-F(4)) => Control::Event(MDEvent::Close),
                    ct_event!(key press CONTROL_SHIFT-'E') => Control::Event(MDEvent::CloseAll),
                    ct_event!(keycode press CONTROL_SHIFT-F(4)) => {
                        Control::Event(MDEvent::CloseAll)
                    }
                    ct_event!(key press CONTROL-'n') => Control::Event(MDEvent::MenuNew),
                    ct_event!(key press CONTROL-'o') => Control::Event(MDEvent::MenuOpen),
                    ct_event!(key press CONTROL-'s') => Control::Event(MDEvent::Save),
                    ct_event!(keycode press Esc) => self.flip_esc_focus(ctx)?,
                    ct_event!(keycode press F(1)) => self.show_help(ctx)?,
                    ct_event!(keycode press F(2)) => self.show_cheat(ctx)?,
                    ct_event!(keycode press F(4)) => Control::Event(MDEvent::JumpToTree),
                    ct_event!(keycode press F(5)) => Control::Event(MDEvent::JumpToFiles),
                    ct_event!(keycode press F(6)) => Control::Event(MDEvent::HideFiles),
                    ct_event!(key press CONTROL-'w') => {
                        self.window_cmd = true;
                        Control::Changed
                    }
                    ct_event!(focus_lost) => Control::Event(MDEvent::Save),
                    _ => Control::Continue,
                };

                r = r.or_else_try(|| self.handle_menu(event, ctx))?;
                r
            }
            MDEvent::Immediate(r) => {
                panic!("found immediate {:?}", r);
            }
            MDEvent::Rendered => {
                // rebuild keyboard + mouse focus
                ctx.focus = Some(FocusBuilder::rebuild_for(self, ctx.focus.take()));
                ctx.focus().enable_log();
                Control::Continue
            }
            MDEvent::Status(n, s) => {
                self.status.status(*n, s);
                Control::Changed
            }
            MDEvent::Message(s) => {
                self.error_dlg.append(s);
                Control::Changed
            }
            MDEvent::MenuNew => self.file_dlg.engage(
                |w| {
                    w.save_dialog_ext(".", "", "md")?;
                    Ok(Control::Changed)
                },
                |p| Ok(Control::Event(MDEvent::New(p))),
            )?,
            MDEvent::MenuOpen => self.file_dlg.engage(
                |w| {
                    w.open_dialog(".")?;
                    Ok(Control::Changed)
                },
                |p| Ok(Control::Event(MDEvent::Open(p))),
            )?,
            MDEvent::MenuSave => Control::Event(MDEvent::Save),
            MDEvent::MenuSaveAs => self.file_dlg.engage(
                |w| {
                    w.save_dialog(".", "")?;
                    Ok(Control::Changed)
                },
                |p| Ok(Control::Event(MDEvent::SaveAs(p))),
            )?,
            MDEvent::StoreConfig => {
                error!("{:?}", ctx.g.cfg.store());
                Control::Continue
            }
            MDEvent::TimeOut(t) => {
                if t.handle == self.clear_status {
                    self.status.status(0, "");
                    Control::Changed
                } else {
                    Control::Continue
                }
            }
            _ => Control::Continue,
        };

        r = r.or_else_try(|| self.editor.event(mdevent, ctx))?;

        Ok(r)
    }

    fn error(&self, event: Error, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.error_dlg.title("Error occured");
        self.error_dlg.append(format!("{:?}", &*event).as_str());
        Ok(Control::Changed)
    }
}

impl MDAppState {
    fn window_cmd(
        &mut self,
        event: &crossterm::event::Event,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        self.window_cmd = false;
        let wr = match event {
            ct_event!(key release CONTROL-'w') => {
                self.window_cmd = true;
                Control::Changed
            }
            ct_event!(keycode press Left) => Control::Event(MDEvent::PrevEditSplit),
            ct_event!(keycode press Right) => Control::Event(MDEvent::NextEditSplit),
            ct_event!(keycode press Tab) => {
                ctx.focus().next_force();
                ctx.queue(Control::Changed);
                Control::Continue
            }
            ct_event!(keycode press SHIFT-BackTab) => {
                ctx.focus().prev_force();
                ctx.queue(Control::Changed);
                Control::Continue
            }
            ct_event!(key press CONTROL-'c')
            | ct_event!(key press 'c')
            | ct_event!(key press 'x')
            | ct_event!(key press CONTROL-'x') => Control::Event(MDEvent::Close),
            ct_event!(key press CONTROL-'d')
            | ct_event!(key press 'd')
            | ct_event!(key press '+') => Control::Event(MDEvent::Split),
            ct_event!(key press CONTROL-'t') | ct_event!(key press 't') => {
                Control::Event(MDEvent::JumpToTabs)
            }
            ct_event!(key press CONTROL-'s') | ct_event!(key press 's') => {
                Control::Event(MDEvent::JumpToEditSplit)
            }
            ct_event!(key press CONTROL-'f') | ct_event!(key press 'f') => {
                Control::Event(MDEvent::JumpToFileSplit)
            }
            _ => Control::Changed,
        };

        if self.window_cmd {
            ctx.queue(Control::Event(MDEvent::Status(1, "^W".into())));
        } else {
            ctx.queue(Control::Event(MDEvent::Status(1, "".into())));
        }

        // don't let anything through to the application.
        Ok(max(wr, Control::Unchanged))
    }

    fn handle_menu(
        &mut self,
        event: &crossterm::event::Event,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let r = match self.menu.handle(event, Popup) {
            MenuOutcome::MenuActivated(0, 0) => Control::Event(MDEvent::MenuNew),
            MenuOutcome::MenuActivated(0, 1) => Control::Event(MDEvent::MenuOpen),
            MenuOutcome::MenuActivated(0, 2) => Control::Event(MDEvent::MenuSave),
            MenuOutcome::MenuActivated(0, 3) => Control::Event(MDEvent::MenuSaveAs),
            MenuOutcome::MenuActivated(0, 4) => {
                self.cfg_dlg.show(ctx)?;
                Control::Changed
            }
            MenuOutcome::MenuActivated(1, 0) => {
                if let Some((_, sel)) = self.editor.split_tab.selected_mut() {
                    ctx.focus().focus(sel);
                    Control::Event(MDEvent::MenuFormat)
                } else {
                    Control::Continue
                }
            }
            MenuOutcome::MenuActivated(1, 1) => {
                if let Some((_, sel)) = self.editor.split_tab.selected_mut() {
                    ctx.focus().focus(sel);
                    Control::Event(MDEvent::MenuFormat)
                } else {
                    Control::Continue
                }
            }
            MenuOutcome::MenuActivated(2, 0) => {
                ctx.g.cfg.show_ctrl = !ctx.g.cfg.show_ctrl;
                Control::Event(MDEvent::CfgShowCtrl)
            }
            MenuOutcome::MenuActivated(2, 1) => Control::Event(MDEvent::Split),
            MenuOutcome::MenuActivated(2, 2) => Control::Event(MDEvent::JumpToTree),
            MenuOutcome::MenuActivated(2, 3) => Control::Event(MDEvent::JumpToFiles),
            MenuOutcome::MenuActivated(2, 4) => Control::Event(MDEvent::HideFiles),
            MenuOutcome::Activated(3) => Control::Quit,
            r => r.into(),
        };

        Ok(r)
    }

    fn flip_esc_focus(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        if !self.menu.is_focused() {
            ctx.focus().focus(&self.menu);
            ctx.queue(Control::Changed);
            Ok(Control::Continue)
        } else {
            if let Some((_, last_edit)) = self.editor.split_tab.selected() {
                ctx.focus().focus(last_edit);
                ctx.queue(Control::Changed);
                Ok(Control::Continue)
            } else {
                ctx.focus().focus(&self.editor.file_list.file_list);
                ctx.queue(Control::Changed);
                Ok(Control::Continue)
            }
        }
    }

    fn show_help(&mut self, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let txt = from_utf8(HELP)?;
        let mut txt2 = String::new();
        for l in txt.lines() {
            txt2.push_str(l);
            txt2.push('\n');
        }
        self.message_dlg.append(&txt2);
        Ok(Control::Changed)
    }

    fn show_cheat(&mut self, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let txt = from_utf8(CHEAT)?;
        let mut txt2 = String::new();
        for l in txt.lines() {
            txt2.push_str(l);
            txt2.push('\n');
        }
        self.message_dlg.append(&txt2);
        Ok(Control::Changed)
    }
}

fn setup_logging() -> Result<(), Error> {
    if let Some(cache) = cache_dir() {
        let log_file = if cfg!(debug_assertions) {
            PathBuf::from("log.log")
        } else {
            let log_path = cache.join("mdedit");
            if !log_path.exists() {
                create_dir_all(&log_path)?;
            }
            log_path.join("log.log")
        };

        _ = fs::remove_file(&log_file);
        fern::Dispatch::new()
            .format(|out, message, _record| {
                out.finish(format_args!("{}", message)) //
            })
            .level(log::LevelFilter::Debug)
            .chain(fern::log_file(&log_file)?)
            .apply()?;
    }
    Ok(())
}

static HELP: &[u8] = include_bytes!("mdedit.md");
static CHEAT: &[u8] = include_bytes!("cheat.md");
