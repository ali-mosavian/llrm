//! Backend regressions that need Nib source to reach their shape.

fn _procedure(listing: &str, name: &str) -> String {
    listing[listing.find(&format!("{name} proc")).unwrap()..listing.find(&format!("{name} endp")).unwrap()].to_owned()
}

fn _nib(fixture: &str, name: &str) -> String {
    use crate::test_nib_frontend as nib;

    let program = nib::parsed(&nib::fixture(&format!("{fixture}.nib")));
    _procedure(&nib::listing(&program, "main", &llrm_core::model::passes::O2()), name)
}

#[test]
fn test_a_dword_read_only_as_offset_and_selector_is_one_far_load() {
    // Nib's sum split each far pointer through the stack.
    let function = _nib("sum", "_sum");
    assert_eq!(function.matches("les ").count(), 2);
    assert!(!function.contains("pop es"));
}
