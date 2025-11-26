use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Nord
/// Credits to original https://github.com/arcticicestudio/nord-vim
/// 

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Nord" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 7));
    }
}

