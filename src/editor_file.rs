use crate::event::MDEvent;
use crate::global::GlobalState;
use crate::AppContext;
use anyhow::{anyhow, Error};
use log::warn;
use pulldown_cmark::{Event, Options, Parser, Tag};
use rat_markdown::dump::md_dump;
use rat_markdown::op::md_format;
use rat_markdown::styles::{parse_md_styles, MDStyle};
use rat_markdown::MarkDown;
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{ct_event, ConsumedEvent, HandleEvent, TextOutcome};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus, Navigation};
use rat_widget::line_number::{LineNumberState, LineNumbers};
use rat_widget::scrolled::Scroll;
use rat_widget::text::clipboard::{Clipboard, ClipboardError};
use rat_widget::text::{upos_type, HasScreenCursor};
use rat_widget::textarea::{TextArea, TextAreaState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, BorderType, Borders, StatefulWidget};
use std::cell::RefCell;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub struct MDFile {
    // vary start margin of the scrollbar
    pub start_margin: u16,
}

#[derive(Debug)]
pub struct MDFileState {
    pub path: PathBuf,
    pub changed: bool,
    pub edit: TextAreaState,
    pub linenr: LineNumberState,
    pub parse_timer: Option<TimerHandle>,
}

impl Clone for MDFileState {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            changed: self.changed,
            edit: self.edit.clone(),
            linenr: Default::default(),
            parse_timer: None,
        }
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

        let line_nr = LineNumbers::new()
            .start(state.edit.offset().1 as upos_type)
            .end(state.edit.len_lines())
            .cursor(state.edit.cursor().y)
            .styles(theme.line_nr_style_doc());

        let line_nr_area = Rect::new(area.x, area.y, line_nr.width(), area.height);
        let text_area = Rect::new(
            area.x + line_nr.width(),
            area.y,
            area.width.saturating_sub(line_nr.width()),
            area.height,
        );

        line_nr.render(line_nr_area, buf, &mut state.linenr);

        TextArea::new()
            .block(
                Block::new()
                    .border_type(BorderType::Rounded)
                    .borders(Borders::RIGHT),
            )
            .vscroll(Scroll::new().start_margin(self.start_margin))
            .styles(theme.textarea_style_doc())
            .text_style(text_style(ctx))
            .render(text_area, buf, &mut state.edit);
        ctx.set_screen_cursor(state.edit.screen_cursor());

        Ok(())
    }
}

