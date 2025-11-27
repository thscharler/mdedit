use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Radium
/// An adaption of nvchad's radium theme.
/// -- credits to original radium theme from <https://github.com/dharmx>

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Radium" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

