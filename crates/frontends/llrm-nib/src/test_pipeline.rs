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
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os)?;
    let object = crate::compile::object(&module, &nib::fixture(&format!("{fixture}.nib")), llrm_core::backend::objbuild::CodeLayout::OneSegment, llrm_target::object::Format::Omf)?;
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

/// `stack_base` is the stack `start.asm` links (STACK_BYTES, which the assembler is told): a different one would size the object's wrongly.
#[test]
fn test_the_stack_base_is_the_one_start_links() {
    let start = std::fs::read_to_string(crate::test_nib_frontend::root().join("runtime/shared/dos/m16/start.asm")).unwrap();
    let os = crate::real_mode().os;
    assert!(start.contains(".stack STACK_BYTES") && os.defines.contains(&("STACK_BYTES".to_owned(), os.stack_base.to_string())));
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

    let check = crate::real_mode().os.stack.clone();
    let runtime = |name: &str| {
        let directory = if name.ends_with(".asm") { format!("{}/../../../runtime/shared/dos/m16", env!("CARGO_MANIFEST_DIR")) } else { format!("{}/src/runtime", env!("CARGO_MANIFEST_DIR")) };
        std::fs::read_to_string(format!("{directory}/{name}")).unwrap()
    };
    assert!(runtime("os.asm").contains(&format!("public {}", check.limit)) && runtime("start.asm").contains(&format!("mov {}, ax", check.limit)));
    assert!(runtime("errors.nib").contains(&format!("@export(name=\"{}\")", check.handler)));
    // The limit sits the reserve `stack_to_add` leaves above the stack's bottom.
    assert!(runtime("start.asm").contains(&"add ax, STACK_RESERVE".to_owned()));
    let mut program = nib::parsed(&nib::fixture("sum.nib"));
    program.stack_check = Some(llrm_core::hir::model::StackCheck { limit: "FOO".into(), handler: "BAR".into(), ..check });
    let sum = _procedure(&nib::listing(&program, "sum", &nib::O2()), "_sum");
    assert!(sum.contains("cmp sp, word ptr FOO") && sum.contains("call far ptr BAR") && !sum.contains("_llrm_os_stack_low"), "{sum}");
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
    let module = crate::compile::assembled(&program, "main", &options, &crate::real_mode().os).expect("assembles");
    let object = crate::compile::object(&module, &source, llrm_core::backend::objbuild::CodeLayout::OneSegment, llrm_target::object::Format::Omf).expect("an object").len();
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
    let flat = crate::Frontend { conventions: vec!["cdecl32".into()], ..crate::real_mode() };
    let refused = |frontend: &crate::Frontend, path: std::path::PathBuf| crate::driver::parsed(&path, frontend, None).expect_err("refused").0;
    assert!(refused(&flat, write("a.nib", "cdecl16")).contains("defines no \"cdecl16\" calling convention"));
    assert!(refused(&crate::real_mode(), write("b.nib", "cdecl32")).contains("defines no \"cdecl32\" calling convention"));
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
    let flat_text = include_str!("../../../target/llrm-x86-m32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, bits: 32, ..crate::real_mode() };
    let compiled = |source: &str, frontend: &crate::Frontend| {
        std::fs::write(&path, source).expect("written");
        crate::driver::parsed(&path, frontend, None).map(|_| ()).map_err(|error| error.0)
    };
    assert!(compiled(&program("mov ax, 0"), &flat).is_ok());
    assert!(compiled(&program("mov es, ax"), &flat).expect_err("refused").contains("this target has no segments: es is not available"));
    assert!(compiled(&program("mov es, ax"), &crate::real_mode()).is_ok(), "real mode has segments");
}

/// `.near()` of a far pointer was `unsafe` on every target, though where far is near the offset is
/// the whole pointer: a flat program needed an `unsafe:` block for a copy.
#[test]
fn test_near_of_a_far_pointer_is_a_plain_copy_where_far_is_near() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("n.nib");
    std::fs::write(&path, "var cell: i16 = 7\n\nfn main() -> i16:\n    unsafe:\n        let wide: *far i16 = &cell\n        let narrow: *near i16 = wide.near()\n        return *narrow\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-m32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..crate::real_mode() };
    crate::driver::parsed(&path, &flat, None).unwrap_or_else(|error| panic!("{}", error.0));
    std::fs::write(&path, "fn narrow(wide: *far i16) -> *near i16:\n    return wide.near()\n\nfn main() -> i16:\n    return 0\n").expect("written");
    let on_flat = crate::driver::parsed(&path, &flat, None);
    assert!(on_flat.is_ok(), "{:?}", on_flat.err());
    assert!(crate::driver::parsed(&path, &crate::real_mode(), None).expect_err("real mode needs unsafe").0.contains("unsafe"));
}

/// A target-sized integer: `usize` is the unsigned integer as wide as the target's near pointer and
/// `NEAR_BYTES` that width as a constant, which a `const` may use (the heap's size classes and header
/// were u16 and 13 whatever the target).
#[test]
fn test_usize_and_near_bytes_follow_the_targets_near_width() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("u.nib");
    std::fs::write(&path, "const BITS = NEAR_BYTES * 8\nconst TOP = (1 << BITS) - 1\n\nfn main() -> i16:\n    let one: usize = 1\n    print(BITS)\n    print(size_of[usize]())\n    print(TOP)\n    print(one << 15)\n    return 0\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-m32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..crate::real_mode() };
    let run = |frontend: &crate::Frontend| {
        let hir = crate::compile_file(&path, frontend).unwrap_or_else(|(_, error)| panic!("{}", error.message));
        llrm_core::hir::execute::run(&llrm_core::hir::codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs").output
    };
    assert_eq!(run(&crate::real_mode()), "16\n2\n65535\n32768\n");
    assert_eq!(run(&flat), "32\n4\n4294967295\n32768\n");
}

/// A length is usize, the target's word (and so is `v.len + 1`): on m32 `let n: u16 = v.len` cut it to 16 bits without a word,
/// and a vector past 64 KB then looked short. It warns, naming the explicit form; `u16(v.len)` and a
/// word-wide target do not, and m16 (where a word is 16 bits) warns only for a byte.
#[test]
fn test_a_length_narrowed_implicitly_warns() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("w.nib");
    std::fs::write(&path, "fn main() -> i16:\n    let v: vec[i32] = [1, 2, 3]\n    let n: u16 = v.len\n    let m: u8 = v.len\n    let k: u16 = u16(v.len)\n    let w: u32 = v.len\n    let p: u16 = v.len + 1\n    print(n + u16(m) + k + p)\n    print(w)\n    return 0\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-m32/src/machines/datalayout.toml");
    let warned = |frontend: crate::Frontend| {
        crate::driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{}", error.0));
        let found: Vec<String> = frontend.warnings.borrow().iter().map(|one| format!("{}:{}", one.span.line, one.message)).collect();
        found
    };
    let flat = warned(crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..crate::real_mode() });
    assert_eq!(flat, ["3:warning: usize is 4 bytes and u16 holds fewer: write u16(...) to cut it", "4:warning: usize is 4 bytes and u8 holds fewer: write u8(...) to cut it", "7:warning: usize is 4 bytes and u16 holds fewer: write u16(...) to cut it"]);
    assert_eq!(warned(crate::real_mode()), ["4:warning: usize is 2 bytes and u8 holds fewer: write u8(...) to cut it"]);
}

