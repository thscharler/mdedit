use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Imperial
/// Uses purple and gold for primary/secondary.
/// Other colors are bright, strong and slightly smudged.

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Imperial" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

