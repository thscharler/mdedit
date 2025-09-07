use crate::cfg::MDConfig;
use crate::config_dlg::ConfigDialogState;
use crate::dlg::config_dlg;
use crate::editor::MDEditState;
use crate::fsys::FileSysStructure;
use crate::global::event::MDEvent;
use crate::global::theme::{dark_themes, DarkTheme};
use crate::global::GlobalState;
use anyhow::Error;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::SendError;
use dirs::cache_dir;
use dlg::{file_dlg, msg_dialog};
use log::error;
use rat_salsa::poll::{PollCrossterm, PollQuit, PollRendered, PollTasks, PollTimers};
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{run_tui, Control, RunConfig, SalsaContext};
use rat_theme2::palettes::IMPERIAL;
use rat_widget::event::{ct_event, try_flow, HandleEvent, MenuOutcome, Popup};
use rat_widget::file_dialog::FileDialogState;
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::hover::Hover;
use rat_widget::menu::{MenuBuilder, MenuStructure, Menubar, MenubarState, Separator};
use rat_widget::msgdialog::MsgDialogState;
use rat_widget::popup::Placement;
use rat_widget::statusline::{StatusLine, StatusLineState};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::StatefulWidget;
use ratatui::widgets::Block;
use std::cmp::max;
use std::env::args;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::str::from_utf8;
use std::time::Duration;
use std::{env, fs, mem};

mod cfg;
mod dlg;
mod doc_type;
mod editor;
mod editor_file;
mod file_list;
mod fsys;
mod global;
mod split_tab;

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

    let mut state = Scenery::default();

    run_tui(
        init,
        render,
        event,
        error,
        &mut global,
        &mut state,
        RunConfig::default()?
            .poll(PollCrossterm)
            .poll(PollTasks::default())
            .poll(PollTimers::default())
            .poll(PollRendered)
            .poll(PollQuit),
    )?;

    Ok(())
}

