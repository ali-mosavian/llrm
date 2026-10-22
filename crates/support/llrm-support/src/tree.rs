//! The build tree a binary reads at run time. A dist binary names it by the
//! path it was compiled in; a removed tree failed every compile deep inside
//! the front end (E1055 on borland.h) without saying the dist was the cause.

use std::path::Path;

/// The message for the first of `needs` (what it is, where) that is gone.
pub fn missing(
    name: &str,
    needs: &[(&str, &Path)],
) -> Option<String> {
    let (what, path) = needs.iter().find(|(_, path)| !path.exists())?;
    Some(format!(
        "{name}: the {what} {} is gone; this binary reads it from the tree it was built in. Rebuild it or restore the tree.",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binary_whose_tree_is_gone_is_refused_by_name() {
        let gone = Path::new("/nonexistent/wt-pair-abc/crates/frontends/llrm-c/src/borland.h");
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let message = missing("llrm-c", &[("tree", here), ("C header", gone)]).expect("refused");
        assert!(message.contains("llrm-c") && message.contains("borland.h") && message.contains("gone"), "{message}");
        assert_eq!(missing("llrm-c", &[("tree", here)]), None);
    }
}
