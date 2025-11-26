use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Black&White

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Black&White" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 3));
    }
}