fn text_style(ctx: &mut RenderContext<'_, GlobalState>) -> [Style; 34] {
    // base-style: Style::default().fg(self.s.white[0]).bg(self.s.black[1])
    [
        Style::default().fg(ctx.g.scheme().yellow[2]).underlined(), // Heading1,
        Style::default().fg(ctx.g.scheme().yellow[1]).underlined(), // Heading2,
        Style::default().fg(ctx.g.scheme().yellow[1]).underlined(), // Heading3,
        Style::default().fg(ctx.g.scheme().orange[2]).underlined(), // Heading4,
        Style::default().fg(ctx.g.scheme().orange[1]).underlined(), // Heading5,
        Style::default().fg(ctx.g.scheme().orange[1]).underlined(), // Heading6,
        //
        Style::default(),                               // Paragraph
        Style::default().fg(ctx.g.scheme().orange[3]),  // BlockQuote,
        Style::default().fg(ctx.g.scheme().redpink[3]), // CodeBlock,
        Style::default().fg(ctx.g.scheme().redpink[3]), // MathDisplay
        Style::default().fg(ctx.g.scheme().white[3]),   // Rule
        Style::default().fg(ctx.g.scheme().gray[3]),    // Html
        //
        Style::default().fg(ctx.g.scheme().bluegreen[2]), // Link
        Style::default().fg(ctx.g.scheme().bluegreen[2]), // LinkDef
        Style::default().fg(ctx.g.scheme().bluegreen[2]), // Image
        Style::default().fg(ctx.g.scheme().bluegreen[3]), // Footnote Definition
        Style::default().fg(ctx.g.scheme().bluegreen[2]), // Footnote Reference
        //
        Style::default(),                              // List
        Style::default(),                              // Item
        Style::default().fg(ctx.g.scheme().orange[2]), // TaskListMarker
        Style::default().fg(ctx.g.scheme().orange[2]), // ItemTag
        Style::default(),                              // DefinitionList
        Style::default().fg(ctx.g.scheme().orange[3]), // DefinitionListTitle
        Style::default().fg(ctx.g.scheme().orange[2]), // DefinitionListDefinition
        //
        Style::default(),                                 // Table
        Style::default().fg(ctx.g.scheme().secondary[1]), // Table-Head
        Style::default(),                                 // Table-Row
        Style::default(),                                 // Table-Cell
        //
        Style::default().fg(ctx.g.scheme().white[0]).italic(), // Emphasis
        Style::default().fg(ctx.g.scheme().white[3]).bold(),   // Strong
        Style::default().fg(ctx.g.scheme().gray[3]).crossed_out(), // Strikethrough
        Style::default().fg(ctx.g.scheme().redpink[3]),        // CodeInline
        Style::default().fg(ctx.g.scheme().redpink[3]),        // MathInline
        //
        Style::default().fg(ctx.g.scheme().orange[2]), // MetadataBlock
    ]
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
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        let r = match event {
            MDEvent::TimeOut(event) => {
                if self.parse_timer == Some(event.handle) {
                    self.parse_markdown();
                    Control::Changed
                } else {
                    Control::Continue
                }
            }
            MDEvent::Event(event) => {
                // call markdown event-handling instead of regular.
                let mut r = match self.edit.handle(event, MarkDown::new(ctx.g.cfg.text_width)) {
                    TextOutcome::TextChanged => {
                        self.update_cursor_pos(ctx);
                        self.text_changed(ctx)
                    }
                    r => r.into(),
                };
                r = r.or_else_try(|| match event {
                    ct_event!(key press CONTROL-'l') => {
                        self.follow_link() //
                    }
                    ct_event!(keycode press F(8)) => {
                        let r =
                            md_format(&mut self.edit, ctx.g.cfg.text_width as usize, false).into();
                        Ok(r)
                    }
                    ct_event!(keycode press F(7)) => {
                        let r =
                            md_format(&mut self.edit, ctx.g.cfg.text_width as usize, true).into();
                        Ok(r)
                    }
                    ct_event!(key press CONTROL-'p') => {
                        let r = md_dump(&mut self.edit).into();
                        Ok(r)
                    }
                    _ => Ok(Control::Continue),
                })?;
                r
            }
            MDEvent::CfgNewline => {
                self.edit.set_newline(ctx.g.cfg.new_line.as_str());
                Control::Changed
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
    /// Follow the link at the cursor.
    fn follow_link(&self) -> Result<Control<MDEvent>, Error> {
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

        let mut text_area = TextAreaState::named(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
        text_area.set_clipboard(Some(CliClipboard::default()));
        text_area.set_newline(ctx.g.cfg.new_line.as_str());
        text_area.set_show_ctrl(ctx.g.cfg.show_ctrl);
        text_area.set_tab_width(4);

        Self {
            path: path.clone(),
            changed: true,
            edit: text_area,
            linenr: Default::default(),
            parse_timer: None,
        }
    }

    // New editor with existing file.
    pub fn open_file(path: &Path, ctx: &mut AppContext<'_>) -> Result<Self, Error> {
        let path = PathBuf::from(path);

        let mut text_area = TextAreaState::named(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );
        text_area.set_clipboard(Some(CliClipboard::default()));
        let t = fs::read_to_string(&path)?;
        text_area.set_text(t.as_str());
        text_area.set_newline(ctx.g.cfg.new_line.as_str());
        text_area.set_show_ctrl(ctx.g.cfg.show_ctrl);
        text_area.set_tab_width(4);

        Ok(Self {
            path: path.clone(),
            changed: false,
            edit: text_area,
            linenr: Default::default(),
            parse_timer: Some(
                ctx.add_timer(TimerDef::new().next(Instant::now() + Duration::from_millis(0))),
            ),
        })
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

    // Parse & set styles.
    pub fn parse_markdown(&mut self) {
        let styles = parse_md_styles(&self.edit.text());
        self.edit.set_styles(styles);
    }

    // Format selected table
    pub fn md_format(&mut self, eq_width: bool, ctx: &mut AppContext<'_>) -> Control<MDEvent> {
        match md_format(&mut self.edit, ctx.g.cfg.text_width as usize, eq_width) {
            TextOutcome::TextChanged => self.text_changed(ctx),
            r => r.into(),
        }
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
