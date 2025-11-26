use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Material
/// Credits to original theme https://github.com/marko-cerovac/material.nvim for existing
/// -

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Material" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

