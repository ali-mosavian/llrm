//! Backend regressions that need Nib source to reach their shape.

fn _procedure(listing: &str, name: &str) -> String {
    listing[listing.find(&format!("{name} proc")).unwrap()..listing.find(&format!("{name} endp")).unwrap()].to_owned()
}

/// `name`'s listing in `fixture` compiled with `entry` as its entry, so the
/// whole program does not fold into it.
fn _nib(fixture: &str, entry: &str, name: &str) -> String {
    use crate::test_nib_frontend as nib;

    let program = nib::parsed(&nib::fixture(&format!("{fixture}.nib")));
    _procedure(&nib::listing(&program, entry, &crate::test_nib_frontend::O2()), name)
}

#[test]
fn test_a_dword_read_only_as_offset_and_selector_is_one_far_load() {
    // Nib's sum split each far pointer through the stack.
    let function = _nib("sum", "sum", "_sum");
    assert_eq!(function.matches("les ").count(), 2);
    assert!(!function.contains("pop es"));
}

fn _stack_of(fixture: &str) -> Result<(String, Vec<u8>), String> {
    use crate::test_nib_frontend as nib;

    let program = nib::parsed(&nib::fixture(&format!("{fixture}.nib")));
    let options = nib::O2();
    let module = crate::compile::assembled(&program, "main", &options)?;
    let object = crate::compile::object(&module, &nib::fixture(&format!("{fixture}.nib")), llrm_core::backend::omfwrite::CodeLayout::OneSegment)?;
    Ok((llrm_core::backend::masm::text(&module).expect("prints"), object))
}

/// #356: an 8 KB local ran past the 4 KB stack `start.asm` links, and the
/// program jumped to garbage. The object adds what the deepest chain needs.
#[test]
fn test_a_frame_larger_than_the_start_stack_adds_a_stack_segment() {
    let (listing, object) = _stack_of("bigframe").unwrap();
    assert!(listing.contains(".stack 4618"), "{listing}");
    assert!(object.windows(5).any(|one| one == b"STACK"));
}

/// The stack `start.asm` links already holds a small program's chain.
#[test]
fn test_a_small_program_adds_no_stack() {
    let (listing, object) = _stack_of("sum").unwrap();
    assert!(!listing.contains(".stack"));
    assert!(!object.windows(5).any(|one| one == b"STACK"));
}

/// #356: a frame no stack segment can hold is an error, not a silent crash.
#[test]
fn test_a_frame_no_stack_segment_holds_is_refused() {
    let error = _stack_of("hugeframe").unwrap_err();
    assert!(error.contains("bytes of stack"), "{error}");
}

/// `STACK_BASE` is the stack `start.asm` links: a different one would size the object's wrongly.
#[test]
fn test_the_stack_base_is_the_one_start_links() {
    let start = std::fs::read_to_string(crate::test_nib_frontend::root().join("crates/frontends/llrm-nib/src/runtime/start.asm")).unwrap();
    assert!(start.contains(&format!(".stack {}", crate::compile::STACK_BASE)));
}