#[derive(Debug)]
struct Menu {
    show_ctrl: bool,
    show_break: bool,
    wrap_text: bool,
    show_linenr: bool,
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
                    submenu.item_parsed("\u{2611} Control chars|Alt-C");
                } else {
                    submenu.item_parsed("\u{2610} Control chars|Alt-C");
                }
                if self.show_break {
                    submenu.item_parsed("\u{2611} Show breaks|Alt-B");
                } else {
                    submenu.item_parsed("\u{2610} Show breaks|Alt-B");
                }
                if self.wrap_text {
                    submenu.item_parsed("\u{2611} Word wrap|Alt-W");
                } else {
                    submenu.item_parsed("\u{2610} Word wrap|Alt-W");
                }
                if self.show_linenr {
                    submenu.item_parsed("\u{2611} Line numbers|Alt-L");
                } else {
                    submenu.item_parsed("\u{2610} Line numbers|Alt-L");
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
pub struct Scenery {
    pub editor: MDEditState,
    pub menu: MenubarState,
    pub status: StatusLineState,
    pub clear_status: TimerHandle,

    pub window_cmd: bool,
}

impl Default for Scenery {
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

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut Scenery,
    ctx: &mut GlobalState,
) -> Result<(), Error> {
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

    editor::render(r[0], buf, &mut state.editor, ctx)?;

    let menu_struct = Menu {
        show_ctrl: ctx.cfg.show_ctrl,
        show_break: ctx.cfg.show_break,
        wrap_text: ctx.cfg.wrap_text,
        show_linenr: ctx.cfg.show_linenr,
    };
    let (menu, menu_popup) = Menubar::new(&menu_struct)
        .title("^^°n°^^")
        .popup_width(25)
        .popup_block(Block::bordered())
        .popup_placement(Placement::Above)
        .styles(if state.menu.is_focused() {
            ctx.theme.menu_style()
        } else {
            ctx.theme.menu_style_hidden()
        })
        .into_widgets();
    menu.render(s[0], buf, &mut state.menu);

    let status = StatusLine::new()
        .layout([Constraint::Fill(1), Constraint::Length(14)])
        .styles(vec![
            if state.menu.is_focused() {
                ctx.theme.status_base()
            } else {
                ctx.theme.status_base_hidden()
            }, //
            if state.menu.is_focused() {
                ctx.theme.status_base()
            } else {
                ctx.theme.status_base_hidden()
            },
        ]);
    status.render(s[1], buf, &mut state.status);

    // some overlays
    Hover::new().render(Rect::default(), buf, &mut ctx.hover);
    // menu popups
    menu_popup.render(s[0], buf, &mut state.menu);
    // dialogs
    ctx.dialogs.clone().render(r[0], buf, ctx)?;

    Ok(())
}

impl HasFocus for Scenery {
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

pub fn init(state: &mut Scenery, ctx: &mut GlobalState) -> Result<(), Error> {
    ctx.set_focus(FocusBuilder::build_for(state));
    // ctx.focus().enable_log();

    editor::init(&mut state.editor, ctx)?;

    state.menu.bar.select(Some(0));
    state
        .status
        .status(0, format!("mdedit {}", env!("CARGO_PKG_VERSION")));
    state.clear_status = ctx.add_timer(TimerDef::new().timer(Duration::from_secs(1)));

    fn spawn_load_dir(path: PathBuf, ctx: &mut GlobalState) -> Result<(), SendError<()>> {
        let cfg = ctx.cfg.globs.clone();
        ctx.spawn(move || {
            let mut sys = FileSysStructure::new();
            sys.load_filesys(&path)?;

            if sys.is_mdbook() {
                let src_path = path.join("src");
                sys.load_current(&src_path, &cfg)?;
            } else {
                sys.load_current(&path, &cfg)?;
            }

            Ok(Control::Event(MDEvent::FileSysChanged(
                Box::new(AtomicCell::new(sys)), //
            )))
        })?;
        Ok(())
    }

    if !ctx.cfg.load_file.is_empty() {
        for load in mem::take(&mut ctx.cfg.load_file) {
            if load.is_dir() {
                spawn_load_dir(load, ctx)?;
            } else {
                _ = state.editor.open(&load, ctx)?;
            }
        }
        _ = state.editor.select_tab_at(0, 0, ctx)?;
        _ = state.editor.sync_file_list(true, ctx)?;
    } else if !ctx.cfg.tab_state.is_empty() {
        for (s, t, load) in ctx.cfg.tab_state.clone() {
            _ = state.editor.open_in((s, t), &load, ctx)?;
        }
        for (s, t, x, y) in ctx.cfg.tab_cursor.clone() {
            if let Some(edit) = state.editor.editor_at(s, t) {
                edit.edit.set_cursor((x, y), false);
            }
        }
        for (s, t, x, y, z) in ctx.cfg.tab_offset.clone() {
            if let Some(edit) = state.editor.editor_at(s, t) {
                edit.edit.set_offset((x as usize, y as usize));
                edit.edit.set_sub_row_offset(z);
            }
        }
        _ = state
            .editor
            .select_tab_at(ctx.cfg.tab_selected.0, ctx.cfg.tab_selected.1, ctx)?;
        state
            .editor
            .split_tab
            .split
            .set_area_lengths(ctx.cfg.edit_split_at.clone());
        _ = state.editor.sync_file_list(true, ctx)?;
    } else {
        let cwd = env::current_dir()?;
        spawn_load_dir(cwd, ctx)?;
    }

    Ok(())
}

pub fn event(
    mdevent: &MDEvent,
    state: &mut Scenery,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    match mdevent {
        MDEvent::Event(event) => {
            try_flow!(ctx.dialogs.clone().handle(mdevent, ctx)?);

            // ^W window commands
            if state.window_cmd {
                try_flow!(window_cmd(state, event, ctx)?);
            }

            ctx.handle_focus(event);

            // regular global
            try_flow!(match &event {
                ct_event!(resized) => Control::Changed,
                ct_event!(key press CONTROL-'q') => Control::Quit,
                ct_event!(key press CONTROL-'e') => Control::Event(MDEvent::Close),
                ct_event!(keycode press CONTROL-F(4)) => Control::Event(MDEvent::Close),
                ct_event!(key press CONTROL_SHIFT-'E') => Control::Event(MDEvent::CloseAll),
                ct_event!(keycode press CONTROL_SHIFT-F(4)) => Control::Event(MDEvent::CloseAll),
                ct_event!(key press CONTROL-'n') => Control::Event(MDEvent::MenuNew),
                ct_event!(key press CONTROL-'o') => Control::Event(MDEvent::MenuOpen),
                ct_event!(key press CONTROL-'s') => Control::Event(MDEvent::MenuSave),
                ct_event!(keycode press Esc) => flip_esc_focus(state, ctx)?,
                ct_event!(keycode press F(1)) => show_help(ctx)?,
                ct_event!(keycode press F(2)) => show_cheat(ctx)?,
                ct_event!(keycode press F(4)) => Control::Event(MDEvent::JumpToFiles),
                ct_event!(keycode press F(5)) => Control::Event(MDEvent::JumpToTree),
                ct_event!(keycode press F(6)) => Control::Event(MDEvent::HideFiles),
                ct_event!(key press ALT-'v') => {
                    ctx.cfg.show_ctrl = !ctx.cfg.show_ctrl;
                    ctx.queue_event(MDEvent::StoreConfig);
                    ctx.queue_event(MDEvent::CfgShowCtrl);
                    Control::Changed
                }
                ct_event!(key press ALT-'b') => {
                    ctx.cfg.show_break = !ctx.cfg.show_break;
                    ctx.queue_event(MDEvent::StoreConfig);
                    ctx.queue_event(MDEvent::CfgShowBreak);
                    Control::Changed
                }
                ct_event!(key press ALT-'w') => {
                    ctx.cfg.wrap_text = !ctx.cfg.wrap_text;
                    ctx.queue_event(MDEvent::StoreConfig);
                    ctx.queue_event(MDEvent::CfgWrapText);
                    Control::Changed
                }
                ct_event!(key press ALT-'n') => {
                    ctx.cfg.show_linenr = !ctx.cfg.show_linenr;
                    ctx.queue_event(MDEvent::StoreConfig);
                    ctx.queue_event(MDEvent::CfgShowLinenr);
                    Control::Changed
                }
                ct_event!(key press CONTROL-'w') => {
                    state.window_cmd = true;
                    Control::Changed
                }
                ct_event!(focus_gained) => {
                    let cfg = ctx.cfg.globs.clone();
                    let root = state.editor.file_list.root().to_path_buf();
                    let current = state.editor.file_list.current_dir().to_path_buf();
                    ctx.spawn(move || {
                        let mut sys = FileSysStructure::new();
                        sys.load_filesys(&root)?;
                        sys.load_current(&current, &cfg)?;
                        Ok(Control::Event(MDEvent::FileSysReloaded(
                            Box::new(AtomicCell::new(sys)), //
                        )))
                    })?;
                    Control::Continue
                }
                ct_event!(focus_lost) => Control::Event(MDEvent::Save),
                _ => Control::Continue,
            });

            try_flow!(handle_menu(state, event, ctx)?);
        }
        MDEvent::Immediate(r) => {
            panic!("found immediate {:?}", r);
        }
        MDEvent::Rendered => {
            try_flow!({
                // rebuild keyboard + mouse focus
                ctx.set_focus(FocusBuilder::rebuild_for(state, ctx.take_focus()));
                // ctx.focus().enable_log();
                Control::Continue
            });
        }
        MDEvent::Quit => {
            try_flow!({
                _ = state.editor.save(ctx)?;
                _ = store_config(state, ctx);
                Control::Quit
            });
        }
        MDEvent::Status(n, s) => {
            try_flow!({
                state.status.status(*n, s);
                Control::Changed
            });
        }
        MDEvent::Message(s) => {
            try_flow!(show_message(s, ctx));
        }
        MDEvent::MenuNew => {
            try_flow!({
                let mut state = FileDialogState::new();
                state.save_dialog_ext(PathBuf::from("."), "", "md")?;
                ctx.dialogs
                    .push(file_dlg::render, file_dlg::event_new, state);
                Control::Changed
            });
        }
        MDEvent::MenuOpen => {
            try_flow!({
                let mut state = FileDialogState::new();
                state.open_dialog(PathBuf::from("."))?;
                ctx.dialogs
                    .push(file_dlg::render, file_dlg::event_open, state);
                Control::Changed
            });
        }
        MDEvent::MenuSave => {
            try_flow!(Control::Event(MDEvent::Save));
        }
        MDEvent::MenuSaveAs => {
            try_flow!({
                let mut state = FileDialogState::new();
                state.save_dialog_ext(PathBuf::from("."), "", "pas")?;
                ctx.dialogs
                    .push(file_dlg::render, file_dlg::event_save_as, state);
                Control::Changed
            });
        }
        MDEvent::StoreConfig => {
            try_flow!(store_config(state, ctx));
        }
        MDEvent::TimeOut(t) => {
            try_flow!(if t.handle == state.clear_status {
                state.status.status(0, "");
                Control::Changed
            } else {
                Control::Continue
            });
        }
        _ => {}
    };

    try_flow!(editor::event(mdevent, &mut state.editor, ctx)?);

    Ok(Control::Continue)
}

fn store_config(state: &mut Scenery, ctx: &mut GlobalState) -> Control<MDEvent> {
    ctx.cfg.store_file_state(&state.editor.split_tab);
    error!("{:?}", ctx.cfg.store());
    Control::Continue
}

fn show_message(msg: &str, ctx: &mut GlobalState) -> Control<MDEvent> {
    if let Some(n) = ctx.dialogs.first::<MsgDialogState>() {
        ctx.dialogs.apply::<MsgDialogState, _>(n, |v| v.append(msg));
    } else {
        let state = MsgDialogState::new_active("Information", msg);
        ctx.dialogs
            .push(msg_dialog::render, msg_dialog::event, state);
    }
    Control::Changed
}

pub fn error(
    event: Error,
    _state: &mut Scenery,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    error!("{:#?}", event);
    Ok(show_message(format!("{:?}", &*event).as_str(), ctx))
}

fn window_cmd(
    state: &mut Scenery,
    event: &crossterm::event::Event,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    state.window_cmd = false;
    let wr = match event {
        ct_event!(key release CONTROL-'w') => {
            state.window_cmd = true;
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
        ct_event!(key press CONTROL-'d') | ct_event!(key press 'd') | ct_event!(key press '+') => {
            Control::Event(MDEvent::Split)
        }
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

    if state.window_cmd {
        ctx.queue(Control::Event(MDEvent::Status(1, "^W".into())));
    } else {
        ctx.queue(Control::Event(MDEvent::Status(1, "".into())));
    }

    // don't let anything through to the application.
    Ok(max(wr, Control::Unchanged))
}

fn handle_menu(
    state: &mut Scenery,
    event: &crossterm::event::Event,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    let r = match state.menu.handle(event, Popup) {
        MenuOutcome::MenuActivated(0, 0) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::MenuNew)
        }
        MenuOutcome::MenuActivated(0, 1) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::MenuOpen)
        }
        MenuOutcome::MenuActivated(0, 2) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::MenuSave)
        }
        MenuOutcome::MenuActivated(0, 3) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::MenuSaveAs)
        }
        MenuOutcome::MenuActivated(0, 4) => {
            _ = flip_esc_focus(state, ctx)?;

            let state = ConfigDialogState::new(ctx)?;
            ctx.dialogs
                .push(config_dlg::render, config_dlg::event, state);
            Control::Changed
        }
        MenuOutcome::MenuActivated(1, 0) => {
            _ = flip_esc_focus(state, ctx)?;

            if let Some((_, sel)) = state.editor.split_tab.selected_mut() {
                ctx.focus().focus(sel);
                Control::Event(MDEvent::MenuFormat)
            } else {
                Control::Continue
            }
        }
        MenuOutcome::MenuActivated(1, 1) => {
            _ = flip_esc_focus(state, ctx)?;

            if let Some((_, sel)) = state.editor.split_tab.selected_mut() {
                ctx.focus().focus(sel);
                Control::Event(MDEvent::MenuFormat)
            } else {
                Control::Continue
            }
        }
        MenuOutcome::MenuActivated(2, 0) => {
            _ = flip_esc_focus(state, ctx)?;

            ctx.cfg.show_ctrl = !ctx.cfg.show_ctrl;
            ctx.queue_event(MDEvent::StoreConfig);
            ctx.queue_event(MDEvent::CfgShowCtrl);
            Control::Changed
        }
        MenuOutcome::MenuActivated(2, 1) => {
            _ = flip_esc_focus(state, ctx)?;

            ctx.cfg.show_break = !ctx.cfg.show_break;
            ctx.queue_event(MDEvent::StoreConfig);
            ctx.queue_event(MDEvent::CfgShowBreak);
            Control::Changed
        }
        MenuOutcome::MenuActivated(2, 2) => {
            _ = flip_esc_focus(state, ctx)?;

            ctx.cfg.wrap_text = !ctx.cfg.wrap_text;
            ctx.queue_event(MDEvent::StoreConfig);
            ctx.queue_event(MDEvent::CfgWrapText);
            Control::Changed
        }
        MenuOutcome::MenuActivated(2, 3) => {
            _ = flip_esc_focus(state, ctx)?;

            ctx.cfg.show_linenr = !ctx.cfg.show_linenr;
            ctx.queue_event(MDEvent::StoreConfig);
            ctx.queue_event(MDEvent::CfgShowLinenr);
            Control::Changed
        }
        MenuOutcome::MenuActivated(2, 4) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::Split)
        }
        MenuOutcome::MenuActivated(2, 5) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::JumpToTree)
        }
        MenuOutcome::MenuActivated(2, 6) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::JumpToFiles)
        }
        MenuOutcome::MenuActivated(2, 7) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Event(MDEvent::HideFiles)
        }
        MenuOutcome::Activated(3) => {
            _ = flip_esc_focus(state, ctx)?;
            Control::Quit
        }
        r => r.into(),
    };

    Ok(r)
}

