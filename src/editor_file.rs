use crate::doc_type::{DocType, DocTypes};
use crate::global::event::MDEvent;
use crate::global::GlobalState;
use anyhow::{anyhow, Error};
use log::warn;
use pulldown_cmark::{Event, Options, Parser, Tag};
use rat_markdown::styles::MDStyle;
use rat_markdown::MarkDown;
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{Control, SalsaContext};
use rat_widget::event::util::MouseFlags;
use rat_widget::event::{ct_event, try_flow, ConsumedEvent, HandleEvent, TextOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus, Navigation};
use rat_widget::line_number::{LineNumberState, LineNumbers};
use rat_widget::scrolled::Scroll;
use rat_widget::text::clipboard::{Clipboard, ClipboardError};
use rat_widget::text::HasScreenCursor;
use rat_widget::textarea::{TextArea, TextAreaState, TextWrap};
use rat_widget::util::fill_buf_area;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, BorderType, Borders, StatefulWidget};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug)]
pub struct MDFileState {
    pub path: PathBuf,
    pub changed: bool,
    pub doc_type: DocTypes,
    pub edit: TextAreaState,
    pub edit_mouse: MouseFlags,
    pub show_linenr: bool,
    pub linenr: LineNumberState,
    pub parse_timer: Option<TimerHandle>,
}

pub fn render(
    start_margin: u16,
    area: Rect,
    buf: &mut Buffer,
    state: &mut MDFileState,
    ctx: &mut GlobalState,
) -> Result<(), Error> {
    let theme = &ctx.theme;

    let ln_width = if state.show_linenr {
        LineNumbers::width_for(state.edit.vertical_offset(), 0, (0, 0), 0)
    } else {
        1
    };

    let text_area = Rect::new(
        area.x + ln_width,
        area.y,
        area.width.saturating_sub(ln_width),
        area.height,
    );

    TextArea::new()
        .block(
            Block::new()
                .border_type(BorderType::Rounded)
                .borders(Borders::RIGHT),
        )
        .vscroll(Scroll::new().start_margin(start_margin))
        .styles(theme.textarea_style_doc())
        .text_style_map(text_style(ctx))
        .render(text_area, buf, &mut state.edit);

    if state.show_linenr {
        let line_nr_area = Rect::new(area.x, area.y, ln_width, area.height);
        LineNumbers::new()
            .with_textarea(&state.edit)
            .styles(theme.line_nr_style_doc())
            .render(line_nr_area, buf, &mut state.linenr);
    } else {
        let line_nr_area = Rect::new(area.x, area.y, ln_width, area.height);
        fill_buf_area(buf, line_nr_area, " ", theme.doc_base());
    }

    ctx.set_screen_cursor(state.edit.screen_cursor());

    Ok(())
}

