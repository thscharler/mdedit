use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for OxoCarbon

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "OxoCarbon" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

