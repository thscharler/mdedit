use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Shell

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Shell" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::TextDark, 2));
    }
}

