use crate::config::MDConfig;
use crate::edit::{MDEdit, MDEditState};
use crate::event::MDEvent;
use crate::facilities::{Facility, MDFileDialog, MDFileDialogState};
use crate::global::GlobalState;
use crate::theme::{dark_themes, DarkTheme};
use anyhow::Error;
use rat_salsa::poll::{PollCrossterm, PollRendered, PollTasks, PollTimers};
use rat_salsa::{run_tui, AppState, AppWidget, Control, RenderContext, RunConfig};
use rat_theme::scheme::IMPERIAL;
use rat_widget::event::{
    ct_event, try_flow, ConsumedEvent, Dialog, HandleEvent, MenuOutcome, Popup, Regular,
};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
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
use std::fs;
use std::str::from_utf8;
use std::time::{Duration, SystemTime};

type AppContext<'a> = rat_salsa::AppContext<'a, GlobalState, MDEvent, Error>;

mod config;
mod edit;
mod event;
mod facilities;
mod file_list;
mod global;
mod md_file;
mod split_tab;
mod theme;

fn main() -> Result<(), Error> {
    setup_logging()?;

    let config = MDConfig {
        show_ctrl: false,
        new_line: if cfg!(windows) {
            "\r\n".to_string()
        } else {
            "\n".to_string()
        },
    };
    let theme = DarkTheme::new("Imperial".into(), IMPERIAL);
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
    use_crlf: bool,
}

impl<'a> MenuStructure<'a> for Menu {
    fn menus(&'a self, menu: &mut MenuBuilder<'a>) {
        menu.item_parsed("_File")
            .item_parsed("_Edit")
            .item_parsed("_View")
            .item_parsed("_Theme")
            .item_parsed("_Quit");
    }

