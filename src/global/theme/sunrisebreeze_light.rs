use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for SunriseBreeze Light

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "SunriseBreeze Light" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 2));
    }
}

