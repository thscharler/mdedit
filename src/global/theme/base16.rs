use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Base16

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Base16" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

