//! The WebAssembly target. Its descriptions are the files in `machines/`.

pub mod dagisel;

use llrm_target::layout::Layout;

/// The pointer width of a module: `wasm32` or `wasm64`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Width {
    W32,
    W64,
}

/// The data layout of a module of `width`.
pub fn layout(width: Width) -> Layout {
    let text = match width {
        Width::W32 => include_str!("machines/datalayout32.toml"),
        Width::W64 => include_str!("machines/datalayout64.toml"),
    };
    Layout::parse(text).expect("the wasm datalayout description parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--target=wasm32|wasm64` of the pin's clang (d1106deb71cc).
    #[test]
    fn the_datalayout_is_the_pin_clangs() {
        assert_eq!(
            layout(Width::W32).datalayout,
            "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
        );
        assert_eq!(
            layout(Width::W64).datalayout,
            "e-m:e-p:64:64-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
        );
    }

    #[test]
    fn a_pointer_is_one_word_in_the_one_address_space() {
        assert_eq!(layout(Width::W32).spaces.unmarked(4), Ok(0));
        assert_eq!(layout(Width::W64).spaces.unmarked(8), Ok(0));
        assert!(layout(Width::W32).spaces.unmarked(8).is_err());
    }
}
