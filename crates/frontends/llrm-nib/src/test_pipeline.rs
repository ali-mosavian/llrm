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
    let module = crate::compile::assembled(&program, "main", &options, &crate::Frontend::default().os)?;
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
    let start = std::fs::read_to_string(crate::test_nib_frontend::root().join("crates/target/llrm-x86-code16/runtime/nib/start.asm")).unwrap();
    assert!(start.contains(&format!(".stack {}", crate::Frontend::default().os.stack_base)));
}

/// The listing's innermost loops, each as the lines from the label a later
/// jump goes back to through that jump, any with no call in them.
fn _innermost_loops(function: &str) -> Vec<Vec<&str>> {
    let lines: Vec<&str> = function.lines().collect();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(target) = line.split_whitespace().last().filter(|_| line.trim_start().starts_with('j')) else { continue };
        let Some(head) = lines[..at].iter().position(|one| one.trim_end() == format!("{target}:")) else { continue };
        if lines[head..=at].iter().all(|one| !one.contains("call")) {
            found.push(lines[head..=at].to_vec());
        }
    }
    found
}

/// Nib's `a[hi]` check ahead of `for j in lo..hi` bounds every index in it:
/// quicksort's partition tested `j` and `i` against the length on every
/// trip (3 compare-and-branch pairs against C's none), 245,433 executed
/// instructions against C's 163,105 (#453).
#[test]
fn test_a_partition_loop_has_no_bounds_check_in_it() {
    let function = _nib("partition", "partition", "_partition");
    let loops = _innermost_loops(&function);
    assert!(!loops.is_empty(), "premise: the loop is found\n{function}");
    for body in loops {
        let checks: Vec<_> = body.iter().filter(|one| one.trim_start().starts_with("jae ") || one.trim_start().starts_with("jb ")).collect();
        assert!(checks.is_empty(), "{checks:?} in {body:#?}");
    }
}

/// `-fsanitize=stack` compares with the word and calls the routine `runtime/stack.toml` names, and
/// both exist in the runtime: a description naming a symbol start-up never fills would compare with zero.
#[test]
fn test_the_stack_check_names_what_the_nib_runtime_defines() {
    use crate::test_nib_frontend as nib;

    let check = crate::Frontend::default().os.stack.clone();
    let runtime = |name: &str| {
        let directory = if name.ends_with(".asm") { format!("{}/../../target/llrm-x86-code16/runtime/nib", env!("CARGO_MANIFEST_DIR")) } else { format!("{}/src/runtime", env!("CARGO_MANIFEST_DIR")) };
        std::fs::read_to_string(format!("{directory}/{name}")).unwrap()
    };
    assert!(runtime("dos.asm").contains(&format!("public {}", check.limit)) && runtime("start.asm").contains(&format!("mov {}, ax", check.limit)));
    assert!(runtime("errors.nib").contains(&format!("@export(name=\"{}\")", check.handler)));
    // The limit sits the reserve `stack_to_add` leaves above the stack's bottom.
    assert!(runtime("start.asm").contains(&format!("add ax, {}", llrm_core::backend::stackusage::STACK_RESERVE)));
    let mut program = nib::parsed(&nib::fixture("sum.nib"));
    program.stack_check = Some(llrm_core::hir::model::StackCheck { limit: "FOO".into(), handler: "BAR".into(), ..check });
    let sum = _procedure(&nib::listing(&program, "sum", &nib::O2()), "_sum");
    assert!(sum.contains("cmp sp, word ptr FOO") && sum.contains("call far ptr BAR") && !sum.contains("N$OSLO"), "{sum}");
    let plain = _procedure(&nib::listing(&nib::parsed(&nib::fixture("sum.nib")), "sum", &nib::O2()), "_sum");
    assert!(!plain.contains("cmp sp"), "{plain}");
}

/// examples/loader.nib at -Os: a loop whose entry loaded what it reads was admitted on a tie in the bytes the spiller
/// counts (its trips were fewer), and the object grew by 15 bytes (2517 to 2532). The encoded code decides a tie.
#[test]
fn test_a_loop_admitted_on_a_tie_in_counted_bytes_does_not_grow_the_object() {
    use crate::test_nib_frontend as nib;

    let source = nib::root().join("examples/loader.nib");
    let program = nib::parsed(&source);
    let options = nib::level("Os");
    let module = crate::compile::assembled(&program, "main", &options, &crate::Frontend::default().os).expect("assembles");
    let object = crate::compile::object(&module, &source, llrm_core::backend::omfwrite::CodeLayout::OneSegment).expect("an object").len();
    // 2517 bytes before the loop was admitted on the tie; 2532 with it.
    assert!(object <= 2517, "{object} bytes");
}

/// A flat target's program naming `cdecl16` compiled as if it were real mode's: the frame it
/// described had 2-byte slots. A target's conventions are its own, so each is refused on the other.
#[test]
fn test_a_convention_the_target_does_not_define_is_refused() {
    let directory = tempfile::tempdir().expect("a directory");
    let write = |name: &str, convention: &str| {
        let path = directory.path().join(name);
        std::fs::write(&path, format!("@extern(\"{convention}\", name=\"f\")\nfn f(a: i16) -> i16\n\nfn main() -> i16:\n    unsafe:\n        return f(1)\n")).expect("written");
        path
    };
    let flat = crate::Frontend { conventions: vec!["cdecl32".into()], ..Default::default() };
    let refused = |frontend: &crate::Frontend, path: std::path::PathBuf| crate::driver::parsed(&path, frontend, None).expect_err("refused").0;
    assert!(refused(&flat, write("a.nib", "cdecl16")).contains("defines no \"cdecl16\" calling convention"));
    assert!(refused(&Default::default(), write("b.nib", "cdecl32")).contains("defines no \"cdecl32\" calling convention"));
    crate::driver::parsed(&write("c.nib", "cdecl32"), &flat, None).unwrap_or_else(|error| panic!("{}", error.0));
}

