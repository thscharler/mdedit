use rat_theme4::palette::{ColorIdx, Colors, Palette};

/// Patch for Rust
/// Rusty theme.

pub fn patch(pal: &mut Palette) {
    if pal.name.as_ref() == "Rust" {
        pal.add_aliased("md+hidden", ColorIdx(Colors::Gray, 0));
    }
}

