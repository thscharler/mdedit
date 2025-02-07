use crate::event::MDEvent;
use crate::global::GlobalState;
use crate::theme::dark_themes;
use crate::AppContext;
use anyhow::Error;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rat_salsa::{AppState, AppWidget, Control, RenderContext};
use rat_widget::button::{Button, ButtonState};
use rat_widget::choice::{Choice, ChoiceState};
use rat_widget::event::{ButtonOutcome, ChoiceOutcome, ConsumedEvent, HandleEvent, Popup, Regular};
use rat_widget::focus::{impl_has_focus, FocusBuilder, FocusFlag, HasFocus};
use rat_widget::layout::{FormLabel, FormWidget, LayoutForm};
use rat_widget::number_input::{NumberInput, NumberInputState};
use rat_widget::pager::{Form, FormState};
use rat_widget::text::HasScreenCursor;
use rat_widget::text_input::{TextInput, TextInputState};
use rat_widget::util::reset_buf_area;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::widgets::{Block, Padding, StatefulWidget, Widget};
use std::cmp::max;
use std::rc::Rc;

#[derive(Debug)]
pub struct ConfigDialog;

#[derive(Debug, Default)]
pub struct ConfigDialogState {
    active: bool,

    form: FormState<FocusFlag>,
    theme: ChoiceState<String>,
    text_width: NumberInputState,
    globs: TextInputState,

    ok_button: ButtonState,
    cancel_button: ButtonState,
}

impl AppWidget<GlobalState, MDEvent, Error> for ConfigDialog {
    type State = ConfigDialogState;

    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
        ctx: &mut RenderContext<'_, GlobalState>,
    ) -> Result<(), Error> {
        let block = Block::bordered()
            .style(ctx.g.theme.dialog_base())
            .border_style(ctx.g.theme.dialog_border());
        let inner = block.inner(area);

        let l = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        reset_buf_area(area, buf);
        block.render(area, buf);

        let mut form = Form::new() //
            .style(ctx.g.theme.dialog_base());

        let layout_size = form.layout_size(l[0]);
        if !state.form.valid_layout(layout_size) {
            let mut layout = LayoutForm::new()
                .spacing(1)
                .line_spacing(1)
                .flex(Flex::Legacy);

            layout.widget(
                state.theme.focus(),
                FormLabel::Str("Theme"),
                FormWidget::Width(25),
            );
            layout.widget(
                state.text_width.focus(),
                FormLabel::Str("Text break at"),
                FormWidget::Width(15),
            );
            layout.widget(
                state.globs.focus(),
                FormLabel::Str("Files glob"),
                FormWidget::Width(35),
            );
            form = form.layout(layout.endless(layout_size.width, Padding::new(1, 1, 1, 1)));
        }
        let mut form = form.into_buffer(l[0], buf, &mut state.form);

        let choice_overlay = form.render2(
            state.theme.focus(),
            || {
                Choice::new()
                    .styles(ctx.g.theme.choice_style())
                    .items(
                        dark_themes()
                            .iter()
                            .map(|v| (v.name().to_string(), v.name().to_string())),
                    )
                    .into_widgets()
            },
            &mut state.theme,
        );
        form.render(
            state.text_width.focus(),
            || NumberInput::new().styles(ctx.g.theme.text_style()),
            &mut state.text_width,
        );
        form.render(
            state.globs.focus(),
            || TextInput::new().styles(ctx.g.theme.text_style()),
            &mut state.globs,
        );
        form.render_opt(state.theme.focus(), || choice_overlay, &mut state.theme);

        // that "§$"§ curser
        ctx.set_screen_cursor(
            state
                .text_width
                .screen_cursor()
                .or(state.globs.screen_cursor()),
        );

        // buttons
        let l2 = Layout::horizontal([Constraint::Length(15), Constraint::Length(15)])
            .spacing(1)
            .flex(Flex::End)
            .split(l[2]);

        Button::new("Cancel")
            .styles(ctx.g.theme.button_style())
            .render(l2[0], buf, &mut state.cancel_button);
        Button::new("Ok")
            .styles(ctx.g.theme.button_style()) //
            .render(l2[1], buf, &mut state.ok_button);

        Ok(())
    }
}

