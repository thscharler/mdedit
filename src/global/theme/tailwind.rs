use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Tailwind

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Tailwind" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 3));
    }
}