/// The interpreter wrote and read 6-byte, 16-bit buffer headers whatever the program's words were:
/// strings and vectors compiled for m32 (12-byte header, 4-byte words) ran as garbage or failed.
/// It reads the word from the program (`descriptor_word`) and the layout from the HIR's one method.
#[test]
fn test_the_interpreter_runs_strings_and_vectors_of_a_target_with_wide_words() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("w.nib");
    std::fs::write(&path, "fn main() -> i16:\n    let s = \"hello\" + \" world\"\n    let mut v: vec[i32] = []\n    for i in 0..40:\n        v.push(i32(i) * 3)\n    print(s)\n    print(s.len)\n    print(v.len)\n    print(v[39])\n    return 0\n").expect("written");
    let flat_text = include_str!("../../../target/llrm-x86-m32/src/machines/datalayout.toml");
    let flat = crate::Frontend { layout: llrm_target::layout::Layout::parse(flat_text).expect("parses"), slot: 4, ..crate::real_mode() };
    let run = |frontend: &crate::Frontend| {
        let hir = crate::compile_file(&path, frontend).unwrap_or_else(|(_, error)| panic!("{}", error.message));
        let executed = llrm_core::hir::execute::run(&llrm_core::hir::codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        assert_eq!(executed.leaked, 0);
        executed.output
    };
    assert_eq!(run(&crate::real_mode()), "hello world\n11\n40\n117\n");
    assert_eq!(run(&flat), "hello world\n11\n40\n117\n");
    // A program that only prints a literal has no descriptor place: the program states its word.
    std::fs::write(&path, "fn main() -> i16:\n    print(\"literal\")\n    return 0\n").expect("written");
    assert_eq!(run(&flat), "literal\n");
}

