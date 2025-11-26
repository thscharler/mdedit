use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for VSCode

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "VSCode" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