fn text_style(ctx: &GlobalState) -> HashMap<usize, Style> {
    let sc = ctx.scheme();
    let sty = |c: Color| Style::new().fg(c);

    let mut map = HashMap::new();

    //let base = sc.white[0];
    map.insert(MDStyle::Heading1.into(), sty(sc.white[3]).underlined());
    map.insert(MDStyle::Heading2.into(), sty(sc.white[3]).underlined());
    map.insert(MDStyle::Heading3.into(), sty(sc.white[2]).underlined());
    map.insert(MDStyle::Heading4.into(), sty(sc.white[2]).underlined());
    map.insert(MDStyle::Heading5.into(), sty(sc.white[1]).underlined());
    map.insert(MDStyle::Heading6.into(), sty(sc.white[1]).underlined());

    map.insert(MDStyle::Paragraph.into(), Style::new());
    map.insert(MDStyle::BlockQuote.into(), sty(sc.orange[2]));
    map.insert(MDStyle::CodeBlock.into(), sty(sc.redpink[2]));
    map.insert(MDStyle::MathDisplay.into(), sty(sc.redpink[2]));
    map.insert(MDStyle::Rule.into(), sty(sc.white[2]));
    map.insert(MDStyle::Html.into(), sty(sc.gray[2]));

    map.insert(MDStyle::Link.into(), sty(sc.bluegreen[1]).underlined());
    map.insert(MDStyle::LinkDef.into(), sty(sc.bluegreen[1]));
    map.insert(MDStyle::Image.into(), sty(sc.bluegreen[1]).underlined());
    map.insert(MDStyle::FootnoteDefinition.into(), sty(sc.bluegreen[2]));
    map.insert(
        MDStyle::FootnoteReference.into(),
        sty(sc.bluegreen[1]).underlined(),
    );

    map.insert(MDStyle::List.into(), Style::new());
    map.insert(MDStyle::Item.into(), Style::new());
    map.insert(MDStyle::TaskListMarker.into(), sty(sc.orange[1]));
    map.insert(MDStyle::ItemTag.into(), sty(sc.orange[1]));
    map.insert(MDStyle::DefinitionList.into(), Style::new());
    map.insert(MDStyle::DefinitionListTitle.into(), sty(sc.orange[2]));
    map.insert(MDStyle::DefinitionListDefinition.into(), sty(sc.orange[1]));

    map.insert(MDStyle::Table.into(), Style::new());
    map.insert(MDStyle::TableHead.into(), sty(sc.orange[2]));
    map.insert(MDStyle::TableRow.into(), Style::new());
    map.insert(MDStyle::TableCell.into(), Style::new());

    map.insert(MDStyle::Emphasis.into(), Style::new().italic());
    map.insert(MDStyle::Strong.into(), Style::new().bold());
    map.insert(MDStyle::Strikethrough.into(), Style::new().crossed_out());

    map.insert(MDStyle::CodeInline.into(), sty(sc.redpink[1]));
    map.insert(MDStyle::MathInline.into(), sty(sc.redpink[1]));
    map.insert(MDStyle::MetadataBlock.into(), sty(sc.orange[1]));

    map
}

impl Clone for MDFileState {
    fn clone(&self) -> Self {
        let mut s = Self {
            path: self.path.clone(),
            changed: self.changed,
            doc_type: self.doc_type,
            edit: self.edit.clone(),
            edit_mouse: self.edit_mouse.clone(),
            show_linenr: self.show_linenr,
            linenr: self.linenr.clone(),
            parse_timer: None,
        };

        // todo: cleanup
        let nnn = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("fine")
            .as_millis()
            % 86400;
        s.edit.focus = FocusFlag::named(format!("{} {}", s.edit.focus.name(), nnn).as_str());

        s
    }
}

impl HasFocus for MDFileState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }

    fn focus(&self) -> FocusFlag {
        self.edit.focus()
    }

    fn area(&self) -> Rect {
        self.edit.area()
    }

    fn navigable(&self) -> Navigation {
        self.edit.navigable()
    }
}

#[derive(Debug, Default, Clone)]
struct CliClipboard {
    clip: RefCell<String>,
}

impl Clipboard for CliClipboard {
    fn get_string(&self) -> Result<String, ClipboardError> {
        match cli_clipboard::get_contents() {
            Ok(v) => Ok(v),
            Err(e) => {
                warn!("{:?}", e);
                Ok(self.clip.borrow().clone())
            }
        }
    }

    fn set_string(&self, s: &str) -> Result<(), ClipboardError> {
        let mut clip = self.clip.borrow_mut();
        *clip = s.to_string();

        match cli_clipboard::set_contents(s.to_string()) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("{:?}", e);
                Err(ClipboardError)
            }
        }
    }
}

