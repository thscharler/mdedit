use rat_salsa::rendered::RenderedEvent;
use rat_salsa::timer::TimeOut;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum MDEvent {
    Event(crossterm::event::Event),
    TimeOut(TimeOut),
    Rendered,
    Message(String),
    Status(usize, String),

    MenuNew,
    MenuOpen,
    MenuSave,
    MenuSaveAs,

    CfgShowCtrl,
    CfgNewline,

    SyncEdit,

    New(PathBuf),
    Open(PathBuf),
    SelectOrOpen(PathBuf),
    SelectOrOpenSplit(PathBuf),
    SaveAs(PathBuf),
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
    CloseAt(usize, usize),
    SelectAt(usize, usize),
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
