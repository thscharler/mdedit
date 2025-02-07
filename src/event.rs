use crate::fs_structure::FileSysStructure;
use crossbeam::atomic::AtomicCell;
use rat_salsa::rendered::RenderedEvent;
use rat_salsa::timer::TimeOut;
use std::path::PathBuf;

/// Events
pub enum MDEvent {
    // crossterm
    Event(crossterm::event::Event),
    // immediates are processed on the return path.
    Immediate(MDImmediate),
    // timer
    TimeOut(TimeOut),
    // just rendered
    Rendered,
    // msg-dialog
    Message(String),
    // status flags
    Status(usize, String),

    // global actions
    MenuNew,
    MenuOpen,
    MenuSave,
    MenuSaveAs,
    MenuFormat,
    MenuFormatEq,
    CfgShowCtrl,
    SyncEdit,
    New(PathBuf),
    Open(PathBuf),
    SelectOrOpen(PathBuf),
    SelectOrOpenSplit(PathBuf),
    SaveAs(PathBuf),
    FileSys(Box<AtomicCell<FileSysStructure>>),
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

impl From<crossterm::event::Event> for MDEvent {
    fn from(value: crossterm::event::Event) -> Self {
        Self::Event(value)
    }
}

impl From<TimeOut> for MDEvent {
    fn from(value: TimeOut) -> Self {
        Self::TimeOut(value)
    }
}
