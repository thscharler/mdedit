use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Tundra
/// An adaption of nvchad's tundra theme.
/// -- Thanks to original theme for existing <https://github.com/sam4llis/nvim-tundra>

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Tundra" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

