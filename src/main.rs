use crate::config::MDConfig;
use crate::config_dlg::{ConfigDialog, ConfigDialogState};
use crate::editor::{MDEdit, MDEditState};
use crate::event::MDEvent;
use crate::fs_structure::FileSysStructure;
use crate::global::GlobalState;
use crate::theme::{dark_themes, DarkTheme};
use anyhow::Error;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::SendError;
use dirs::cache_dir;
use log::error;
use rat_dialog::widgets::{FileDialog, FileDialogState, MsgDialog, MsgDialogState};
use rat_dialog::{DialogStack, DialogWidget};
use rat_salsa::poll::{PollCrossterm, PollRendered, PollTasks, PollTimers};
use rat_salsa::thread_pool::Cancel;
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{run_tui, AppState, AppWidget, Control, RenderContext, RunConfig};
use rat_theme2::palettes::IMPERIAL;
use rat_widget::event::{
    ct_event, try_flow, ConsumedEvent, FileOutcome, HandleEvent, MenuOutcome, Popup,
};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::hover::Hover;
use rat_widget::menu::{MenuBuilder, MenuStructure, Menubar, MenubarState, Separator};
use rat_widget::popup::Placement;
use rat_widget::statusline::{StatusLine, StatusLineState};
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
mod file_list;
mod fs_structure;
mod global;
mod split_tab;
mod theme;

fn main() -> Result<(), Error> {
    setup_logging()?;

    let mut config = MDConfig::load()?;

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
}

