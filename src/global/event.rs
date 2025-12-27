use crate::fsys::FileSysStructure;
use crate::rat_salsa::event::{QuitEvent, RenderedEvent};
use crate::rat_salsa::timer::TimeOut;
use crossbeam::atomic::AtomicCell;
use ratatui::crossterm::event::Event;
use std::path::PathBuf;
use try_as::traits::TryAsRef;

/// Events
pub enum MDEvent {
    // crossterm
    Event(Event),
    // immediates are processed on the return path.
    Immediate(MDImmediate),
    // timer
    TimeOut(TimeOut),
    // just rendered
    Rendered,
    // will quit
    Quit,
    // msg-dialog
    Message(String),
    // status flags
    Info(String),
    //
    NoOp,

    // global actions
    MenuNew,
    MenuOpen,
    MenuSave,
    MenuSaveAs,
    MenuFormat,
    MenuFormatEq,
    CfgShowCtrl,
    CfgShowBreak,
    CfgShowLinenr,
    CfgWrapText,
    SyncEdit,
    SyncFileList,
    New(PathBuf),
    Open(PathBuf),
    SelectOrOpen(PathBuf),
    SelectOrOpenSplit(PathBuf),
    SaveAs(PathBuf),
    FileSysChanged(Box<AtomicCell<FileSysStructure>>),
    FileSysReloaded(Box<AtomicCell<FileSysStructure>>),
    Save,
    Split,
    JumpToFileSplit,
    JumpToTree,
    JumpToFiles,
    JumpToTabs,
    JumpToEditSplit,
    PrevEditSplit,
    NextEditSplit,
    HideFiles,
    Close,
    CloseAll,
    CloseAt(usize, usize),
    SelectAt(usize, usize),
    StoreConfig,
}

/// Immediates are events that are checked on the return path
/// of event-handling. They operate similar to Outcome-types for
/// regular widgets.
#[derive(Debug)]
pub enum MDImmediate {
    /// tab has been closed.
    TabClosed,
}

impl From<RenderedEvent> for MDEvent {
    fn from(_: RenderedEvent) -> Self {
        Self::Rendered
    }
}

impl From<QuitEvent> for MDEvent {
    fn from(_: QuitEvent) -> Self {
        Self::Quit
    }
}

impl TryAsRef<Event> for MDEvent {
    fn try_as_ref(&self) -> Option<&Event> {
        match self {
            MDEvent::Event(e) => Some(e),
            _ => None,
        }
    }
}

impl From<Event> for MDEvent {
    fn from(value: Event) -> Self {
        Self::Event(value)
    }
}

impl<'a> TryFrom<&'a MDEvent> for &'a Event {
    type Error = ();

    fn try_from(value: &'a MDEvent) -> Result<Self, Self::Error> {
        match value {
            MDEvent::Event(event) => Ok(event),
            _ => Err(()),
        }
    }
}

impl From<TimeOut> for MDEvent {
    fn from(value: TimeOut) -> Self {
        Self::TimeOut(value)
    }
}
