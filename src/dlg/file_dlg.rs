use crate::global::event::MDEvent;
use crate::global::GlobalState;
use anyhow::Error;
use rat_dialog::StackControl;
use rat_salsa::SalsaContext;
use rat_widget::event::{Dialog, FileOutcome, HandleEvent, Outcome};
use rat_widget::file_dialog::{FileDialog, FileDialogState};
use rat_widget::layout::layout_middle;
use rat_widget::text::HasScreenCursor;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::StatefulWidget;
use std::any::Any;

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut (dyn Any + 'static),
    ctx: &mut GlobalState,
) -> Result<(), Error> {
    let state = state.downcast_mut().expect("dialog-state");

    let area = layout_middle(
        area,
        Constraint::Percentage(19),
        Constraint::Percentage(19),
        Constraint::Length(2),
        Constraint::Length(2),
    );

    FileDialog::new()
        .styles(ctx.theme.file_dialog_style())
        .render(area, buf, state);

    ctx.set_screen_cursor(state.screen_cursor());

    Ok(())
}

pub fn event_new(
    event: &MDEvent,
    state: &mut dyn Any,
    ctx: &mut GlobalState,
) -> Result<StackControl<MDEvent>, Error> {
    let state = state
        .downcast_mut::<FileDialogState>()
        .expect("dialog-state");
    match event {
        MDEvent::Event(event) => match state.handle(event, Dialog)? {
            FileOutcome::Cancel => Ok(StackControl::Pop),
            FileOutcome::Ok(p) => {
                ctx.queue_event(MDEvent::New(p));
                Ok(StackControl::Pop)
            }
            r => Ok(Outcome::from(r).into()),
        },
        _ => Ok(StackControl::Continue),
    }
}

pub fn event_open(
    event: &MDEvent,
    state: &mut dyn Any,
    ctx: &mut GlobalState,
) -> Result<StackControl<MDEvent>, Error> {
    let state = state
        .downcast_mut::<FileDialogState>()
        .expect("dialog-state");
    match event {
        MDEvent::Event(event) => match state.handle(event, Dialog)? {
            FileOutcome::Cancel => Ok(StackControl::Pop),
            FileOutcome::Ok(p) => {
                ctx.queue_event(MDEvent::Open(p));
                Ok(StackControl::Pop)
            }
            r => Ok(Outcome::from(r).into()),
        },
        _ => Ok(StackControl::Continue),
    }
}

pub fn event_save_as(
    event: &MDEvent,
    state: &mut dyn Any,
    ctx: &mut GlobalState,
) -> Result<StackControl<MDEvent>, Error> {
    let state = state
        .downcast_mut::<FileDialogState>()
        .expect("dialog-state");
    match event {
        MDEvent::Event(event) => match state.handle(event, Dialog)? {
            FileOutcome::Cancel => Ok(StackControl::Pop),
            FileOutcome::Ok(p) => {
                ctx.queue_event(MDEvent::SaveAs(p));
                Ok(StackControl::Pop)
            }
            r => Ok(Outcome::from(r).into()),
        },
        _ => Ok(StackControl::Continue),
    }
}
