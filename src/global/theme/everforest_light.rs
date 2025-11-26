use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for EverForest Light

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "EverForest Light" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::BlueGreen, 3));
    }
}

