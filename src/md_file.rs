use crate::event::MDEvent;
use crate::global::GlobalState;
use crate::AppContext;
use anyhow::Error;
use log::warn;
use rat_markdown::op::md_format;
use rat_markdown::{parse_md_styles, MarkDown};
use rat_salsa::timer::{TimerDef, TimerHandle};
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::event::{HandleEvent, TextOutcome};
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
        Style::default(),                            // Table
        Style::default().fg(ctx.g.scheme().gray[3]), // Table-Head
        Style::default().fg(ctx.g.scheme().gray[3]), // Table-Row
        Style::default().fg(ctx.g.scheme().gray[3]), // Table-Cell
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
struct CliClipboard;

impl Clipboard for CliClipboard {
    fn get_string(&self) -> Result<String, ClipboardError> {
        match cli_clipboard::get_contents() {
            Ok(v) => Ok(v),
            Err(e) => {
                warn!("{:?}", e);
                Err(ClipboardError)
            }
        }
    }

    fn set_string(&self, s: &str) -> Result<(), ClipboardError> {
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
                let r = match self.edit.handle(event, MarkDown::new(65)) {
                    TextOutcome::TextChanged => self.text_changed(ctx),
                    r => r.into(),
                };

                if self.edit.is_focused() && r == Control::Changed {
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

                r
            }
            MDEvent::CfgNewline => {
                self.edit.set_newline(ctx.g.cfg.new_line.as_str());
                Control::Continue
            }
            MDEvent::CfgShowCtrl => {
                self.edit.set_show_ctrl(ctx.g.cfg.show_ctrl);
                Control::Continue
            }
            _ => Control::Continue,
        };

        Ok(r)
    }
}

impl MDFileState {
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
        text_area.set_clipboard(Some(CliClipboard));
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
        text_area.set_clipboard(Some(CliClipboard));
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
        match md_format(&mut self.edit, 65, eq_width) {
            TextOutcome::TextChanged => self.text_changed(ctx),
            r => r.into(),
        }
    }

    // Flag any text-changes.
    pub fn text_changed(&mut self, ctx: &mut AppContext<'_>) -> Control<MDEvent> {
        self.changed = true;
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
