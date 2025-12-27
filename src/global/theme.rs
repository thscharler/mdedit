use rat_markdown::styles::MDStyle;
use rat_theme4::palette::{ColorIdx, Colors};
use rat_theme4::theme::SalsaTheme;
use rat_theme4::{create_salsa_theme, RatWidgetColor, StyleName, WidgetStyle};
use rat_widget::choice::ChoiceStyle;
use rat_widget::menu::MenuStyle;
use rat_widget::scrolled::{ScrollStyle, ScrollSymbols};
use rat_widget::text::TextStyle;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use std::collections::HashMap;

pub trait MDWidgets {
    const MENU_HIDDEN: &'static str = "md+menu-hidden";
    const CHOICE_TOOLS: &'static str = "md+choice-tools";
    const TEXT_DOCUMENT: &'static str = "md+text-document";
    const TEXT_STYLES: &'static str = "md+text-styles";
}
impl MDWidgets for WidgetStyle {}

pub trait MDStyles {
    const TEXT_BASE: &'static str = "md+text-base";
    const STATUS_HIDDEN: &'static str = "md+status-hidden";
}
impl MDStyles for Style {}

pub trait MDColor {
    const HIDDEN_FG: &'static str = "md+hidden";
}
impl MDColor for Color {}

mod base16;
mod black_white;
mod blackout;
mod everforest;
mod everforest_light;
mod imperial;
mod imperial_light;
mod material;
mod monekai;
mod monochrome;
mod nord;
mod ocean;
mod oxocarbon;
mod radium;
mod reds;
mod rust;
mod rust_light;
mod shell;
mod solarized;
mod sunrisebreeze_light;
mod tailwind;
mod tailwind_light;
mod tundra;
mod vscode;

pub fn create_mdedit_theme(name: &str) -> SalsaTheme {
    let mut theme = create_salsa_theme(name);

    for patch in [
        everforest::patch,
        everforest_light::patch,
        blackout::patch,
        black_white::patch,
        imperial::patch,
        material::patch,
        monekai::patch,
        base16::patch,
        imperial_light::patch,
        monochrome::patch,
        nord::patch,
        ocean::patch,
        oxocarbon::patch,
        radium::patch,
        reds::patch,
        rust::patch,
        rust_light::patch,
        shell::patch,
        solarized::patch,
        sunrisebreeze_light::patch,
        tailwind::patch,
        tailwind_light::patch,
        tundra::patch,
        vscode::patch,
    ] {
        patch(&mut theme.p);
    }
    if theme.p.try_aliased(Color::HIDDEN_FG).is_none() {
        theme
            .p
            .add_aliased(Color::HIDDEN_FG, ColorIdx(Colors::Gray, 1));
    }

    match theme.theme.as_str() {
        "Light" => {
            theme.define_style(Style::TEXT_BASE, theme.style_style(Style::DOCUMENT_BASE));
            theme.define_fn(WidgetStyle::TEXT_DOCUMENT, text_document);
            theme.define_fn(WidgetStyle::TEXT_STYLES, |th| text_style_light(th));

            theme.define_fn(WidgetStyle::MENU_HIDDEN, menu_hidden);
            theme.define_style(
                Style::STATUS_HIDDEN,
                theme
                    .p
                    .fg_bg_style_alias(Color::HIDDEN_FG, Color::STATUS_BASE_BG),
            );
            theme.define_fn(WidgetStyle::CHOICE_TOOLS, choice_tools);
        }
        "Dark" | "Shell" | _ => {
            theme.define_style(Style::TEXT_BASE, theme.style_style(Style::DOCUMENT_BASE));
            theme.define_fn(WidgetStyle::TEXT_DOCUMENT, text_document);
            theme.define_fn(WidgetStyle::TEXT_STYLES, |th| text_style(th));

            theme.define_fn(WidgetStyle::MENU_HIDDEN, menu_hidden);
            theme.define_style(
                Style::STATUS_HIDDEN,
                theme
                    .p
                    .fg_bg_style_alias(Color::HIDDEN_FG, Color::STATUS_BASE_BG),
            );
            theme.define_fn(WidgetStyle::CHOICE_TOOLS, choice_tools);
        }
    }

    theme.modify(WidgetStyle::SCROLL, |mut s: ScrollStyle, _| {
        s.horizontal = Some(ScrollSymbols {
            track: "─",
            thumb: "▄",
            begin: "▗",
            end: "▖",
            min: " ",
        });
        s.vertical = Some(ScrollSymbols {
            track: "│",
            thumb: "█",
            begin: "▄",
            end: "▀",
            min: " ",
        });
        s
    });
    theme.modify(WidgetStyle::SCROLL_DIALOG, |mut s: ScrollStyle, _| {
        s.horizontal = Some(ScrollSymbols {
            track: "─",
            thumb: "▄",
            begin: "▗",
            end: "▖",
            min: " ",
        });
        s.vertical = Some(ScrollSymbols {
            track: "│",
            thumb: "█",
            begin: "▄",
            end: "▀",
            min: " ",
        });
        s
    });
    theme.modify(WidgetStyle::SCROLL_POPUP, |mut s: ScrollStyle, _| {
        s.horizontal = Some(ScrollSymbols {
            track: "─",
            thumb: "▄",
            begin: "▗",
            end: "▖",
            min: " ",
        });
        s.vertical = Some(ScrollSymbols {
            track: "│",
            thumb: "█",
            begin: "▄",
            end: "▀",
            min: " ",
        });
        s
    });

    theme
}