impl_has_focus!(theme, text_width, globs, ok_button, cancel_button for ConfigDialogState);

impl AppState<GlobalState, MDEvent, Error> for ConfigDialogState {
    fn init(&mut self, ctx: &mut AppContext<'_>) -> Result<(), Error> {
        self.text_width.set_format_loc("###0", ctx.g.cfg.loc)?;
        Ok(())
    }

    fn event(
        &mut self,
        event: &MDEvent,
        ctx: &mut rat_salsa::AppContext<'_, GlobalState, MDEvent, Error>,
    ) -> Result<Control<MDEvent>, Error> {
        if !self.active {
            return Ok(Control::Continue);
        }

        if let MDEvent::Event(event) = event {
            let mut focus = FocusBuilder::build_for(self);
            let f = focus.handle(event, Regular);
            ctx.queue(f);
        }

        let r = match event {
            MDEvent::Event(event) => {
                let mut r: Control<MDEvent> = match self.theme.handle(event, Popup) {
                    ChoiceOutcome::Value => {
                        let theme = dark_themes()
                            .iter()
                            .find(|v| v.name() == self.theme.value().as_str())
                            .cloned()
                            .expect("theme");

                        ctx.g.theme = Rc::new(theme);
                        Control::Changed
                    }
                    r => r.into(),
                };
                r = r.or_else(|| self.text_width.handle(event, Regular).into());
                r = r.or_else(|| self.globs.handle(event, Regular).into());

                r = r.or_else_try(|| {
                    match self
                        .ok_button
                        .handle(event, KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
                    {
                        ButtonOutcome::Pressed => self.save(ctx),
                        r => Ok(r.into()),
                    }
                })?;
                r = r.or_else_try(|| {
                    match self
                        .cancel_button
                        .handle(event, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                    {
                        ButtonOutcome::Pressed => self.cancel(ctx),
                        r => Ok(r.into()),
                    }
                })?;

                max(r, Control::Unchanged)
            }
            _ => Control::Continue,
        };

        Ok(r)
    }
}

impl ConfigDialogState {
    pub fn new() -> Self {
        Self::default()
    }

    fn cancel(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.active = false;

        let theme = dark_themes()
            .iter()
            .find(|v| v.name() == ctx.g.cfg.theme)
            .cloned()
            .expect("theme");
        ctx.g.theme = Rc::new(theme);

        Ok(Control::Changed)
    }

    fn save(&mut self, ctx: &mut AppContext<'_>) -> Result<Control<MDEvent>, Error> {
        self.active = false;

        let cfg = &mut ctx.g.cfg;
        cfg.theme = self.theme.value();
        cfg.text_width = self.text_width.value()?;
        cfg.globs = self
            .globs
            .value::<String>()
            .split([' ', ','])
            .filter_map(|v| {
                if !v.is_empty() {
                    Some(v.to_string())
                } else {
                    None
                }
            })
            .collect();

        ctx.queue(Control::Event(MDEvent::StoreConfig));
        Ok(Control::Changed)
    }

    pub fn show(&mut self, ctx: &AppContext<'_>) -> Result<(), Error> {
        self.active = true;

        let cfg = &ctx.g.cfg;
        self.theme.set_value(cfg.theme.clone());
        self.text_width.set_value(cfg.text_width)?;
        self.globs
            .set_value(cfg.globs.iter().fold(String::new(), |mut v, w| {
                if !v.is_empty() {
                    v.push_str(", ");
                }
                v.push_str(w);
                v
            }));

        let focus = FocusBuilder::build_for(self);
        focus.first();

        Ok(())
    }

    pub fn active(&self) -> bool {
        self.active
    }
}