fn flip_esc_focus(state: &mut Scenery, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
    if !state.menu.is_focused() {
        ctx.focus().focus(&state.menu);
        ctx.queue(Control::Changed);
        Ok(Control::Continue)
    } else {
        if let Some((_, last_edit)) = state.editor.split_tab.selected() {
            ctx.focus().focus(last_edit);
            ctx.queue(Control::Changed);
            Ok(Control::Continue)
        } else {
            state.editor.file_list.focus_files(ctx);
            ctx.queue(Control::Changed);
            Ok(Control::Continue)
        }
    }
}

fn show_help(ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
    let txt = from_utf8(HELP)?;
    let mut txt2 = String::new();
    for l in txt.lines() {
        txt2.push_str(l);
        txt2.push('\n');
    }

    ctx.dialogs.push(
        msg_dialog::render_info,
        msg_dialog::event,
        MsgDialogState::new_active("Help", txt2),
    );
    Ok(Control::Changed)
}

fn show_cheat(ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
    let txt = from_utf8(CHEAT)?;
    let mut txt2 = String::new();
    for l in txt.lines() {
        txt2.push_str(l);
        txt2.push('\n');
    }

    ctx.dialogs.push(
        msg_dialog::render_info,
        msg_dialog::event,
        MsgDialogState::new_active("Cheats", txt2),
    );
    Ok(Control::Changed)
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
