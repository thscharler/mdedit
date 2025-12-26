use crate::global::event::MDEvent;
use crate::global::theme::create_mdedit_theme;
use crate::global::GlobalState;
use crate::rat_salsa::SalsaContext;
use anyhow::Error;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rat_dialog::WindowControl;
#[cfg(feature = "wgpu")]
use rat_salsa_wgpu::font_data::FontData;
use rat_theme4::{salsa_themes, StyleName, WidgetStyle};
use rat_widget::button::{Button, ButtonState};
use rat_widget::choice::{Choice, ChoiceState};
#[cfg(feature = "wgpu")]
use rat_widget::event::SliderOutcome;
use rat_widget::event::{try_flow, ButtonOutcome, ChoiceOutcome, HandleEvent, Popup, Regular};
use rat_widget::focus::{FocusBuilder, FocusFlag, HasFocus};
use rat_widget::form::{Form, FormState};
use rat_widget::layout::{layout_middle, FormLabel, FormWidget, LayoutForm};
use rat_widget::number_input::{NumberInput, NumberInputState};
#[cfg(feature = "wgpu")]
use rat_widget::slider::{Slider, SliderState};
use rat_widget::text::HasScreenCursor;
use rat_widget::text_input::{TextInput, TextInputState};
use rat_widget::util::reset_buf_area;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Padding, StatefulWidget, Widget};
use std::any::Any;

#[derive(Debug, Default)]
pub struct ConfigDialogState {
    form: FormState<usize>,
    theme: ChoiceState<String>,
    text_width: NumberInputState,
    globs: TextInputState,
    #[cfg(feature = "wgpu")]
    font: ChoiceState<String>,
    #[cfg(feature = "wgpu")]
    font_size: SliderState<f64>,

    ok_button: ButtonState,
    cancel_button: ButtonState,
}

pub fn render(area: Rect, buf: &mut Buffer, state: &mut dyn Any, ctx: &mut GlobalState) {
    let state = state.downcast_mut::<ConfigDialogState>().expect("state");

    let cfg_area = layout_middle(
        area,
        Constraint::Percentage(19),
        Constraint::Percentage(19),
        Constraint::Percentage(19),
        Constraint::Percentage(19),
    );

    let block = Block::bordered()
        .style(ctx.theme.style_style(Style::DIALOG_BASE))
        .border_style(ctx.theme.style_style(Style::DIALOG_BORDER_FG));
    let inner = block.inner(cfg_area);

    let l = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    reset_buf_area(cfg_area, buf);
    block.render(cfg_area, buf);

    let mut form = Form::new() //
        .show_navigation(false)
        .style(ctx.theme.style_style(Style::DIALOG_BASE));

    let layout_size = form.layout_size(l[0]);
    if !state.form.valid_layout(layout_size) {
        let mut layout = LayoutForm::new()
            .padding(Padding::new(1, 1, 1, 1))
            .spacing(1)
            .line_spacing(1)
            .flex(Flex::Legacy);

        layout.widget(
            state.theme.id(),
            FormLabel::Str("Theme"),
            FormWidget::Width(25),
        );
        layout.widget(
            state.text_width.id(),
            FormLabel::Str("Text break at"),
            FormWidget::Width(15),
        );
        layout.widget(
            state.globs.id(),
            FormLabel::Str("Files glob"),
            FormWidget::Width(35),
        );
        #[cfg(feature = "wgpu")]
        {
            layout.widget(
                state.font.id(),
                FormLabel::Str("Font"),
                FormWidget::Width(25),
            );
            layout.widget(
                state.font_size.id(),
                FormLabel::Str("Size"),
                FormWidget::Width(10),
            );
        }
        form = form.layout(layout.build_endless(layout_size.width));
    }
    let mut form = form.into_buffer(l[0], buf, &mut state.form);

    let theme_popup = form.render2(
        state.theme.id(),
        || {
            Choice::new()
                .styles(ctx.theme.style(WidgetStyle::CHOICE))
                .items(
                    salsa_themes()
                        .iter()
                        .map(|v| (v.to_string(), v.to_string())),
                )
                .into_widgets()
        },
        &mut state.theme,
    );
    form.render(
        state.text_width.id(),
        || NumberInput::new().styles(ctx.theme.style(WidgetStyle::TEXT)),
        &mut state.text_width,
    );
    form.render(
        state.globs.id(),
        || TextInput::new().styles(ctx.theme.style(WidgetStyle::TEXT)),
        &mut state.globs,
    );
    #[cfg(feature = "wgpu")]
    let font_popup = {
        let font_popup = form.render2(
            state.font.id(),
            || {
                Choice::new()
                    .items(
                        FontData
                            .installed_fonts()
                            .iter()
                            .map(|v| (v.clone(), v.clone())),
                    )
                    .styles(ctx.theme.style(WidgetStyle::CHOICE))
                    .popup_boundary(l[0])
                    .into_widgets()
            },
            &mut state.font,
        );
        let fs = state.font_size.value();
        form.render(
            state.font_size.id(),
            || {
                Slider::<f64>::new()
                    .range((10.0, 40.0))
                    .step(1.0)
                    .long_step(5.0)
                    .upper_bound(format!(" {}", fs))
                    .styles(ctx.theme.style(WidgetStyle::SLIDER))
            },
            &mut state.font_size,
        );

        font_popup
    };

    form.render_popup(state.theme.id(), || theme_popup, &mut state.theme);
    #[cfg(feature = "wgpu")]
    form.render_popup(state.font.id(), || font_popup, &mut state.font);

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
        .styles(ctx.theme.style(WidgetStyle::BUTTON))
        .render(l2[0], buf, &mut state.cancel_button);
    Button::new("Ok")
        .styles(ctx.theme.style(WidgetStyle::BUTTON)) //
        .render(l2[1], buf, &mut state.ok_button);
}

