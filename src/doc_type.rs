use rat_markdown::dump::md_dump;
use rat_markdown::op::md_format;
use rat_markdown::styles::parse_md_styles;
use rat_widget::event::TextOutcome;
use rat_widget::textarea::TextAreaState;

/// Do some doc-type variation of the editors behaviour.
pub trait DocType {
    /// Parse document.
    fn parse(&self, txt: &mut TextAreaState);

    /// Format document
    fn format(&self, txt: &mut TextAreaState, width: u16, table_eq_width: bool) -> TextOutcome;

    /// Dump parser debug info to log.
    fn log_parser(&self, txt: &TextAreaState);
}

#[derive(Debug)]
pub enum DocTypes {
    MD,
    TXT,
}

impl DocType for DocTypes {
    #[inline]
    fn parse(&self, txt: &mut TextAreaState) {
        match self {
            DocTypes::MD => DocTypeMD.parse(txt),
            DocTypes::TXT => DocTypeTXT.parse(txt),
        }
    }

    #[inline]
    fn format(&self, txt: &mut TextAreaState, width: u16, table_eq_width: bool) -> TextOutcome {
        match self {
            DocTypes::MD => DocTypeMD.format(txt, width, table_eq_width),
            DocTypes::TXT => DocTypeTXT.format(txt, width, table_eq_width),
        }
    }

    #[inline]
    fn log_parser(&self, txt: &TextAreaState) {
        match self {
            DocTypes::MD => DocTypeMD.log_parser(txt),
            DocTypes::TXT => DocTypeTXT.log_parser(txt),
        }
    }
}

struct DocTypeMD;

impl DocType for DocTypeMD {
    fn parse(&self, txt: &mut TextAreaState) {
        let styles = parse_md_styles(&txt.text());
        txt.set_styles(styles);
    }

    fn format(&self, txt: &mut TextAreaState, width: u16, table_eq_width: bool) -> TextOutcome {
        md_format(txt, width as usize, table_eq_width)
    }

    fn log_parser(&self, txt: &TextAreaState) {
        md_dump(txt);
    }
}

struct DocTypeTXT;

impl DocType for DocTypeTXT {
    fn parse(&self, _: &mut TextAreaState) {
        // noop
    }

    fn format(&self, _: &mut TextAreaState, _: u16, _: bool) -> TextOutcome {
        // noop
        TextOutcome::Continue
    }

    fn log_parser(&self, _: &TextAreaState) {
        // noop
    }
}
