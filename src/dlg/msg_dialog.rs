use crate::global::event::MDEvent;
use crate::global::GlobalState;
use anyhow::Error;
use rat_dialog::DialogControl;
use rat_widget::event::{Dialog, HandleEvent, Outcome};
use rat_widget::layout::layout_middle;
use rat_widget::msgdialog::{MsgDialog, MsgDialogState};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Padding, StatefulWidget};
use std::any::Any;

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut (dyn Any + 'static),
    ctx: &mut GlobalState,
) {
    let state = state.downcast_mut().expect("msgdialog-state");

    let area = layout_middle(
        area,
        Constraint::Percentage(19),
        Constraint::Percentage(19),
        Constraint::Length(2),
        Constraint::Length(2),
    );

    MsgDialog::new()
        .styles(ctx.theme.msg_dialog_style())
        .render(area, buf, state);
}

pub fn render_info(
    area: Rect,
    buf: &mut Buffer,
    state: &mut (dyn Any + 'static),
    ctx: &mut GlobalState,
) {
    let state = state
        .downcast_mut::<MsgDialogState>()
        .expect("dialog-state");

    MsgDialog::new()
        .block(
            Block::bordered()
                .style(
                    Style::default() //
                        .fg(ctx.theme.palette().white[2])
                        .bg(ctx.theme.palette().deepblue[0]),
                )
                .border_type(BorderType::Rounded)
                .title_style(Style::new().fg(ctx.scheme().bluegreen[0]))
                .padding(Padding::new(1, 1, 1, 1)),
        )
        .styles(ctx.theme.msg_dialog_style())
        .render(area, buf, state);
}

pub fn event(
    event: &MDEvent,
    state: &mut dyn Any,
    _ctx: &mut GlobalState,
) -> Result<DialogControl<MDEvent>, Error> {
    let r = if let MDEvent::Event(event) = event {
        let state = state
            .downcast_mut::<MsgDialogState>()
            .expect("msgdialog-state");

        match state.handle(event, Dialog) {
            Outcome::Changed => {
                if !state.active() {
                    DialogControl::Close(None)
                } else {
                    DialogControl::Changed
                }
            }
            r => r.into(),
        }
    } else {
        DialogControl::Continue
    };
    Ok(r)
}