fn choice_tools(th: &SalsaTheme) -> ChoiceStyle {
    ChoiceStyle {
        style: th.style_style(Style::CONTAINER_BASE),
        select: Some(th.style_style(Style::SELECT)),
        focus: Some(th.style_style(Style::FOCUS)),
        popup_style: Some(th.style(Style::CONTAINER_BASE)),
        popup_scroll: Some(th.style(WidgetStyle::SCROLL)),
        popup_block: Some(
            Block::bordered()
                .borders(Borders::LEFT)
                .border_style(th.style_style(Style::CONTAINER_BORDER_FG)),
        ),
        popup: Default::default(),
        ..Default::default()
    }
}

fn menu_hidden(th: &SalsaTheme) -> MenuStyle {
    let mut m = th.style::<MenuStyle>(WidgetStyle::MENU);
    m.style = m.style.fg(th.p.color_alias(Color::HIDDEN_FG));
    m
}

fn text_document(th: &SalsaTheme) -> TextStyle {
    TextStyle {
        style: th.style_style(Style::TEXT_BASE),
        scroll: Some(th.style(WidgetStyle::SCROLL)),
        border_style: Some(th.style(Style::CONTAINER_BORDER_FG)),
        focus: Some(th.style_style(Style::TEXT_BASE)),
        select: Some(th.style_style(Style::INPUT_SELECT)),
        ..Default::default()
    }
}

fn text_style(th: &SalsaTheme) -> HashMap<usize, Style> {
    let p = &th.p;

    let mut map = HashMap::new();

    //let base = sc.white[0];
    map.insert(
        MDStyle::Heading1.into(),
        p.fg_style(Colors::TextLight, 1).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading2.into(),
        p.fg_style(Colors::TextLight, 1).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading3.into(),
        p.fg_style(Colors::TextLight, 2).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading4.into(),
        p.fg_style(Colors::TextLight, 2).underlined(),
    );
    map.insert(
        MDStyle::Heading5.into(),
        p.fg_style(Colors::TextLight, 2).underlined(),
    );
    map.insert(
        MDStyle::Heading6.into(),
        p.fg_style(Colors::TextLight, 2).underlined(),
    );

    map.insert(MDStyle::Paragraph.into(), Style::new());
    map.insert(
        MDStyle::BlockQuote.into(),
        p.fg_style(Colors::Orange, 2).italic(),
    );
    map.insert(MDStyle::CodeBlock.into(), p.fg_style(Colors::RedPink, 2));
    map.insert(MDStyle::MathDisplay.into(), p.fg_style(Colors::RedPink, 2));
    map.insert(MDStyle::Rule.into(), p.fg_style(Colors::White, 2));
    map.insert(MDStyle::Html.into(), p.fg_style(Colors::Gray, 2));

    map.insert(
        MDStyle::Link.into(),
        p.fg_style(Colors::BlueGreen, 1).underlined(),
    );
    map.insert(MDStyle::LinkDef.into(), p.fg_style(Colors::BlueGreen, 1));
    map.insert(
        MDStyle::Image.into(),
        p.fg_style(Colors::BlueGreen, 1).underlined(),
    );
    map.insert(
        MDStyle::FootnoteDefinition.into(),
        p.fg_style(Colors::BlueGreen, 2),
    );
    map.insert(
        MDStyle::FootnoteReference.into(),
        p.fg_style(Colors::BlueGreen, 1).underlined(),
    );

    map.insert(MDStyle::List.into(), Style::new());
    map.insert(MDStyle::Item.into(), Style::new());
    map.insert(
        MDStyle::TaskListMarker.into(),
        p.fg_style(Colors::Orange, 1),
    );
    map.insert(MDStyle::ItemTag.into(), p.fg_style(Colors::Orange, 1));
    map.insert(MDStyle::DefinitionList.into(), Style::new());
    map.insert(
        MDStyle::DefinitionListTitle.into(),
        p.fg_style(Colors::Orange, 2),
    );
    map.insert(
        MDStyle::DefinitionListDefinition.into(),
        p.fg_style(Colors::Orange, 1),
    );

    map.insert(MDStyle::Table.into(), Style::new());
    map.insert(MDStyle::TableHead.into(), p.fg_style(Colors::Orange, 2));
    map.insert(MDStyle::TableRow.into(), Style::new());
    map.insert(MDStyle::TableCell.into(), Style::new());

    map.insert(MDStyle::Emphasis.into(), Style::new().italic());
    map.insert(MDStyle::Strong.into(), Style::new().bold());
    map.insert(MDStyle::Strikethrough.into(), Style::new().crossed_out());

    map.insert(MDStyle::CodeInline.into(), p.fg_style(Colors::RedPink, 1));
    map.insert(MDStyle::MathInline.into(), p.fg_style(Colors::RedPink, 1));
    map.insert(MDStyle::MetadataBlock.into(), p.fg_style(Colors::Orange, 1));

    map
}