impl Default for MDAppState {
    fn default() -> Self {
        let s = Self {
            editor: MDEditState::default(),
            menu: MenubarState::named("menu"),
            status: Default::default(),
            clear_status: Default::default(),
            window_cmd: false,
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
        // menu popups
        menu_popup.render(s[0], buf, &mut state.menu);
        // dialogs
        DialogStack.render(r[0], buf, &mut ctx.g.dialogs.clone(), ctx)?;

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
        // ctx.focus().enable_log();

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

                if sys.is_mdbook() {
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
            _ = self.editor.sync_file_list(true, ctx)?;
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
                try_flow!(ctx.g.dialogs.clone().event(mdevent, ctx)?);

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
                    ct_event!(focus_gained) => {
                        let cfg = ctx.g.cfg.globs.clone();
                        let root = self.editor.file_list.root().to_path_buf();
                        let current = self.editor.file_list.current_dir().to_path_buf();
                        _ = ctx.spawn(move |_can, _send| {
                            let mut sys = FileSysStructure::new();
                            sys.load_filesys(&root)?;
                            sys.load_current(&current, &cfg)?;
                            Ok(Control::Event(MDEvent::FileSys(
                                Box::new(AtomicCell::new(sys)), //
                            )))
                        });
                        Control::Continue
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
                // ctx.focus().enable_log();
                Control::Continue
            }
            MDEvent::Status(n, s) => {
                self.status.status(*n, s);
                Control::Changed
            }
            MDEvent::Message(s) => {
                if !ctx.g.dialogs.is_empty() && ctx.g.dialogs.top_state_is::<MsgDialogState>()? {
                    ctx.g
                        .dialogs
                        .map_top_state_if::<MsgDialogState, _, _>(|v| {
                            v.append(s.as_str());
                        })?;
                } else {
                    ctx.g.dialogs.push_dialog(
                        |area, buf, state, ctx| {
                            MsgDialog::new()
                                .block(
                                    Block::bordered()
                                        .style(ctx.g.theme.dialog_base())
                                        .border_type(BorderType::Rounded)
                                        .title_style(Style::new().fg(ctx.g.scheme().red[0]))
                                        .padding(Padding::new(1, 1, 1, 1)),
                                )
                                .styles(ctx.g.theme.msg_dialog_style())
                                .render(area, buf, state, ctx)
                        },
                        MsgDialogState::new(s),
                    );
                }
                Control::Changed
            }
            MDEvent::MenuNew => {
                let mut state = FileDialogState::new();
                state.save_dialog_ext(self.editor.file_list.current_dir(), "", "md")?;
                state.map_outcome(|r| match r {
                    FileOutcome::Ok(p) => Control::Event(MDEvent::New(p)),
                    r => r.into(),
                });

                ctx.g.dialogs.push_dialog(
                    |area, buf, state, ctx| {
                        FileDialog::new()
                            .styles(ctx.g.theme.file_dialog_style())
                            .render(area, buf, state, ctx)
                    },
                    state,
                );
                Control::Changed
            }
            MDEvent::MenuOpen => {
                let mut state = FileDialogState::new();
                state.open_dialog(".")?;
                state.map_outcome(|r| match r {
                    FileOutcome::Ok(p) => Control::Event(MDEvent::Open(p)),
                    r => r.into(),
                });

                ctx.g.dialogs.push_dialog(
                    |area, buf, state, ctx| {
                        FileDialog::new()
                            .styles(ctx.g.theme.file_dialog_style())
                            .render(area, buf, state, ctx)
                    },
                    state,
                );
                Control::Changed
            }
            MDEvent::MenuSave => Control::Event(MDEvent::Save),
            MDEvent::MenuSaveAs => {
                let mut state = FileDialogState::new();
                state.save_dialog(".", "")?;
                state.map_outcome(|r| match r {
                    FileOutcome::Ok(p) => Control::Event(MDEvent::SaveAs(p)),
                    r => r.into(),
                });

                ctx.g.dialogs.push_dialog(
                    |area, buf, state, ctx| {
                        FileDialog::new()
                            .styles(ctx.g.theme.file_dialog_style())
                            .render(area, buf, state, ctx)
                    },
                    state,
                );
                Control::Changed
            }
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

    fn error(&self, event: Error, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        error!("{:#?}", event);

        if !ctx.g.dialogs.is_empty() && ctx.g.dialogs.top_state_is::<MsgDialogState>()? {
            ctx.g
                .dialogs
                .map_top_state_if::<MsgDialogState, _, _>(|v| {
                    v.append(format!("{:?}", &*event));
                })?;
        } else {
            ctx.g.dialogs.push_dialog(
                |area, buf, state, ctx| {
                    MsgDialog::new()
                        .block(
                            Block::bordered()
                                .style(ctx.g.theme.dialog_base())
                                .border_type(BorderType::Rounded)
                                .title_style(Style::new().fg(ctx.g.scheme().red[0]))
                                .padding(Padding::new(1, 1, 1, 1)),
                        )
                        .styles(ctx.g.theme.msg_dialog_style())
                        .render(area, buf, state, ctx)
                },
                MsgDialogState::new(format!("{:?}", &*event)).title("Error occured"),
            );
        }
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
                let mut dlg = ConfigDialogState::new(ctx)?;
                dlg.show(ctx)?;

                ctx.g.dialogs.push_dialog(
                    |area, buf, state, ctx| {
                        ConfigDialog //
                            .render(area, buf, state, ctx)
                    },
                    dlg,
                );
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
                self.editor.file_list.focus_files(ctx);
                ctx.queue(Control::Changed);
                Ok(Control::Continue)
            }
        }
    }

    fn show_help(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let txt = from_utf8(HELP)?;
        let mut txt2 = String::new();
        for l in txt.lines() {
            txt2.push_str(l);
            txt2.push('\n');
        }

        ctx.g.dialogs.push_dialog(
            |area, buf, state, ctx| {
                MsgDialog::new()
                .block(
                    Block::bordered()
                        .style(
                            Style::default() //
                                .fg(ctx.g.theme.scheme().white[2])
                                .bg(ctx.g.theme.scheme().deepblue[0]),
                        )
                        .border_type(BorderType::Rounded)
                        .title_style(Style::new().fg(ctx.g.scheme().bluegreen[0]))
                        .padding(Padding::new(1, 1, 1, 1)),
                )
                .styles(ctx.g.theme.msg_dialog_style())
                    .render(area,buf,state,ctx)
            },
            MsgDialogState::new(txt2),
        );
        Ok(Control::Changed)
    }

    fn show_cheat(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        let txt = from_utf8(CHEAT)?;
        let mut txt2 = String::new();
        for l in txt.lines() {
            txt2.push_str(l);
            txt2.push('\n');
        }

        ctx.g.dialogs.push_dialog(
            |area, buf, state, ctx| {
                MsgDialog::new()
                    .block(
                        Block::bordered()
                            .style(
                                Style::default() //
                                    .fg(ctx.g.theme.scheme().white[2])
                                    .bg(ctx.g.theme.scheme().deepblue[0]),
                            )
                            .border_type(BorderType::Rounded)
                            .title_style(Style::new().fg(ctx.g.scheme().bluegreen[0]))
                            .padding(Padding::new(1, 1, 1, 1)),
                    )
                    .styles(ctx.g.theme.msg_dialog_style())
                    .render(area,buf,state,ctx)
            },
            MsgDialogState::new(txt2),
        );
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
            .chain(fern::log_file(&log_file)?)
            .apply()?;
    }
    Ok(())
}

static HELP: &[u8] = include_bytes!("mdedit.md");
static CHEAT: &[u8] = include_bytes!("cheat.md");
