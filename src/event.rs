use crate::fs_structure::FileSysStructure;
use crossbeam::atomic::AtomicCell;
use rat_salsa::rendered::RenderedEvent;
use rat_salsa::timer::TimeOut;
use std::path::PathBuf;

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