impl HasFocus for ConfigDialogState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.widget(&self.theme);
        builder.widget(&self.text_width);
        builder.widget(&self.globs);
        #[cfg(feature = "wgpu")]
        {
            builder.widget(&self.font);
            builder.widget(&self.font_size);
        }
        builder.widget(&self.ok_button);
        builder.widget(&self.cancel_button);
    }

    fn focus(&self) -> FocusFlag {
        unimplemented!("not defined")
    }

    fn area(&self) -> ratatui::layout::Rect {
        unimplemented!("not defined")
    }
}

pub fn event(
    event: &MDEvent,
    state: &mut dyn Any,
    ctx: &mut GlobalState,
) -> Result<WindowControl<MDEvent>, Error> {
    let state = state.downcast_mut::<ConfigDialogState>().expect("state");

    if let MDEvent::Event(event) = event {
        let mut focus = FocusBuilder::build_for(state);
        let f = focus.handle(event, Regular);
        ctx.queue(f);
    }

    match event {
        MDEvent::Event(event) => {
            try_flow!(match state.theme.handle(event, Popup) {
                ChoiceOutcome::Value => {
                    let theme = salsa_themes()
                        .iter()
                        .find(|v| **v == state.theme.value().as_str())
                        .cloned()
                        .expect("theme");

                    ctx.theme = create_mdedit_theme(theme);
                    WindowControl::Changed
                }
                r => r.into(),
            });
            try_flow!(state.text_width.handle(event, Regular));
            #[cfg(feature = "wgpu")]
            {
                try_flow!(match state.font.handle(event, Popup) {
                    ChoiceOutcome::Value => {
                        ctx.set_font_family(&state.font.value());
                        WindowControl::Changed
                    }
                    r => r.into(),
                });
                try_flow!(match state.font_size.handle(event, Regular) {
                    SliderOutcome::Value => {
                        ctx.set_font_size(state.font_size.value());
                        WindowControl::Changed
                    }
                    r => r.into(),
                });
            }
            try_flow!(state.globs.handle(event, Regular));

            try_flow!(match state
                .ok_button
                .handle(event, KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
            {
                ButtonOutcome::Pressed => state.save(ctx)?,
                r => r.into(),
            });
            try_flow!(match state
                .cancel_button
                .handle(event, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            {
                ButtonOutcome::Pressed => state.cancel(ctx)?,
                r => r.into(),
            });

            Ok(WindowControl::Unchanged)
        }
        _ => Ok(WindowControl::Continue),
    }
}

impl ConfigDialogState {
    pub fn new(ctx: &mut GlobalState) -> Result<Self, Error> {
        let mut s = Self::default();
        s.text_width.set_format_loc("###0", ctx.cfg.loc)?;

        let cfg = &ctx.cfg;
        s.theme.set_value(cfg.theme.clone());
        s.text_width.set_value(cfg.text_width)?;
        #[cfg(feature = "wgpu")]
        {
            s.font_size.set_value(ctx.font_size());
            s.font.set_value(cfg.font.clone());
        }
        s.globs
            .set_value(cfg.globs.iter().fold(String::new(), |mut v, w| {
                if !v.is_empty() {
                    v.push_str(", ");
                }
                v.push_str(w);
                v
            }));

        let focus = FocusBuilder::build_for(&s);
        focus.first();

        Ok(s)
    }

    fn cancel(&mut self, ctx: &mut GlobalState) -> Result<WindowControl<MDEvent>, Error> {
        let theme = salsa_themes()
            .iter()
            .find(|v| **v == ctx.cfg.theme)
            .cloned()
            .expect("theme");
        ctx.theme = create_mdedit_theme(theme);

        Ok(WindowControl::Close(MDEvent::NoOp))
    }

    fn save(&mut self, ctx: &mut GlobalState) -> Result<WindowControl<MDEvent>, Error> {
        let cfg = &mut ctx.cfg;
        cfg.theme = self.theme.value();
        cfg.text_width = self.text_width.value()?;
        #[cfg(feature = "wgpu")]
        {
            cfg.font = self.font.value();
            cfg.font_size = self.font_size.value();
        }
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

        ctx.queue_event(MDEvent::StoreConfig);
        Ok(WindowControl::Close(MDEvent::NoOp))
    }
}