pub fn event(
    event: &MDEvent,
    state: &mut MDFileState,
    ctx: &mut GlobalState,
) -> Result<Control<MDEvent>, Error> {
    match event {
        MDEvent::TimeOut(event) => {
            try_flow!(if state.parse_timer == Some(event.handle) {
                state.doc_type.parse(&mut state.edit);
                Control::Changed
            } else {
                Control::Continue
            });
        }
        MDEvent::Event(event) => {
            // click click
            try_flow!(match event {
                ct_event!(mouse any for m) if state.edit_mouse.doubleclick(state.edit.inner, m) => {
                    state.follow_link(ctx)?
                }
                _ => Control::Continue,
            });
            // call markdown event-handling instead of regular.
            try_flow!(
                match state.edit.handle(event, MarkDown::new(ctx.cfg.text_width)) {
                    TextOutcome::TextChanged => {
                        state.update_cursor_pos(ctx);
                        state.text_changed(ctx)
                    }
                    TextOutcome::Changed => {
                        state.update_cursor_pos(ctx);
                        Control::Changed
                    }
                    r => r.into(),
                }
            );

            if state.is_focused() {
                try_flow!(match event {
                    ct_event!(key press CONTROL-'l') => {
                        state.follow_link(ctx)? //
                    }
                    ct_event!(keycode press F(8)) => {
                        if state.edit.is_focused() {
                            state.reformat(false, ctx)?
                        } else {
                            Control::Continue
                        }
                    }
                    ct_event!(keycode press F(7)) => {
                        if state.edit.is_focused() {
                            state.reformat(true, ctx)?
                        } else {
                            Control::Continue
                        }
                    }
                    ct_event!(key press CONTROL-'p') => {
                        if state.edit.is_focused() {
                            state.doc_type.log_parser(&state.edit);
                            Control::Continue
                        } else {
                            Control::Continue
                        }
                    }
                    _ => Control::Continue,
                });
            }
        }
        MDEvent::MenuFormat => {
            try_flow!(if state.edit.is_focused() {
                state.reformat(false, ctx)?
            } else {
                Control::Continue
            });
        }
        MDEvent::MenuFormatEq => {
            try_flow!(if state.edit.is_focused() {
                state.reformat(true, ctx)?
            } else {
                Control::Continue
            });
        }
        MDEvent::CfgShowCtrl => {
            try_flow!({
                state.edit.set_show_ctrl(ctx.cfg.show_ctrl);
                Control::Changed
            });
        }
        MDEvent::CfgShowBreak => {
            try_flow!({
                state.edit.set_wrap_ctrl(ctx.cfg.show_break);
                Control::Changed
            });
        }
        MDEvent::CfgShowLinenr => {
            try_flow!({
                state.show_linenr = !state.show_linenr;
                Control::Changed
            });
        }
        MDEvent::CfgWrapText => {
            try_flow!({
                state.edit.set_text_wrap(if ctx.cfg.wrap_text {
                    TextWrap::Word(8)
                } else {
                    TextWrap::Shift
                });
                Control::Changed
            });
        }
        _ => {}
    }

    Ok(Control::Continue)
}

impl MDFileState {
    /// Reformat
    fn reformat(
        &mut self,
        eq_width: bool,
        ctx: &mut GlobalState,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r: Control<MDEvent> = self
            .doc_type
            .format(&mut self.edit, ctx.cfg.text_width, eq_width)
            .into();
        r = r.and_then(|| {
            self.update_cursor_pos(ctx);
            self.text_changed(ctx)
        });
        Ok(r)
    }

    /// Follow the link at the cursor.
    fn follow_link(&mut self, ctx: &mut GlobalState) -> Result<Control<MDEvent>, Error> {
        let pos = self.edit.byte_at(self.edit.cursor());
        let Some(link_range) = self.edit.style_match(pos.start, MDStyle::Link.into()) else {
            return Ok(Control::Continue);
        };

        let link_txt = self.edit.str_slice_byte(link_range);
        let p = Parser::new_ext(link_txt.as_ref(), Options::empty()).into_iter();
        for e in p {
            match e {
                Event::Start(Tag::Link { dest_url, .. }) => {
                    if !dest_url.starts_with("/") && dest_url.ends_with(".md") {
                        if let Some(parent) = self.path.parent() {
                            let path = parent.join(dest_url.as_ref());

                            // auto-create
                            if !path.exists() {
                                if let Some(parent) = path.parent() {
                                    fs::create_dir_all(parent)?;
                                    File::create(&path)?;
                                    ctx.queue_event(MDEvent::SyncFileList);
                                }
                            }

                            return Ok(Control::Event(MDEvent::SelectOrOpen(path)));
                        } else {
                            return Err(anyhow!("Can't locate current file??"));
                        }
                    } else {
                        return Err(anyhow!("Can't follow this link."));
                    }
                }
                _ => {}
            }
        }

        Ok(Control::Continue)
    }

