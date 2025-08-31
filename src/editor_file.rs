use crate::doc_type::{DocType, DocTypes};
use crate::event::MDEvent;
use crate::global::GlobalState;
use crate::AppContext;
use anyhow::{anyhow, Error};
use log::warn;
use pulldown_cmark::{Event, Options, Parser, Tag};
use rat_markdown::styles::MDStyle;
use rat_markdown::MarkDown;
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::util::MouseFlags;
use rat_widget::event::{ct_event, ConsumedEvent, HandleEvent, TextOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus, Navigation};
use rat_widget::line_number::{LineNumberState, LineNumbers};
use rat_widget::scrolled::Scroll;
use rat_widget::text::clipboard::{Clipboard, ClipboardError};
use rat_widget::text::HasScreenCursor;
use rat_widget::textarea::{TextArea, TextAreaState, TextWrap};
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

#[derive(Debug, Default, Clone)]
pub struct MDFile {
    // vary start margin of the scrollbar
    start_margin: u16,
}

#[derive(Debug)]
pub struct MDFileState {
    pub path: PathBuf,
    pub changed: bool,
    pub doc_type: DocTypes,
    pub edit: TextAreaState,
    pub edit_mouse: MouseFlags,
    pub linenr: LineNumberState,
    pub parse_timer: Option<TimerHandle>,
}

impl MDFile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_margin(mut self, start_margin: u16) -> Self {
        self.start_margin = start_margin;
        self
    }
}

impl Clone for MDFileState {
    fn clone(&self) -> Self {
        let mut s = Self {
            path: self.path.clone(),
            changed: self.changed,
            doc_type: self.doc_type,
            edit: self.edit.clone(),
            edit_mouse: self.edit_mouse.clone(),
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

impl AppWidget<GlobalState, MDEvent, Error> for MDFile {
    type State = MDFileState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let theme = &ctx.g.theme;

        let ln_width = LineNumbers::width_for(state.edit.vertical_offset(), 0, (0, 0), 0);

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
            .vscroll(Scroll::new().start_margin(self.start_margin))
            .styles(theme.textarea_style_doc())
            .text_style_map(text_style(ctx))
            .render(text_area, buf, &mut state.edit);

        let line_nr_area = Rect::new(area.x, area.y, ln_width, area.height);
        LineNumbers::new()
            .with_textarea(&state.edit)
            .styles(theme.line_nr_style_doc())
            .render(line_nr_area, buf, &mut state.linenr);

        ctx.set_screen_cursor(state.edit.screen_cursor());

        Ok(())
    }
}

fn text_style(ctx: &RenderContext<'_, GlobalState>) -> HashMap<usize, Style> {
    let sc = ctx.g.scheme();
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

impl AppState<GlobalState, MDEvent, Error> for MDFileState {
    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let r = match event {
            MDEvent::TimeOut(event) => {
                if self.parse_timer == Some(event.handle) {
                    self.doc_type.parse(&mut self.edit);
                    Control::Changed
                } else {
                    Control::Continue
                }
            }
            MDEvent::Event(event) => {
                match event {
                    ct_event!(mouse any for m)
                        if self.edit_mouse.doubleclick(self.edit.inner, m) =>
                    {
                        match self.follow_link(ctx) {
                            Ok(md @ Control::Event(MDEvent::SelectOrOpen(_))) => ctx.queue(md),
                            _ => {}
                        }
                    }
                    _ => {}
                }
                // call markdown event-handling instead of regular.
                let mut r = match self.edit.handle(event, MarkDown::new(ctx.g.cfg.text_width)) {
                    TextOutcome::TextChanged => {
                        self.update_cursor_pos(ctx);
                        self.text_changed(ctx)
                    }
                    TextOutcome::Changed => {
                        self.update_cursor_pos(ctx);
                        Control::Changed
                    }
                    r => r.into(),
                };
                r = r.or_else_try(|| match event {
                    ct_event!(key press CONTROL-'l') => {
                        self.follow_link(ctx) //
                    }
                    ct_event!(keycode press F(8)) => {
                        if self.edit.is_focused() {
                            self.reformat(false, ctx)
                        } else {
                            Ok(Control::Continue)
                        }
                    }
                    ct_event!(keycode press F(7)) => {
                        if self.edit.is_focused() {
                            self.reformat(true, ctx)
                        } else {
                            Ok(Control::Continue)
                        }
                    }
                    ct_event!(key press CONTROL-'p') => {
                        if self.edit.is_focused() {
                            self.doc_type.log_parser(&self.edit);
                            Ok(Control::Continue)
                        } else {
                            Ok(Control::Continue)
                        }
                    }
                    ct_event!(key press ALT-'w') => match self.edit.text_wrap() {
                        TextWrap::Shift => {
                            self.edit.set_text_wrap(TextWrap::Word(6));
                            Ok(Control::Changed)
                        }
                        TextWrap::Hard | TextWrap::Word(_) => {
                            self.edit.set_text_wrap(TextWrap::Shift);
                            Ok(Control::Changed)
                        }
                        _ => {
                            self.edit.set_text_wrap(TextWrap::Word(6));
                            Ok(Control::Changed)
                        }
                    },
                    _ => Ok(Control::Continue),
                })?;
                r
            }
            MDEvent::MenuFormat => {
                if self.edit.is_focused() {
                    self.reformat(false, ctx)?
                } else {
                    Control::Continue
                }
            }
            MDEvent::MenuFormatEq => {
                if self.edit.is_focused() {
                    self.reformat(true, ctx)?
                } else {
                    Control::Continue
                }
            }
            MDEvent::CfgShowCtrl => {
                self.edit.set_show_ctrl(ctx.g.cfg.show_ctrl);
                Control::Changed
            }
            _ => Control::Continue,
        };

        Ok(r)
    }
}

impl MDFileState {
    /// Reformat
    fn reformat(
        &mut self,
        eq_width: bool,
        ctx: &mut AppContext<'_>,
    ) -> Result<Control<MDEvent>, Error> {
        let mut r: Control<MDEvent> = self
            .doc_type
            .format(&mut self.edit, ctx.g.cfg.text_width, eq_width)
            .into();
        r = r.and_then(|| {
            self.update_cursor_pos(ctx);
            self.text_changed(ctx)
        });
        Ok(r)
    }

    /// Follow the link at the cursor.
    fn follow_link(&self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
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
                                    ctx.queue(Control::Event(MDEvent::SyncFileList));
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
    pub fn new_file(path: &Path, ctx: &mut AppContext<'_>) -> Self {
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
        edit.set_show_ctrl(ctx.g.cfg.show_ctrl);
        edit.set_tab_width(4);

        Self {
            path: path.clone(),
            changed: Default::default(),
            doc_type,
            edit,
            edit_mouse: Default::default(),
            linenr: Default::default(),
            parse_timer: None,
        }
    }

    // New editor with existing file.
    pub fn open_file(path: &Path, ctx: &mut AppContext<'_>) -> Result<Self, Error> {
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
        edit.set_show_ctrl(ctx.g.cfg.show_ctrl);
        edit.set_tab_width(4);

        Ok(Self {
            path: path.clone(),
            changed: Default::default(),
            doc_type,
            edit,
            edit_mouse: Default::default(),
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
    pub fn update_cursor_pos(&self, ctx: &mut AppContext<'_>) {
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
    pub fn text_changed(&mut self, ctx: &mut AppContext<'_>) -> Control<MDEvent> {
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