/// Inline assembly on a flat target was assembled as 16-bit code and emitted without a word: its
/// `mov ax, 0` became bytes a 32-bit decoder reads as `mov eax, imm32`, and a program hung. The block is
/// assembled in the target's mode now (its bits, segments and address width are the description's): the
/// same `mov ax, 0` takes the 66h prefix, and a segment register is refused where the target has none.
#[test]
fn test_inline_assembly_is_assembled_in_the_targets_mode() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("a.nib");
    let program = |line: &str| format!("fn main() -> i16:\n    unsafe:\n        asm(clobbers=[ax, es, flags]):\n            {line}\n    return 0\n");
    let flat_text = include_str!("../../../target/llrm-x86-code32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, bits: 32, ..Default::default() };
    let compiled = |source: &str, frontend: &crate::Frontend| {
        std::fs::write(&path, source).expect("written");
        crate::driver::parsed(&path, frontend, None).map(|_| ()).map_err(|error| error.0)
    };
    assert!(compiled(&program("mov ax, 0"), &flat).is_ok());
    assert!(compiled(&program("mov es, ax"), &flat).expect_err("refused").contains("this target has no segments: es is not available"));
    assert!(compiled(&program("mov es, ax"), &Default::default()).is_ok(), "real mode has segments");
}

/// `.near()` of a far pointer was `unsafe` on every target, though where far is near the offset is
/// the whole pointer: a flat program needed an `unsafe:` block for a copy.
#[test]
fn test_near_of_a_far_pointer_is_a_plain_copy_where_far_is_near() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("n.nib");
    std::fs::write(&path, "var cell: i16 = 7\n\nfn main() -> i16:\n    unsafe:\n        let wide: *far i16 = &cell\n        let narrow: *near i16 = wide.near()\n        return *narrow\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-code32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..Default::default() };
    crate::driver::parsed(&path, &flat, None).unwrap_or_else(|error| panic!("{}", error.0));
    std::fs::write(&path, "fn narrow(wide: *far i16) -> *near i16:\n    return wide.near()\n\nfn main() -> i16:\n    return 0\n").expect("written");
    let on_flat = crate::driver::parsed(&path, &flat, None);
    assert!(on_flat.is_ok(), "{:?}", on_flat.err());
    assert!(crate::driver::parsed(&path, &Default::default(), None).expect_err("real mode needs unsafe").0.contains("unsafe"));
}

/// A target-sized integer: `usize` is the unsigned integer as wide as the target's near pointer and
/// `NEAR_BYTES` that width as a constant, which a `const` may use (the heap's size classes and header
/// were u16 and 13 whatever the target).
#[test]
fn test_usize_and_near_bytes_follow_the_targets_near_width() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("u.nib");
    std::fs::write(&path, "const BITS = NEAR_BYTES * 8\nconst TOP = (1 << BITS) - 1\n\nfn main() -> i16:\n    let one: usize = 1\n    print(BITS)\n    print(size_of[usize]())\n    print(TOP)\n    print(one << 15)\n    return 0\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-code32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..Default::default() };
    let run = |frontend: &crate::Frontend| {
        let hir = crate::compile_file(&path, frontend).unwrap_or_else(|(_, error)| panic!("{}", error.message));
        llrm_core::hir::execute::run(&llrm_core::hir::codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs").output
    };
    assert_eq!(run(&Default::default()), "16\n2\n65535\n32768\n");
    assert_eq!(run(&flat), "32\n4\n4294967295\n32768\n");
}

/// A length is usize, the target's word (and so is `v.len + 1`): on code32 `let n: u16 = v.len` cut it to 16 bits without a word,
/// and a vector past 64 KB then looked short. It warns, naming the explicit form; `u16(v.len)` and a
/// word-wide target do not, and code16 (where a word is 16 bits) warns only for a byte.
#[test]
fn test_a_length_narrowed_implicitly_warns() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("w.nib");
    std::fs::write(&path, "fn main() -> i16:\n    let v: vec[i32] = [1, 2, 3]\n    let n: u16 = v.len\n    let m: u8 = v.len\n    let k: u16 = u16(v.len)\n    let w: u32 = v.len\n    let p: u16 = v.len + 1\n    print(n + u16(m) + k + p)\n    print(w)\n    return 0\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-code32/src/machines/datalayout.toml");
    let warned = |frontend: crate::Frontend| {
        crate::driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{}", error.0));
        let found: Vec<String> = frontend.warnings.borrow().iter().map(|one| format!("{}:{}", one.span.line, one.message)).collect();
        found
    };
    let flat = warned(crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..Default::default() });
    assert_eq!(flat, ["3:warning: usize is 4 bytes and u16 holds fewer: write u16(...) to cut it", "4:warning: usize is 4 bytes and u8 holds fewer: write u8(...) to cut it", "7:warning: usize is 4 bytes and u16 holds fewer: write u16(...) to cut it"]);
    assert_eq!(warned(Default::default()), ["4:warning: usize is 2 bytes and u8 holds fewer: write u8(...) to cut it"]);
}
