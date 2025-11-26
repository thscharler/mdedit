use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Solarized
/// credit https://github.com/altercation/solarized/tree/master/vim-colors-solarized

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Solarized" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Black, 4));
    }
}

