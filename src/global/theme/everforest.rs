use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for EverForest

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "EverForest" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

