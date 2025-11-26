use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Blackout

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Blackout" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Black, 0));
    }
}