    fn submenu(&'a self, n: usize, submenu: &mut MenuBuilder<'a>) {
        match n {
            0 => {
                submenu.item_parsed("_New..|Ctrl-N");
                submenu.item_parsed("_Open..|Ctrl-O");
                submenu.item_parsed("_Save..|Ctrl-S");
                submenu.item_parsed("Save _as..");
            }
            1 => {
                submenu.item_parsed("Format Item|Alt-F");
                submenu.item_parsed("Alt-Format Item|Alt-Shift-F");
            }
            2 => {
                if self.show_ctrl {
                    submenu.item_parsed("\u{2611} Control chars");
                } else {
                    submenu.item_parsed("\u{2610} Control chars");
                }
                if self.use_crlf {
                    submenu.item_parsed("\u{2611} Use CR+LF");
                } else {
                    submenu.item_parsed("\u{2610} Use CR+LF");
                }
                submenu.separator(Separator::Dotted);
                submenu.item_parsed("_Split view|Ctrl-W D");
                submenu.item_parsed("_Jump to File|F5");
                submenu.item_parsed("_Hide files|F6");
            }
            3 => {
                for t in dark_themes() {
                    submenu.item_string(t.name().into());
                }
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

    pub message_dlg: MsgDialogState,
    pub error_dlg: MsgDialogState,
    pub file_dlg: MDFileDialogState,
}

impl Default for MDAppState {
    fn default() -> Self {
        let s = Self {
            editor: MDEditState::default(),
            menu: MenubarState::named("menu"),
            status: Default::default(),
            message_dlg: Default::default(),
            error_dlg: Default::default(),
            file_dlg: Default::default(),
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

        let r = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
        let s = Layout::horizontal([Constraint::Percentage(61), Constraint::Percentage(39)])
            .split(r[1]);

        MDEdit.render(r[0], buf, &mut state.editor, ctx)?;

        let menu_struct = Menu {
            show_ctrl: ctx.g.cfg.show_ctrl,
            use_crlf: ctx.g.cfg.new_line == "\r\n",
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

        let l_fd = layout_middle(
            r[0],
            Constraint::Length(state.menu.bar.item_areas[0].x),
            Constraint::Percentage(39),
            Constraint::Percentage(39),
            Constraint::Length(0),
        );
        MDFileDialog::new()
            .style(theme.file_dialog_style())
            .render(l_fd, buf, &mut state.file_dlg);
        ctx.set_screen_cursor(state.file_dlg.screen_cursor());

        menu_popup.render(s[0], buf, &mut state.menu);

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
                                .fg(theme.scheme().white[2])
                                .bg(theme.scheme().deepblue[0]),
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
        self.menu.bar.select(Some(0));
        self.menu.focus().set(true);
        self.editor.init(ctx)?;
        Ok(())
    }

    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r = match event {
            MDEvent::Event(event) => self.crossterm(event, ctx)?,
            MDEvent::Rendered => {
                // rebuild keyboard + mouse focus
                ctx.focus = Some(FocusBuilder::rebuild_for(self, ctx.focus.take()));
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
            _ => Control::Continue,
        };

        r = r.or_else_try(|| self.editor.event(event, ctx))?;

        if self.editor.set_active_split() {
            self.editor.sync_views(ctx)?;
        }

        Ok(r)
    }

    fn error(&self, event: Error, _ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.error_dlg.title("Error occured");
        self.error_dlg.append(format!("{:?}", &*event).as_str());
        Ok(Control::Changed)
    }
}

impl MDAppState {
    fn crossterm(
        &mut self,
        event: &crossterm::event::Event,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        try_flow!(self.error_dlg.handle(event, Dialog));
        try_flow!(self.message_dlg.handle(event, Dialog));
        try_flow!(self.file_dlg.handle(event)?);

        let f = Control::from(ctx.focus_mut().handle(event, Regular));
        ctx.queue(f);

        // regular global
        let mut r = match &event {
            ct_event!(resized) => Control::Changed,
            ct_event!(key press CONTROL-'q') => Control::Quit,
            ct_event!(keycode press Esc) => {
                if !self.menu.is_focused() {
                    ctx.focus().focus(&self.menu);
                    Control::Changed
                } else {
                    if let Some((_, last_edit)) = self.editor.split_tab.selected() {
                        ctx.focus().focus(last_edit);
                        Control::Changed
                    } else {
                        Control::Continue
                    }
                }
            }
            ct_event!(keycode press F(1)) => {
                let txt = from_utf8(HELP)?;
                let mut txt2 = String::new();
                for l in txt.lines() {
                    txt2.push_str(l);
                    txt2.push('\n');
                }
                self.message_dlg.append(&txt2);
                Control::Changed
            }
            ct_event!(keycode press F(2)) => {
                let txt = from_utf8(CHEAT)?;
                let mut txt2 = String::new();
                for l in txt.lines() {
                    txt2.push_str(l);
                    txt2.push('\n');
                }
                self.message_dlg.append(&txt2);
                Control::Changed
            }
            _ => Control::Continue,
        };

        r = r.or_else(|| match self.menu.handle(event, Popup) {
            MenuOutcome::MenuActivated(0, 0) => Control::Event(MDEvent::MenuNew),
            MenuOutcome::MenuActivated(0, 1) => Control::Event(MDEvent::MenuOpen),
            MenuOutcome::MenuActivated(0, 2) => Control::Event(MDEvent::MenuSave),
            MenuOutcome::MenuActivated(0, 3) => Control::Event(MDEvent::MenuSaveAs),
            MenuOutcome::MenuActivated(1, 0) => {
                if let Some((_, sel)) = self.editor.split_tab.selected_mut() {
                    ctx.focus().focus(sel);
                    sel.md_format(false, ctx)
                } else {
                    Control::Continue
                }
            }
            MenuOutcome::MenuActivated(1, 1) => {
                if let Some((_, sel)) = self.editor.split_tab.selected_mut() {
                    ctx.focus().focus(sel);
                    sel.md_format(true, ctx)
                } else {
                    Control::Continue
                }
            }
            MenuOutcome::MenuActivated(2, 0) => {
                ctx.g.cfg.show_ctrl = !ctx.g.cfg.show_ctrl;
                Control::Event(MDEvent::CfgShowCtrl)
            }
            MenuOutcome::MenuActivated(2, 1) => {
                let changed = if ctx.g.cfg.new_line.as_str() == "\r\n" {
                    "\n".into()
                } else {
                    "\r\n".into()
                };
                ctx.g.cfg.new_line = changed;
                Control::Event(MDEvent::CfgNewline)
            }
            MenuOutcome::MenuActivated(2, 2) => Control::Event(MDEvent::Split),
            MenuOutcome::MenuActivated(2, 3) => Control::Event(MDEvent::JumpToFiles),
            MenuOutcome::MenuActivated(2, 4) => Control::Event(MDEvent::HideFiles),
            MenuOutcome::MenuSelected(3, n) => {
                ctx.g.theme = dark_themes()[n].clone();
                Control::Changed
            }
            MenuOutcome::Activated(4) => Control::Quit,
            r => r.into(),
        });

        Ok(r)
    }
}

fn setup_logging() -> Result<(), Error> {
    // todo: ???
    _ = fs::remove_file("log.log");
    fern::Dispatch::new()
        .format(|out, message, _record| out.finish(format_args!("{}", message)))
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file("log.log")?)
        .apply()?;
    Ok(())
}

static HELP: &[u8] = include_bytes!("mdedit.md");
static CHEAT: &[u8] = include_bytes!("cheat.md");
