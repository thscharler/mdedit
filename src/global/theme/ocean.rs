use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Ocean
/// My take on an ocean theme.

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Ocean" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