/// The machine's physical addresses are one fact in the platform description; a module names one as
/// `PHYSICAL_<NAME>` (the text screen's video memory was typed as 0xB8000000 in each program).
#[test]
fn test_a_module_names_the_targets_physical_addresses() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("p.nib");
    std::fs::write(&path, "const SCREEN = PHYSICAL_TEXT_SCREEN\n\nfn main() -> i16:\n    print(i32(SCREEN))\n    return 0\n").expect("written");
    let frontend = crate::real_mode();
    let hir = crate::compile_file(&path, &frontend).unwrap_or_else(|(_, error)| panic!("{}", error.message));
    let executed = llrm_core::hir::execute::run(&llrm_core::hir::codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
    assert_eq!(executed.output, "753664\n");
    assert!(crate::Frontend { physical: Vec::new(), ..crate::real_mode() }.physical_constants().is_empty());
}

/// The reserve below the deepest chain was `STACK_RESERVE = 512` in the compiler and a `512` in each start-up: the OS layer
/// states it once (`os.toml`), the compiler reads it and the assembler is told it, and no start-up has a number of its own.
#[test]
fn the_stack_reserve_is_the_os_layers_and_no_startup_names_one() {
    use llrm_target::Target;
    for (target, start) in [(&llrm_x86_m16::M16 as &dyn Target, "runtime/shared/dos/m16/start.asm"), (&llrm_x86_m32::M32, "runtime/shared/dos/m32/start.asm")] {
        let os = crate::Os::for_target(target).unwrap();
        assert_eq!(os.stack_reserve, target.os_layer().unwrap().integer("stack_reserve").unwrap());
        let text = std::fs::read_to_string(std::path::Path::new(env!("LLRM_ROOT")).join(start)).unwrap();
        assert!(text.contains("STACK_RESERVE") && !text.contains("512"), "{start}");
    }
}

/// A convention spells a symbol as the object format asks: `f_` for OMF's default convention, and `f` for ELF's, where the
/// description's `symbol` table says `*`. Nib spelled `Abi::symbol`'s pattern whatever format was asked.
#[test]
fn test_an_exported_symbol_is_spelled_as_the_object_format_asks() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("a.nib");
    std::fs::write(&path, "@export(\"watcall32\")\nfn f(a: i32) -> i32:\n    return a\n\nfn main() -> i32:\n    return f(1)\n").expect("written");
    let spelled = |symbols: &[(&str, &str)]| {
        let frontend = crate::Frontend { conventions: vec!["watcall32".into()], symbols: symbols.iter().map(|(name, pattern)| ((*name).to_owned(), (*pattern).to_owned())).collect(), ..crate::real_mode() };
        let program = crate::driver::parsed(&path, &frontend, None).unwrap_or_else(|error| panic!("{}", error.0));
        program.modules[0].functions.iter().map(|one| one.name.clone()).collect::<Vec<_>>()
    };
    assert!(spelled(&[]).contains(&"f_".to_owned()));
    assert!(spelled(&[("watcall32", "*")]).contains(&"f".to_owned()));
}