    // New editor with fresh file.
    pub fn new_file(path: &Path, ctx: &mut GlobalState) -> MDFileState {
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("md");
        }

        let doc_type = Self::doc_type(&path);

        let mut edit = TextAreaState::named(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
        edit.set_clipboard(Some(CliClipboard::default()));
        edit.set_show_ctrl(ctx.cfg.show_ctrl);
        edit.set_wrap_ctrl(ctx.cfg.show_break);
        edit.set_text_wrap(if ctx.cfg.wrap_text {
            TextWrap::Word(8)
        } else {
            TextWrap::Shift
        });
        edit.set_tab_width(4);

        MDFileState {
            path: path.clone(),
            changed: Default::default(),
            doc_type,
            edit,
            edit_mouse: Default::default(),
            show_linenr: ctx.cfg.show_linenr,
            linenr: Default::default(),
            parse_timer: None,
        }
    }

    // New editor with existing file.
    pub fn open_file(path: &Path, ctx: &mut GlobalState) -> Result<MDFileState, Error> {
        let path = PathBuf::from(path);

        let doc_type = Self::doc_type(&path);

        let mut edit = TextAreaState::named(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
        edit.set_clipboard(Some(CliClipboard::default()));
        let t = fs::read_to_string(&path)?;
        edit.set_text(t.as_str());
        edit.set_show_ctrl(ctx.cfg.show_ctrl);
        edit.set_wrap_ctrl(ctx.cfg.show_break);
        edit.set_text_wrap(if ctx.cfg.wrap_text {
            TextWrap::Word(8)
        } else {
            TextWrap::Shift
        });
        edit.set_tab_width(4);

        Ok(MDFileState {
            path: path.clone(),
            changed: Default::default(),
            doc_type,
            edit,
            edit_mouse: Default::default(),
            show_linenr: ctx.cfg.show_linenr,
            linenr: Default::default(),
            parse_timer: Some(
                ctx.add_timer(TimerDef::new().next(Instant::now() + Duration::from_millis(0))),
            ),
        })
    }

    fn doc_type(path: &Path) -> DocTypes {
        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().as_ref() {
                "md" => DocTypes::MD,
                _ => DocTypes::TXT,
            }
        } else {
            DocTypes::TXT
        }
    }

    // Save as
    pub fn save_as(&mut self, path: &Path) -> Result<(), Error> {
        self.path = path.into();
        self.save()
    }

    // Save
    pub fn save(&mut self) -> Result<(), Error> {
        if self.changed {
            let mut f = BufWriter::new(File::create(&self.path)?);
            let mut buf = Vec::new();
            for line in self.edit.text().lines() {
                buf.extend(line.bytes());
                buf.extend_from_slice(self.edit.newline().as_bytes());
            }
            f.write_all(&buf)?;

            self.changed = false;
        }
        Ok(())
    }

    // Update cursor info
    pub fn update_cursor_pos(&mut self, ctx: &mut GlobalState) {
        // update cursor / selection info
        if self.edit.is_focused() {
            let cursor = self.edit.cursor();
            let sel = self.edit.selection();
            let sel_len = if sel.start.y == sel.end.y {
                sel.end.x.saturating_sub(sel.start.x)
            } else {
                sel.end.y.saturating_sub(sel.start.y) + 1
            };
            ctx.queue(Control::Event(MDEvent::Status(
                1,
                format!("{}:{}|{}", cursor.x, cursor.y, sel_len),
            )));
        }
    }

    // Flag any text-changes.
    pub fn text_changed(&mut self, ctx: &mut GlobalState) -> Control<MDEvent> {
        self.changed = self.edit.undo_buffer().expect("undo").open_undo() > 0;
        // send sync
        ctx.queue(Control::Event(MDEvent::SyncEdit));
        // restart timer
        self.parse_timer = Some(ctx.replace_timer(
            self.parse_timer,
            TimerDef::new().next(Instant::now() + Duration::from_millis(200)),
        ));
        Control::Changed
    }
}