fn text_style_light(th: &SalsaTheme) -> HashMap<usize, Style> {
    let p = &th.p;

    let mut map = HashMap::new();

    //let base = sc.white[0];
    map.insert(
        MDStyle::Heading1.into(),
        p.fg_style(Colors::TextDark, 1).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading2.into(),
        p.fg_style(Colors::TextDark, 1).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading3.into(),
        p.fg_style(Colors::TextDark, 2).underlined().bold(),
    );
    map.insert(
        MDStyle::Heading4.into(),
        p.fg_style(Colors::TextDark, 2).underlined(),
    );
    map.insert(
        MDStyle::Heading5.into(),
        p.fg_style(Colors::TextDark, 2).underlined(),
    );
    map.insert(
        MDStyle::Heading6.into(),
        p.fg_style(Colors::TextDark, 2).underlined(),
    );

    map.insert(MDStyle::Paragraph.into(), Style::new());
    map.insert(
        MDStyle::BlockQuote.into(),
        p.fg_style(Colors::Orange, 6).italic(),
    );
    map.insert(MDStyle::CodeBlock.into(), p.fg_style(Colors::RedPink, 6));
    map.insert(MDStyle::MathDisplay.into(), p.fg_style(Colors::RedPink, 6));
    map.insert(MDStyle::Rule.into(), p.fg_style(Colors::White, 6));
    map.insert(MDStyle::Html.into(), p.fg_style(Colors::Gray, 6));

    map.insert(
        MDStyle::Link.into(),
        p.fg_style(Colors::BlueGreen, 5).underlined(),
    );
    map.insert(MDStyle::LinkDef.into(), p.fg_style(Colors::BlueGreen, 5));
    map.insert(
        MDStyle::Image.into(),
        p.fg_style(Colors::BlueGreen, 5).underlined(),
    );
    map.insert(
        MDStyle::FootnoteDefinition.into(),
        p.fg_style(Colors::BlueGreen, 6),
    );
    map.insert(
        MDStyle::FootnoteReference.into(),
        p.fg_style(Colors::BlueGreen, 5).underlined(),
    );

    map.insert(MDStyle::List.into(), Style::new());
    map.insert(MDStyle::Item.into(), Style::new());
    map.insert(
        MDStyle::TaskListMarker.into(),
        p.fg_style(Colors::Orange, 5),
    );
    map.insert(MDStyle::ItemTag.into(), p.fg_style(Colors::Orange, 5));
    map.insert(MDStyle::DefinitionList.into(), Style::new());
    map.insert(
        MDStyle::DefinitionListTitle.into(),
        p.fg_style(Colors::Orange, 6),
    );
    map.insert(
        MDStyle::DefinitionListDefinition.into(),
        p.fg_style(Colors::Orange, 5),
    );

    map.insert(MDStyle::Table.into(), Style::new());
    map.insert(MDStyle::TableHead.into(), p.fg_style(Colors::Orange, 6));
    map.insert(MDStyle::TableRow.into(), Style::new());
    map.insert(MDStyle::TableCell.into(), Style::new());

    map.insert(MDStyle::Emphasis.into(), Style::new().italic());
    map.insert(MDStyle::Strong.into(), Style::new().bold());
    map.insert(MDStyle::Strikethrough.into(), Style::new().crossed_out());

    map.insert(MDStyle::CodeInline.into(), p.fg_style(Colors::RedPink, 5));
    map.insert(MDStyle::MathInline.into(), p.fg_style(Colors::RedPink, 5));
    map.insert(MDStyle::MetadataBlock.into(), p.fg_style(Colors::Orange, 5));

    map
}
