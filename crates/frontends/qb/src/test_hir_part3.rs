//! `tests/test_hir.py` QB cases, part 3; helpers in `test_hir`.
//!
//! Also the stage observer test
//! `test_stage_observer_uses_one_compilation_and_preserves_object_bytes` and
//! `tests/test_qb_frontend_command.
//! py::test_common_hir_profiles_do_not_become_qb_frontend_options`.
// skipped: test_an_oversized_exact_loop_is_never_cloned_as_a_peel_candidate:
// monkeypatches loopclone.peeled

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use llrm_codegen::backend::masm;
use llrm_codegen::driver::basic::finalized;
use llrm_codegen::hir::model::{self as hir, Operand};
use llrm_codegen::model::ir::{self, Loc, Operation};
use llrm_codegen::model::lir;
use llrm_codegen::objectfile::omf;
use llrm_codegen::support::hash::IndexMap;

use super::abi::_contract;
use super::compile as qb_compile;
use super::driver as qb_driver;
use super::test_hir::*;

// ---- small helpers -------------------------------------------------------

fn compat(path: &str) -> std::path::PathBuf {
    root().join("tests/differential/conformance").join(path)
}

/// `qb_driver.parsed(source, dialect=..., runtime=..., array_order=...,
/// huge_arrays=..., checked_arrays=...)`.
fn parsed_with(
    source: &Path,
    dialect: &str,
    runtime: &str,
    array_order: &str,
    huge: bool,
    checked: bool,
) -> hir::Program {
    qb_driver::parsed(
        source,
        &qb_driver::Frontend {
            array_order: array_order.into(),
            huge_arrays: huge,
            checked_arrays: checked,
            ..qb_driver::Frontend::new(dialect, runtime)
        },
        None,
    )
    .unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

/// Each listing line without its comment, whitespace collapsed.
fn stripped_lines(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(|line| line.split(';').next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn externals(
    program: &hir::Program,
    name: &str,
) -> Vec<String> {
    omf::externals(&records(program, name))
}

fn function_named<'p>(
    program: &'p hir::Program,
    name: &str,
) -> (usize, &'p hir::Function) {
    program.modules[0].functions.iter().enumerate().find(|(_, one)| one.name == name).expect("the function")
}

fn returning(
    id: i64,
    instructions: Vec<hir::Instruction>,
) -> hir::Block {
    hir::Block::new(id, instructions, hir::Terminator::new(hir::TerminatorKind::Return, vec![], vec![]))
}

fn vbdos(module: hir::Module) -> hir::Program {
    hir::Program::new(hir::Dialect::Vbdos, hir::RuntimeProfile::Vbdos, vec![module])
}

fn is_word(one: char) -> bool {
    one.is_alphanumeric() || one == '_'
}

fn all_word(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_word)
}

/// `re.findall(r"mov word ptr SUM_THREE\$D\d+,", text)` count.
fn static_stores(text: &str) -> usize {
    let needle = "mov word ptr SUM_THREE$D";
    text.match_indices(needle)
        .filter(|(at, _)| {
            let rest = &text[at + needle.len()..];
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && rest[digits..].starts_with(',')
        })
        .count()
}

/// `re.search(r"word ptr \[\w+\+\w+\+<disp>\]", text)`.
fn has_indexed_field(
    text: &str,
    disp: &str,
) -> bool {
    let needle = "word ptr [";
    text.match_indices(needle).any(|(at, _)| {
        let Some((inside, _)) = text[at + needle.len()..].split_once(']') else { return false };
        let parts: Vec<&str> = inside.split('+').collect();
        parts.len() == 3 && all_word(parts[0]) && all_word(parts[1]) && parts[2] == disp
    })
}

/// `re.finditer(r"\bj\w+ (\w+)\n", text)` when `newline`, else
/// `re.findall(r"\bj(?!mp)\w+ (\w+)$", text, re.MULTILINE)`: `(start, end,
/// label)`.
fn jumps(
    text: &str,
    newline: bool,
) -> Vec<(usize, usize, String)> {
    let mut found = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let ended = line.ends_with('\n');
        let body = line.strip_suffix('\n').unwrap_or(line);
        if !newline || ended {
            let characters: Vec<(usize, char)> = body.char_indices().collect();
            for (index, (at, one)) in characters.iter().enumerate() {
                if *one != 'j' || (index > 0 && is_word(characters[index - 1].1)) {
                    continue;
                }
                let rest = &body[at + 1..];
                if !newline && rest.starts_with("mp") {
                    continue;
                }
                if let Some((mnemonic, label)) = rest.split_once(' ') {
                    if all_word(mnemonic) && all_word(label) {
                        found.push((offset + at, offset + body.len() + usize::from(newline), label.to_owned()));
                        break;
                    }
                }
            }
        }
        offset += line.len();
    }
    found
}

/// `re.findall(r"^(\w+):$", text, re.MULTILINE)`.
/// Labels of `re.findall(r"^(\w+):\n((?:    .*\n)*)", text, re.MULTILINE)`
/// blocks calling B$...BND.
fn bound_call_labels(text: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut found = BTreeSet::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(name) = line.strip_suffix(":\n").filter(|name| all_word(name)) else { continue };
        let block: String =
            lines[at + 1..].iter().take_while(|one| one.starts_with("    ") && one.ends_with('\n')).copied().collect();
        if block.contains("B$") && block.contains("BND") {
            found.insert(name.to_owned());
        }
    }
    found
}

/// sumThree's listing, `-fsanitize=bounds` where `checked`.
fn sum_three(checked: bool) -> String {
    let source = root().join("tests/inputs/qb/sum_three.bas");
    let program = parsed_with(&source, "vbdos", "vbdos", "column-major", false, checked);
    let text = listing(&program);
    let start = text.find("SUMTHREE proc").expect("SUMTHREE proc");
    let end = text.find("SUMTHREE endp").expect("SUMTHREE endp");
    text[start..end].to_owned()
}

/// `_backward_loop`: from the first label a later jump returns to, through that
/// jump.
fn backward_loop(procedure: &str) -> String {
    found_backward_loop(procedure).expect("no loop")
}

fn found_backward_loop(procedure: &str) -> Option<String> {
    for (start, end, label) in jumps(procedure, true) {
        if let Some(at) = procedure.find(&format!("{label}:\n")) {
            if at < start {
                return Some(procedure[at..end].to_owned());
            }
        }
    }
    None
}

#[test]
fn test_a_counter_whose_start_seeds_pointers_counts_up_to_its_symbolic_bound() {
    // The pin's indvars rewrites the exit to `icmp eq %next, lb + count`
    // (`opt -passes='default<O2>'` on our early IR gives ours instruction for
    // instruction since foldICmpBinOp's add/sub arms), so the counter runs
    // down to zero: `dec; jne`, with the two reloads from [bp].
    let loop_ = backward_loop(&sum_three(false));
    assert_eq!(loop_.matches("[bp").count(), 2, "{loop_}");
    assert!(regex::Regex::new(r"\n    dec \w+\n    jne \w+\n$").unwrap().is_match(&loop_), "{loop_}");
}

#[test]
fn test_a_loop_whose_exit_moves_a_value_closes_on_its_branch() {
    // The exit's `mov ax,cx` sat after `retf`, so every trip ran `je` out and
    // `jmp` back. The loop closes on its conditional branch (`jne`, the pin's
    // `icmp eq` exit test for this bound).
    let loop_ = backward_loop(&sum_three(false));
    assert!(regex::Regex::new(r"\bjne \w+\n$").unwrap().is_match(&loop_), "{loop_}");
    assert!(!loop_.contains("jmp"), "{loop_}");
}

/// `_function`.
fn function_in(
    directory: &tempfile::TempDir,
    name: &str,
    text: &str,
    wanted: &str,
    dialect: &str,
) -> (hir::Program, hir::Function) {
    let source = written(directory, name, text.as_bytes());
    let program = parsed_as(&source, dialect, dialect);
    let function = program.modules[0].functions.iter().find(|one| one.name == wanted).expect("the function").clone();
    (program, function)
}

/// `_value_exit`.
fn value_exit(function: &hir::Function) -> &hir::Block {
    function
        .blocks
        .iter()
        .find(|block| block.terminator.kind == hir::TerminatorKind::Return && !block.terminator.operands.is_empty())
        .expect("a value exit")
}

fn callees(block: &hir::Block) -> Vec<Option<&str>> {
    block.instructions.iter().map(|one| one.callee.as_deref()).collect()
}

fn position(
    calls: &[Option<&str>],
    name: &str,
) -> usize {
    calls.iter().position(|one| *one == Some(name)).unwrap_or_else(|| panic!("{name} not called"))
}

// ---- tests ---------------------------------------------------------------

/// QGL HOST_RENDER's three `delta ^ 2` terms left duplicate x87 values live.
#[test]
fn test_inline_square_leaves_no_float_live_out() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SQUARE.BAS", b"dim x as single, answer as single\r\nanswer = x ^ 2\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    assert!(!object_bytes(&source, "SQUARE.BAS").expect("emits").is_empty());
}

/// SYS_MEM_MARK stopped before HIR because FRE("") was sent through numeric
/// lowering.
#[test]
fn test_string_fre_emits_the_measured_vbdos_runtime_call() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "FRESTR.BAS", b"dim available as long\r\navailable = fre(\"\")\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    assert!(externals(&source, "FRESTR.BAS").iter().any(|one| one == "B$FRSD"));
}

/// A masked subscript of a zero-based far array was folded into a scaled
/// 32-bit address whose index `promote` cannot widen: "secondary address
/// values have no definition".
#[test]
fn test_a_masked_far_subscript_compiles() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "MASKED.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB t (f)\r\nt 3\r\nSUB t (f)\r\nDIM s(511), u(319)\r\nFOR x = 0 TO 319\r\nu(x) = s((x + f) AND 511)\r\nNEXT\r\nEND SUB\r\n",
    );
    let program = parsed_as(&basic, "qb45", "qb45");
    object_bytes(&program, "MASKED.BAS").expect("compiles");
    // The masked `s(i)` is a 16-bit offset doubled by an add and cut to the
    // array's 1022 (no `shl`, no 32-bit index to widen), and the unmasked
    // `u(x)` addresses through a doubled 32-bit index whose upper half is
    // zeroed once before the loop. DEVIATE LU2 (#3646): the lea that doubles
    // the index (`lea bx, [eax+eax]`) carries the store's offset, so the loop
    // counter runs to zero and no `movzx` widens it.
    let text = listing(&program);
    let body = &text[text.find("T proc").expect("T proc")..text.find("T endp").expect("T endp")];
    let mask = regex::Regex::new(r"and (\w+), 1022\n\s+mov \w+, word ptr fs:\[(\w+)\]").unwrap();
    let masked = mask.captures(body).is_some_and(|one| one[1] == one[2]);
    let index = regex::Regex::new(r"lea \w+, \[(e\w\w)\+(e\w\w)\]").unwrap();
    let doubled = index.captures_iter(body).any(|one| one[1] == one[2]);
    assert!(masked && doubled && !body.contains("shl"), "{body}");
}

/// The main body of `source`'s listing.
fn main_listing(
    name: &str,
    source: &[u8],
) -> String {
    procedure_listing(name, source, "$QB$MAIN")
}

/// `procedure`'s part of `source`'s listing.
fn procedure_listing(
    name: &str,
    source: &[u8],
    procedure: &str,
) -> String {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, name, source);
    let program = parsed_as(&basic, "qb45", "qb45");
    object_bytes(&program, name).expect("compiles");
    let text = listing(&program);
    text[text.find(&format!("{procedure} proc")).expect("start")..text.find(&format!("{procedure} endp")).expect("end")]
        .to_owned()
}

/// qbdemo's FRACLINE2 never converged: strength rewrote `x% + y320%`, an
/// index already its invariant plus the counter, into a copy of the
/// invariant plus the counter; gvn folded the copy back, every round.
#[test]
fn test_an_index_already_carried_by_its_counter_is_not_reduced_again() {
    let source = "DECLARE SUB fracline2 (y%, y1#, y2#, x1#, x2#, distthr#)\r\nDEFINT A-Z\r\nDEF SEG = &HA000\r\n\
FOR r = 0 TO 199: fracline2 r, 0, 1, 0, 1, 4: NEXT\r\nDEFDBL A-Z\r\nSUB fracline2 (y%, y1, y2, x1, x2, distthr)\r\n\
deltax = (x2 - x1) / 320\r\ndeltay = (y2 - y1) / 320\r\ny = y1\r\nx% = 0\r\ny320% = y% * 320\r\nx = x1\r\nDO\r\n\
IF PEEK(x% + y320% - 320) = PEEK(x% + y320% + 320) THEN\r\niter% = PEEK(x% + y320% - 320)\r\nELSE\r\n\
re = 0: im = 0: iter% = 0\r\nDO\r\ntemp = re * re - im * im\r\nim = re * im\r\nim = im + im + y\r\nre = temp + x\r\n\
iter% = iter% + 1\r\nLOOP UNTIL re * re + im * im >= distthr OR iter% = 255\r\nEND IF\r\nPOKE x% + y320%, iter%\r\n\
x% = x% + 1\r\nIF x% >= 320 THEN EXIT SUB\r\nx = x + deltax\r\ny = y + deltay\r\nLOOP\r\nEND SUB\r\n";
    procedure_listing("FRAC.BAS", source.as_bytes(), "FRACLINE2");
}

/// qbdemo's PLASMA: a split piece of `DEF SEG = &HA000` was evicted in the
/// loop's preheader and never split again, so the pixel loop remade the
/// segment with `push 0A000h / pop fs` every trip.
#[test]
fn test_an_evicted_piece_splits_again_rather_than_remake_a_segment_in_its_loop() {
    let source = "DEFINT A-Z\r\nDECLARE SUB plasma (totalframes)\r\nDECLARE SUB updpalplasma (f)\r\nplasma 2\r\n\
SUB plasma (totalframes)\r\nDIM unf(320), unfunf(320)\r\nDIM sine(512)\r\nDIM fuh(128, 128)\r\nDEF SEG = &HA000\r\n\
FOR x = 0 TO 512\r\nsine(x) = SIN(x * 3.14 / 256) * 32 + 32\r\nNEXT\r\nFOR f = 1 TO totalframes\r\n\
FOR x = 0 TO 320\r\nunf(x) = sine((x + f) AND 511) + sine((3 * x + 7 * f + 3) AND 511)\r\nNEXT\r\no = 0\r\n\
FOR y = 0 TO 128\r\nunf2 = sine((y * 7 + f * 5) AND 511) + sine((y * 14 + f * 11 + 1943) AND 511)\r\n\
FOR x = 0 TO 128\r\nfuh(x, y) = unf(x) + unf2\r\no = o + 1\r\nNEXT\r\nNEXT\r\n\
FOR x = 0 TO 320\r\nunf(x) = sine((x * 11 + f * 7) AND 511) + sine((3 * x + 7 * f + 3) AND 511)\r\n\
unfunf(x) = sine((x * 4 + f * 5) AND 511) + sine((9 * x + 2 * f + 371) AND 511)\r\nNEXT\r\no = 0\r\n\
FOR y = 0 TO 199\r\nunf2 = sine((y * 11 + f * 6) AND 511) + sine((y * 14 + f * 11 + 1943) AND 511)\r\n\
unf3 = sine((y * 9 + f * 4) AND 511) + sine((y * 17 + f * 23 + 1943) AND 511)\r\n\
FOR x = 0 TO 319\r\nPOKE o, fuh((unf(x) + unf2) AND 127, (unfunf(x) + unf3) AND 127)\r\no = o + 1\r\nNEXT\r\nNEXT\r\n\
updpalplasma f\r\nIF INKEY$ > \"\" THEN EXIT SUB\r\nNEXT\r\nEND SUB\r\nSUB updpalplasma (f)\r\nOUT &H3C8, f\r\nEND SUB\r\n";
    let text = procedure_listing("PLASMA.BAS", source.as_bytes(), "PLASMA");
    let remade = regex::Regex::new(r"\bpop [efg]s\b").unwrap();
    let loops: Vec<(usize, usize)> = jumps(&text, true)
        .into_iter()
        .filter_map(|(start, end, label)| {
            text.find(&format!("{label}:\n")).filter(|at| *at < start).map(|at| (at, end))
        })
        .collect();
    let innermost = loops.iter().filter(|(at, end)| !loops.iter().any(|(other, _)| at < other && other < end));
    for (at, end) in innermost {
        assert!(!remade.is_match(&text[*at..*end]), "{}", &text[*at..*end]);
    }
}

/// A static array's proven-exact subscript kept its shift and read the
/// symbol through a 16-bit register: no 32-bit symbolic form or fixup.
#[test]
fn test_an_exact_static_subscript_folds_into_a_32_bit_symbolic_address() {
    let body = main_listing(
        "STATIC.BAS",
        b"DEFINT A-Z\r\nDIM s(1000), t(319)\r\nFOR i = 0 TO 1000\r\ns(i) = i * 3\r\nNEXT\r\nFOR x = 0 TO 319\r\nt(x) = s((x * x) AND 511)\r\nNEXT\r\nPRINT t(5)\r\n",
    );
    assert!(body.contains("S%[e") && !body.contains("shl"), "{body}");
}

/// A negative subscript leaf, zero-extended, addressed `A%+20[esi+esi]`
/// 128K past the element. The address stays 16 bit; the scaled index may be
/// computed in a 32-bit `lea` (its low half is the same, and no header copy
/// reshapes the loop now).
#[test]
fn test_a_negative_static_subscript_stays_16_bit() {
    let body = main_listing(
        "NEGATIVE.BAS",
        b"DEFINT A-Z\r\nDIM a(-10 TO 10)\r\nFOR j = 0 TO 99\r\ni = (PEEK(j) AND 7) - 5\r\na(i) = j\r\nNEXT\r\nPRINT a(5)\r\n",
    );
    let wide: Vec<&str> = body.lines().filter(|line| line.contains("A%") && line.contains("[e")).collect();
    assert!(wide.is_empty(), "{wide:?}\n{body}");
}

/// D_SURF SC_FTAKE lost far-array address definitions during secondary folding.
#[test]
fn test_dynamic_array_walk_keeps_far_pointer_halves_defined() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FARWALK.BAS",
        concat!(
            "option explicit\r\n",
            "dim shared head() as integer\r\n",
            "dim shared link() as integer\r\n",
            "dim shared group() as integer\r\n",
            "function take (byval order as integer, byval wanted as integer) as integer\r\n",
            "dim block as integer, previous as integer\r\n",
            "block = head(order)\r\n",
            "previous = -1\r\n",
            "while block >= 0\r\n",
            "if group(block) = wanted then\r\n",
            "if previous >= 0 then link(previous) = link(block) else head(order) = link(block)\r\n",
            "take = block\r\n",
            "exit function\r\n",
            "end if\r\n",
            "previous = block\r\n",
            "block = link(block)\r\n",
            "wend\r\n",
            "take = -1\r\n",
            "end function\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_with(&basic, "vbdos", "vbdos", "row-major", false, false);
    assert!(!object_bytes(&source, "FARWALK.BAS").expect("emits").is_empty());
}

/// SCREEN stopped at ABI lowering although VBDOS B$DSG0 is a zero-argument
/// RETF.
#[test]
fn test_bare_def_seg_reaches_object_emission() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "DEFSEG.BAS", b"def seg = 40960\r\npoke 12, 34\r\ndef seg\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    let names = externals(&source, "DEFSEG.BAS");
    assert!(names.iter().any(|one| one == "B$DSG0"));
    assert!(!names.iter().any(|one| one == "B$POKE"));
}

/// SCN9 called B$CSCN without B$EGAUSED, so LINK omitted EGA and SCREEN 9
/// raised error 5.
#[test]
fn test_constant_screen_mode_pulls_its_graphics_driver() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SCN9.BAS", b"screen 9\r\nscreen 0\r\n");
    let source = parsed(&basic);
    assert!(externals(&source, "SCN9.BAS").iter().any(|one| one == "B$EGAUSED"));
}

/// qbdemo's SCREEN 13 raised "Illegal function call" under QB45: nothing
/// references B$VGAUSED, so the OBJ writer pruned the EXTDEF that links it.
#[test]
fn test_a_qb45_screen_mode_keeps_its_driver_request() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SCN13.BAS", b"screen 13\r\n");
    let source = parsed_as(&basic, "qb45", "qb45");
    assert!(externals(&source, "SCN13.BAS").iter().any(|one| one == "B$VGAUSED"));
}

/// Gorillas SCREEN Mode linked no graphics modules and failed before drawing
/// its first frame.
#[test]
fn test_variable_screen_mode_pulls_all_graphics_drivers() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SCNVAR.BAS", b"dim mode as integer\r\nmode = 9\r\nscreen mode\r\n");
    let source = parsed(&basic);
    assert!(externals(&source, "SCNVAR.BAS").iter().any(|one| one == "B$GRPUSED"));
}

/// Gorillas emitted IDIV AX twice for 30 \\ (80 \\ MaxCol), faulting on its
/// first shot. Once the frontend folded 80 to a LONG constant, lowering dropped
/// it: `idiv eax`.
#[test]
fn test_nested_integer_division_keeps_each_dividend() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "NESTDIV.BAS",
        concat!(
            "defint a-z\r\n",
            "declare function scale (maxCol)\r\n",
            "print scale(80)\r\n",
            "end\r\n",
            "function scale (maxCol)\r\n",
            "scale = 30 \\ (80 \\ maxCol)\r\n",
            "end function\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let assembly = listing(&source);
    let scale = between(&assembly, "SCALE proc far\n", "SCALE endp\n");

    // CVP narrows both quotients (80 \\ x fits 16 bits, then 8): two word
    // divisions in place of two dword ones, each with its own dividend.
    assert!(scale.contains("mov ax, 80\n"));
    assert!(regex::Regex::new(r"mov [a-d]x, 30\n").unwrap().is_match(scale), "{scale}");
    assert_eq!(scale.matches("idiv").count(), 2);
    assert!(!scale.contains("idiv e"));
}

/// -2 ^ 3 printed 8: the exponent's parity sat in eax across `fnstsw ax`.
#[test]
fn test_a_float_compare_status_word_does_not_overwrite_a_live_ax() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "POWSIGN.BAS", b"b! = -2\r\ne! = 3\r\nr! = b! ^ e!\r\nprint r!\r\n");
    let source = parsed_as(&basic, "qb45", "qb45");
    let lines = stripped_lines(&listing(&source));
    let ax = regex::Regex::new(r"\b(e?ax|al|ah)\b").unwrap();
    for (index, line) in lines.iter().enumerate() {
        if line != "fnstsw ax" {
            continue;
        }
        for later in &lines[index + 1..] {
            if later == "sahf" || later.starts_with('j') || later.starts_with("L0_") {
                continue;
            }
            let (mnemonic, operands) = later.split_once(' ').unwrap_or((later, ""));
            let (destination, sources) = operands.split_once(',').unwrap_or((operands, ""));
            assert!(
                !ax.is_match(sources)
                    && !(ax.is_match(destination)
                        && !matches!(mnemonic, "mov" | "fnstsw")
                        && !mnemonic.starts_with("set")),
                "AX read after fnstsw: {later}"
            );
            if ax.is_match(destination) {
                break;
            }
        }
    }
}

/// RESUME NEXT after -8 ^ (1/3) raised error 5 reported "No line number":
/// the raise ran outside its statement, so the runtime found no statement
/// to resume after.
#[test]
fn test_a_raise_under_an_error_handler_resumes_after_its_statement() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "POWRES.BAS",
        b"on error goto h\r\nb! = -8: e! = .5\r\nfor i% = 1 to 2\r\nr! = b! ^ e!\r\nprint r!\r\nnext\r\nsystem\r\nh:\r\nresume next\r\n",
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let assembly = listing(&source);
    // The raise names its statement (in whichever frame slot: the frame also
    // holds the conversions' scratch cells), and RESUME NEXT from that
    // statement continues at the PRINT.
    let raised = regex::Regex::new(
        r"mov word ptr (?:ss:)?\$QB\$FRAME\+\d+, (\d+)\n(?:    fstp st\(0\)\n)*    pushw 5\n    call far ptr B\$SERR",
    )
    .unwrap();
    let statement = &raised.captures(&assembly).unwrap_or_else(|| panic!("{assembly}"))[1];
    // The dispatch continues at the statement's resume point, laid next
    // (`jne` past it) or branched to (`je`) as the placement falls.
    let dispatch = regex::Regex::new(&format!(
        r"cmp (?:ax|word ptr (?:ss:)?\$QB\$FRAME\+\d+), {statement}\n    (?:jne \w+\n    jmp (\w+)|je (\w+))\n"
    ))
    .unwrap();
    let found = dispatch.captures(&assembly).unwrap_or_else(|| panic!("{assembly}"));
    let label = found.get(1).or_else(|| found.get(2)).unwrap().as_str();
    let start = assembly.find(&format!("\n{label}:\n")).unwrap_or_else(|| panic!("{assembly}"));
    let block = |at: usize| {
        let tail = &assembly[at..];
        &tail[..tail.find("jmp ").unwrap_or(tail.len())]
    };
    let mut next = block(start).to_owned();
    // A resume point may reload what the statement computed and jump on to the
    // PRINT.
    let after = &assembly[start..];
    if let Some(target) = after[after.find("jmp ").map_or(after.len(), |at| at + 4)..].split_whitespace().next()
        && let Some(at) = assembly.find(&format!("\n{target}:\n"))
    {
        next.push_str(block(at));
    }
    assert!(next.contains("call far ptr B$PER4"), "{assembly}");
}

/// ENT_MOVE_TRIGS passed a four-byte far field address to a two-byte scalar
/// formal.
#[test]
fn test_byref_dynamic_array_field_copies_through_a_near_formal() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FARFIELD.BAS",
        concat!(
            "option explicit\r\n",
            "type Item\r\npad as integer\r\nvalue as integer\r\nend type\r\n",
            "declare sub consume (number as integer)\r\n",
            "dim shared items() as Item\r\n",
            "sub invoke (byval index as integer)\r\n",
            "consume items(index).value\r\n",
            "end sub\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_with(&basic, "vbdos", "vbdos", "row-major", false, false);

    let module = &source.modules[0];
    let invoke = module.functions.iter().find(|function| function.name == "INVOKE").expect("INVOKE");
    let types: IndexMap<i64, &hir::Type> = module.types.iter().map(|type_| (type_.id, type_)).collect();
    let values: IndexMap<i64, &hir::Type> =
        invoke.values.iter().map(|value| (value.id, types[&value.r#type])).collect();
    let instructions: Vec<&hir::Instruction> = invoke.blocks.iter().flat_map(|block| &block.instructions).collect();
    let call_at = instructions
        .iter()
        .position(|one| one.op == hir::Op::Call && one.callee.as_deref() == Some("CONSUME"))
        .expect("the CONSUME call");
    let Operand::ValueRef(argument) = &instructions[call_at].operands[0] else { panic!("a ValueRef argument") };
    assert_eq!(values[&argument.value].name, "near*integer");
    assert!(
        instructions.iter().any(|one| one.op == hir::Op::Load && matches!(one.operands[0], Operand::IndirectPlace(_)))
    );
    assert!(
        instructions[call_at + 1..]
            .iter()
            .any(|one| one.op == hir::Op::Store && matches!(one.operands[0], Operand::IndirectPlace(_)))
    );

    assert!(!object_bytes(&source, "FARFIELD.BAS").expect("emits").is_empty());
}

/// Q45N01's native SUB SP shifted B$ENRA's documented frame fields by four
/// bytes.
#[test]
fn test_runtime_frame_owns_spill_reservation_without_a_native_prefix() {
    let source = parsed_as(&compat("qb45/q45n01.bas"), "qb45", "qb45");
    let text = listing(&source);
    let procedure = between(&text, "$QB$MAIN proc far", "$QB$MAIN endp");
    let before_runtime_frame = procedure.split_once("call far ptr B$ENRA").map_or(procedure, |(before, _)| before);
    assert!(!before_runtime_frame.contains("sub sp"));
}

/// LOCERR retains an ABI entry before its independently resumable ERROR.
///
/// Threading the empty language entry into that statement made late
/// B$ENRA/B$OEGP insertion miss its block, so the emitted procedure began at
/// ERROR 53 with no runtime frame or local handler registration.
#[test]
fn test_local_error_and_resume_label_use_their_measured_procedure_abi() {
    let source = parsed_with(&compat("vbdos/locerr.bas"), "vbdos", "vbdos", "row-major", false, false);
    let (_, recover) = function_named(&source, "RECOVER_LOCALLY");
    assert!(recover.error_handler.is_some());
    assert!(recover.error_handler_local);

    let text = listing(&source);
    let procedure = between(&text, "RECOVER_LOCALLY proc far", "RECOVER_LOCALLY endp");
    let entered = procedure.find("call far ptr B$ENRA").expect("B$ENRA");
    let registered = procedure.find("call far ptr B$OEGP").expect("B$OEGP");
    let raised = procedure.find("call far ptr B$SERR").expect("B$SERR");
    assert!(entered < registered && registered < raised, "{procedure}");
    // The handler address is pushed as an immediate (`push offset`, #5135), not
    // made in `ax` first.
    assert!(procedure[..registered].contains("push offset"), "{procedure}");
    assert!(!procedure.contains("call far ptr B$OEGA"), "{procedure}");
}

/// PDLOCAL reported ERL 0 and resumed at L1_9 instead of recovered L1_4.
#[test]
fn test_pds_resume_target_and_numbered_erl_survive_distinct_identity_spaces() {
    let source = parsed_as(&compat("pds71/pdlocal.bas"), "pds71", "pds71");
    let module = optimized_mir(&source);
    let procedure = function_text(&module, "MISSINGFILE");
    // ERL is 100 at the only error site, so its check folds away: only the
    // ERR mismatch still reports, and the landing resumes into `recovered`.
    let reports: Vec<&str> = procedure.lines().filter(|line| line.contains("@REPORTFAIL(")).collect();
    assert_eq!(reports.len(), 1, "{procedure}");
    let errnum = module
        .lines()
        .find(|line| line.contains("c\"errnum\""))
        .and_then(|line| line.split(' ').next())
        .expect("the errnum literal");
    assert!(procedure.contains(&format!("(ptr {errnum}, ")), "{procedure}");
    assert!(procedure.contains("landingpad"), "{procedure}");
    assert!(
        procedure.lines().any(|line| line.contains("ptr @\"RECOVERYCOUNT%\"") && line.trim().starts_with("store")),
        "{procedure}"
    );
}

/// PDHUGE wrapped/aliased beyond 64 KiB when /Ah was dropped and its
/// elements were addressed as far ones. A huge element is addressed inline,
/// its offset carrying into the selector, with no B$HARY.
#[test]
fn test_pds_huge_array_uses_measured_ddim_and_inline_huge_addresses() {
    let source = parsed_with(&compat("pds71/pdhuge.bas"), "pds71", "pds71", "row-major", true, false);
    let assembly = listing(&source);
    assert!(assembly.contains("call far ptr B$DDIM"));
    assert!(!assembly.contains("B$HARY"), "{assembly}");
    // 123 is stored through a computed selector.
    assert!(regex::Regex::new(r"mov word ptr es:\[\w+\], 123").unwrap().is_match(&assembly), "{assembly}");
}

/// Q45P04 passed uninitialized slots after optimization deleted 100000 and 23.
#[test]
fn test_byref_call_keeps_the_temporary_values_it_publishes() {
    let text = optimized_mir(&parsed_as(&compat("qb45/q45p04.bas"), "qb45", "qb45"));
    // The sum is known: the program prints its PASS. A call to addLong that
    // stays reads the slots its caller stores, so they stay too.
    assert!(text.contains("PASS procedures") && !text.contains("FAIL"), "{text}");
    if text.lines().any(|line| line.contains("call") && line.contains("ADDLONG")) {
        assert!(text.contains("100000") && text.contains(" 23"), "{text}");
    }
}

/// FSTKBR's ``PICK = -1/0`` formerly left a volatile FILD live over its arm
/// jump.
///
/// Float allocation then refused the join with ``floating stack live-out
/// requires cross-block allocation``.  `$arg` is only published when an
/// ADDRESS reaches a BYREF call; an ordinary conversion scratch slot must
/// retain neither that publication nor an x87 value after its exact store
/// folds to bits.
#[test]
fn test_unpublished_float_conversion_temporary_does_not_hold_the_x87_stack_across_a_branch() {
    let source = parsed(&fixture("fstkbr.bas"));
    assert!(!object_bytes(&source, "FSTKBR.BAS").expect("emits").is_empty());
}

/// Q45LE71 reached B$LEFT but emission refused the previously VBDOS-only
/// cleanup.
#[test]
fn test_classic_string_stack_abis_are_measured_for_every_qb_runtime_family() {
    for family in [hir::RuntimeProfile::Qb45, hir::RuntimeProfile::Pds71, hir::RuntimeProfile::Vbdos] {
        for (name, pushed) in [
            ("B$LEFT", 4),
            ("B$RGHT", 4),
            ("B$UCAS", 2),
            ("B$FHEX", 4),
            ("B$FMKS", 4),
            ("B$FMKD", 8),
            ("B$FMSF", 4),
            ("B$FMDF", 8),
            ("B$FCVI", 2),
            ("B$FCVL", 2),
            ("B$FCVS", 2),
            ("B$FCVD", 2),
            ("B$MCVS", 2),
            ("B$MCVD", 2),
            ("B$INS3", 6),
        ] {
            let contract = _contract(name, hir::StackCleanup::Callee, pushed, family).expect("a contract");
            assert!(contract.established, "{name} {family:?}");
            assert_eq!(contract.cleanup, Some(pushed), "{name} {family:?}");
            assert_eq!(contract.inputs, Some(BTreeSet::new()), "{name} {family:?}");
        }
    }
}

/// Nibbles reached physical HIR and stopped at unaudited screen-call cleanup.
#[test]
fn test_vbdos_nibbles_screen_calls_have_fixed_stack_contracts() {
    for (name, pushed) in [("B$SCLS", 2), ("B$VWPT", 4), ("B$SPLY", 2), ("B$INKY", 0), ("B$USNG", 2)] {
        let contract =
            _contract(name, hir::StackCleanup::Callee, pushed, hir::RuntimeProfile::Vbdos).expect("a contract");
        assert!(contract.established, "{name}");
        assert_eq!(contract.cleanup, Some(pushed), "{name}");
        assert_eq!(contract.inputs, Some(BTreeSet::new()), "{name}");
    }
}

/// Q45FP61 reached OBJ emission with one unencodable eight-byte PUSH.
#[test]
fn test_double_runtime_argument_is_split_high_to_low_at_the_qb_abi_boundary() {
    let program = parsed_as(&compat("qb45/q45fp61.bas"), "qb45", "qb45");
    assert!(!object_bytes(&program, "Q45FP61.BAS").expect("emits").is_empty());
}

/// screen's green/blue fields formerly became an unlowerable address-of far
/// cell.
#[test]
fn test_dynamic_fixed_field_address_compiles() {
    let source = parsed(&fixture("dynamic_fixed_fields.bas"));
    assert!(assembled(&source).is_ok());
}

/// sc_selftest's SEG array argument formerly selected illegal ``[bp+bx]``.
#[test]
fn test_segmented_local_array_address_splits_frame_base_from_dynamic_offset() {
    let source = parsed(&fixture("segmented_local_array.bas"));
    assert!(assembled(&source).is_ok());
}

/// common's g.env.cam_script formerly reached selection as ``lea
/// [abs+offset]``.
#[test]
fn test_byref_fixed_string_field_forms_far_offset_without_absolute_lea() {
    let source = parsed(&fixture("byref_fixed_string_field.bas"));
    assert!(assembled(&source).is_ok());
}

/// common's VAL result formerly vanished because its following load named no
/// MIR use.
#[test]
fn test_runtime_pointer_result_used_as_memory_base_is_an_explicit_mir_use() {
    let text = emitted_mir(&parsed(&fixture("val.bas")));
    let results: Vec<&str> = text
        .lines()
        .filter(|line| line.contains(" = call ") && line.contains("@llrm.qb.B$FVAL("))
        .map(|line| line.trim().split(' ').next().unwrap())
        .collect();
    assert!(!results.is_empty());
    for result in results {
        assert!(text.contains(&format!("ptr {result}\n")) || text.contains(&format!("ptr {result},")), "{text}");
    }
}

/// ent copied VEC3 as a fictitious 12-byte register before aggregate lowering.
#[test]
fn test_udt_assignment_is_scalar_memory_copy_not_wide_register_value() {
    let text = emitted_mir(&parsed(&fixture("aggregate_copy.bas")));
    assert!(!regex::Regex::new(r"\bi(9[6-9]|1[0-9][0-9])\b").unwrap().is_match(&text), "{text}");
}

/// screen passed extended values directly to a SINGLE-by-value UGL call.
#[test]
fn test_byval_float_is_stored_at_declared_width_before_stack_push() {
    let source = parsed(&fixture("byval_float.bas"));
    assert!(assembled(&source).is_ok());
}

/// ent.bas reached B$RDIM with stack arguments but an object-raiser GP liveness
/// contract.
#[test]
fn test_redim_stack_contract_uses_typed_rank_cleanup_not_register_arguments() {
    let directory = tempfile::tempdir().expect("a directory");
    let source =
        written(&directory, "REDIM.BAS", b"'$DYNAMIC\r\nDIM a(5) AS INTEGER\r\nREDIM a(9) AS INTEGER\r\nERASE a\r\n");
    let assembly = listing(&parsed(&source));
    // The runtime pops its own arguments.
    let after = assembly.split_once("call far ptr B$RDIM\n").expect("B$RDIM").1;
    assert!(!after.lines().next().unwrap().contains("add sp"), "{assembly}");
}

/// r_bsp reached B$ERAS. VBDOS's erase.asm is QB 4.5's, so it shares the
/// established OWN contract.
#[test]
fn test_vbdos_erase_uses_typed_stack_call_with_the_established_contract() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "ERASE.BAS", b"'$DYNAMIC\r\nDIM a(5) AS INTEGER\r\nERASE a\r\nPRINT 1\r\n");
    let assembly = listing(&parsed(&source));
    let after = assembly.split_once("call far ptr B$ERAS\n").expect("B$ERAS").1;
    assert!(!after.lines().next().unwrap().contains("add sp"), "{assembly}");
}

/// d_poly's SIN must stay an inline float value, not become B$SIN or cross
/// CALL.
#[test]
fn test_qb_inline_sin_is_inline_x87_without_a_runtime_call() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "TRIG.BAS",
        b"DECLARE SUB wave (x AS SINGLE)\r\nwave 1\r\nSUB wave (x AS SINGLE)\r\nPRINT SIN(x)\r\nEND SUB\r\n",
    );
    let program = parsed(&source);
    // Recognition belongs at the HIR -> MIR boundary: SIN is pure math,
    // not an opaque call that bars every optimizer.
    assert!(!emitted_mir(&program).contains("B$SIN"));
    let assembly = listing(&program);
    let wave = between(&assembly, "WAVE proc far", "WAVE endp");
    assert!(wave.contains("db 0d9h,0feh"), "{assembly}");
    assert!(!wave.contains("B$SIN"), "{assembly}");
}

/// A fresh QB procedure must end in RETF n; semantic MIR carries no stack ABI
/// bytes.
#[test]
fn test_qb_finalizer_attaches_callee_cleanup_to_far_return() {
    let returned = lir::Insn::new(
        1,
        Some((1, 1)),
        Some(ir::Semantics { name: Some("".into()), ..ir::Semantics::new(Operation::Return) }),
        vec![],
        vec![],
    );
    let body = lir::LirBody::new(
        "callee",
        1,
        vec![lir::LirBlock::new(1, vec![Arc::new(returned)]).into()],
        IndexMap::default(),
        IndexMap::default(),
    );
    let last = finalized(&body, 6).expect("finalizes");
    let returned = last.body.insns()[0].what.clone().expect("a return");
    assert_eq!(returned.sources, vec![Loc::Imm(ir::Imm { value: 6, width: 2, address: None })]);
}

/// RPOINTLEAF treated readonly RPLANEDIST as a write to every descriptor.
#[test]
fn test_qb_module_instantiates_user_callee_modref_on_pointer_actuals() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "MODREF.BAS",
        b"DECLARE SUB rd (p AS INTEGER)\r\nDIM SHARED total AS INTEGER\r\nDIM v AS INTEGER\r\nv = 1\r\nrd v\r\nPRINT v\r\n\
SUB rd (p AS INTEGER)\r\ntotal = p\r\nEND SUB\r\n",
    );
    let read = function_text(&optimized_mir(&parsed(&source)), "RD");
    let header = read.lines().next().unwrap();

    assert!(header.contains("readonly") && header.contains("argmem: read"), "{read}");
}

/// SCMPABI's B$SCMP ABI site was dropped because STRING_EQ is not Op.CALL.
#[test]
fn test_qb_string_comparison_abi_site_survives_alias_annotation() {
    let source = parsed(&fixture("scmpabi.bas"));
    let (_, function) = function_named(&source, "MATCHES");
    let instruction = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|one| one.id == function.calls[0].instruction)
        .expect("the site");

    assert_eq!(instruction.op, hir::Op::StringEq);
    assert!(listing(&source).contains("call far ptr B$SCMP"));
}

/// IN_KEYSTROKE held a released key forever after GVN kept its first read.
///
/// A BYREF pointee declared `VOLATILE` may be changed by an interrupt or
/// another runtime callback without a source store in the function.  Both
/// the guard and the back-edge condition must therefore remain observable
/// loads.
#[test]
fn test_a_volatile_byref_loop_condition_reloads_the_pointee() {
    let source = parsed_as(&fixture("byreflp.bas"), "vbdos", "vbdos");
    let text = function_text(&optimized_mir(&source), "WAITKEY");
    let loads: Vec<&str> = text.lines().filter(|line| line.contains(" = load ")).collect();

    assert_eq!(loads.len(), 2, "{text}");
    assert!(loads.iter().all(|line| line.contains("load volatile")), "{text}");
    // The guard and the back-edge condition, each in its own block.
    assert_eq!(blocks(&text).iter().filter(|block| block.contains("load volatile")).count(), 2, "{text}");
}

/// Every iteration of a counted loop reloaded its BYREF arguments, as if the
/// loop waited on them: TEXTFILL's `Fill` read `ch` and `at` 2000 times. A
/// BYREF not declared `VOLATILE` is read once and is not ordered.
#[test]
fn test_a_counted_loop_reads_an_unannotated_byref_once() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FILLBYREF.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB Fill (ch)\r\nFill 65\r\nSUB Fill (ch)\r\nDEF SEG = &HB800\r\nFOR o = 0 TO 3998 STEP 2\r\nPOKE o, ch\r\nNEXT\r\nEND SUB\r\n",
    );
    let source = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = function_text(&optimized_mir(&source), "FILL");
    let loop_ = poke_block(&text);

    assert!(!text.contains("load volatile"), "{text}");
    // `ch` is one plain load per trip, as the pin leaves it: a volatile far
    // store may reach it (4f82e22e1, #3994). Not a wait on it: no volatile.
    assert_eq!(loop_.matches(" = load ").count(), 1, "{text}");
    assert!(loop_.contains("store volatile"), "the POKE still lands: {text}");
}

/// ENTPHI lost a dynamic-array address after its identity phi edge vanished.
///
/// The source's `CASE 0, 1` join reaches a field through an address value
/// which has the same physical register on both edges.  Its copies therefore
/// emit nothing, but their virtual definition must survive until all LIR
/// control-flow threading is complete.
#[test]
fn test_identity_phi_edge_survives_control_flow_threading() {
    let source = parsed_with(&fixture("entphi.bas"), "vbdos", "vbdos", "row-major", false, false);
    assert!(!object_bytes(&source, "ENTPHI.BAS").expect("emits").is_empty());
}

/// `--whole-program` never reached qbfront, so every SUB stayed public.
#[test]
fn test_whole_program_procedures_are_not_public() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "WHOLE.BAS", b"DECLARE SUB s ()\nCALL s\nSUB s\nEND SUB\n");
    let public = |whole_program| {
        let frontend = qb_driver::Frontend { whole_program, ..qb_driver::Frontend::new("qb45", "qb45") };
        listing(&qb_driver::parsed(&source, &frontend, None).expect("parses")).contains("public S\n")
    };
    assert!(public(false));
    assert!(!public(true));
}

/// `1 <= n` lowered to `cmp 1, bx`, which x86 cannot encode; UBOUND made it on
/// every array.
#[test]
fn test_a_constant_on_the_left_of_a_comparison_still_encodes() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "LEFT.BAS",
        b"DEFINT A-Z\nDECLARE SUB Disp (n)\nDisp 3\nSUB Disp (n)\n  IF 1 <= n THEN PRINT \"YES\"\nEND SUB\n",
    );
    let program = parsed(&source);

    assert!(!object_bytes(&program, "LEFT.BAS").expect("emits").is_empty());
    assert!(!listing(&program).contains("cmp 1,"));
}

/// sumThree reloaded three descriptors and stored `total` after every add.
///
/// Neither the parameter pointers nor B$UBND can reach a static whose
/// address the module never hands out.
#[test]
fn test_array_parameters_do_not_pin_private_statics_inside_their_loop() {
    let procedure = sum_three(false);
    let loop_ = &backward_loop(&procedure);

    assert!(
        !["bx", "si", "di"]
            .iter()
            .any(|base| ["2", "10"].iter().any(|disp| loop_.contains(&format!("word ptr [{base}+{disp}]"))))
    );
    assert!(static_stores(loop_) <= 2); // total and index, once each
    assert!(!loop_.contains("call"));
}

/// Every UBOUND called B$UBND, whose unknown writes pinned all memory around
/// it.
///
/// The descriptor holds the bounds; the call remains only where the runtime
/// would raise "Subscript out of range".
#[test]
fn test_array_bounds_are_read_from_the_descriptor() {
    let procedure = sum_three(false);

    assert!(has_indexed_field(&procedure, "12"), "{procedure}");
    assert!(has_indexed_field(&procedure, "10"), "{procedure}");
}

/// sumThree stored its STATIC `total` and `index` on every iteration.
///
/// Proving the cells held the loop's entry values walked back through the
/// B$LBND fallback and refused every call, although that call's modelled
/// effects cannot reach a private static.
#[test]
fn test_static_locals_are_stored_once_after_the_loop() {
    let procedure = sum_three(false);
    assert!(!backward_loop(&procedure).contains("SUM_THREE$D"));
}

/// LBOUND's runtime call was the fall-through; the descriptor read sat behind
/// three jumps.
///
/// The call only raises "Subscript out of range", so the frontend marks its
/// block cold and layout places it after the return.
#[test]
fn test_the_bound_error_call_is_placed_after_the_hot_path() {
    let procedure = sum_three(true);

    let returned = procedure.find("retf").expect("retf");
    assert!(procedure.find("B$LBND").expect("B$LBND") > returned);
    assert!(procedure.find("B$UBND").expect("B$UBND") > returned);
}

/// An ELSE that only raises an error was laid out before the return.
///
/// Nothing marks it cold: B$SERR never returns, and that is enough.
#[test]
fn test_an_error_statement_is_placed_after_the_hot_path() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "RAISE.BAS",
        b"DEFINT A-Z\nDECLARE SUB Check (n)\nCheck 3\nSUB Check (n)\n  IF n >= 0 THEN\n    PRINT n\n  ELSE\n    ERROR 5\n  END IF\nEND SUB\n",
    );
    let text = listing(&parsed(&source));
    let procedure = &text[text.find("CHECK proc").expect("CHECK proc")..text.find("CHECK endp").expect("CHECK endp")];

    assert!(procedure.find("B$SERR").expect("B$SERR") > procedure.find("retf").expect("retf"));
}

/// LBOUND(a, 1) compared the rank against 1 before reading the descriptor.
///
/// Every allocated array has a first dimension, so only the allocation
/// test (`cmp word ptr [descriptor+2], 0`) may branch to a runtime call: the
/// test before each such branch is that compare, and none is the rank's.
#[test]
fn test_the_first_dimension_is_not_rank_checked() {
    let procedure = sum_three(true);

    let calls = bound_call_labels(&procedure);
    let tests: BTreeSet<&str> = jumps(&procedure, false)
        .iter()
        .filter(|(_, _, label)| calls.contains(label))
        .map(|(start, _, _)| {
            // The compare may be followed by flag-free moves before its branch.
            procedure[..*start].lines().rev().map(str::trim).find(|line| line.starts_with("cmp ")).unwrap_or("")
        })
        .collect();
    assert!(!tests.is_empty() && tests.iter().all(|test| test.ends_with("+2], 0")), "{tests:?}");
}

/// Without -fsanitize=bounds LBOUND trusts the descriptor: no B$LBND/B$UBND
/// fallback.
#[test]
fn test_unchecked_bounds_read_the_descriptor_without_runtime_calls() {
    let procedure = sum_three(false);

    assert!(!procedure.contains("B$LBND") && !procedure.contains("B$UBND"));
    assert!(has_indexed_field(&procedure, "12"), "{procedure}");
}

/// OUT/POKE of a SINGLE raised Unlowered (no one-byte fistp), and the
/// listing printed `out dx` / `in al` without their second operand.
#[test]
fn test_port_io_narrows_a_float_through_integer_and_prints_both_operands() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "PORTS.BAS",
        b"defint a-z\r\np = &H3C8: f! = 41.6\r\nout p, f!\r\npoke 0, f!\r\na = inp(p + 1)\r\n",
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let lines: BTreeSet<String> = stripped_lines(&listing(&source)).into_iter().collect();
    assert!(lines.contains("out dx, al") && lines.contains("in al, dx"), "{lines:?}");
    assert!(lines.iter().any(|line| line.starts_with("fistp word ptr")), "{lines:?}");
}

/// oimad's OPEN was refused: B$OPEN had no audited stack effect (RETF 8).
#[test]
fn test_open_compiles_with_its_callee_cleaned_arguments() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "OPENS.BAS", b"open \"DATA.DAT\" for binary as #1\r\nclose #1\r\n");
    let source = parsed_as(&basic, "qb45", "qb45");
    assert!(listing(&source).contains("B$OPEN"));
}

/// oimad's bare RND was refused: B$RND0 had no audited stack effect.
#[test]
fn test_rnd_without_an_argument_compiles() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "RND0.BAS", b"x! = rnd\r\nprint x!\r\n");
    let source = parsed_as(&basic, "qb45", "qb45");
    assert!(listing(&source).contains("B$RND0"));
}

/// oimad froze after a few hundred frames: CIRCLE pushed its radius twice,
/// four stack bytes B$CIRC never pops. QB45 circle.asm and the VBDOS /A
/// listing both take one parmD radius and one color word.
#[test]
fn test_circle_pushes_one_radius() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "CIRC.BAS", b"screen 13\r\nr! = 16\r\ncircle (10, 100), r!, 5\r\n");
    let source = parsed_as(&basic, "qb45", "qb45");
    let lines = stripped_lines(&listing(&source));
    let start = lines.iter().position(|line| line == "call far ptr B$N1I2").expect("B$N1I2");
    let end = lines.iter().position(|line| line == "call far ptr B$CIRC").expect("B$CIRC");
    let mut pushed = 0;
    for line in &lines[start + 1..end] {
        let (mnemonic, operand) = line.split_once(' ').unwrap_or((line, ""));
        pushed += match mnemonic {
            "pushd" => 4,
            "pushw" => 2,
            "push" if operand.starts_with("dword") || operand.starts_with('e') => 4,
            "push" => 2,
            _ => 0,
        };
    }
    assert_eq!(pushed, 6, "{:?}", &lines[start..=end]);
}

/// Qlight printed 3492255: 1000000 was lexed as INTEGER 0x4240 and
/// sign-extended.
#[test]
fn test_an_unsuffixed_decimal_above_32767_is_a_long_literal() {
    let directory = tempfile::TempDir::new().unwrap();
    let (program, function) = function_in(
        &directory,
        "scale.bas",
        "function qlightScale& (word as integer)\nqlightScale& = clng(word) * 1000000\nend function\n",
        "QLIGHTSCALE&",
        "qb45",
    );
    let widths: IndexMap<i64, i64> =
        program.modules.iter().flat_map(|module| &module.types).map(|one| (one.id, one.width)).collect();
    let constants: Vec<&hir::Constant> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .flat_map(|instruction| &instruction.operands)
        .filter_map(|operand| match operand {
            Operand::Constant(constant)
                if matches!(constant.value, hir::Number::Int(1_000_000))
                    || matches!(
                        constant.value,
                        hir::Number::Float(value) if value == 1_000_000.0
                    ) =>
            {
                Some(constant)
            }
            _ => None,
        })
        .collect();

    assert!(!constants.is_empty() && constants.iter().all(|one| widths[&one.r#type] == 4));
}

/// Loading TOTAL before B$ERAS let the cleanup call clobber the returned value.
#[test]
fn test_a_long_function_reloads_its_result_after_erasing_local_arrays() {
    let directory = tempfile::TempDir::new().unwrap();
    let (_, function) = function_in(
        &directory,
        "total.bas",
        "function total as long\ndim cells(0 to 0) as integer\ntotal = 42\nend function\n",
        "TOTAL",
        "vbdos",
    );
    let result = function.places.iter().find(|one| one.name == "TOTAL").expect("TOTAL").id;
    let exit = value_exit(&function);
    let calls = callees(exit);
    let loads: Vec<usize> = exit
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, one)| one.op == hir::Op::Load && one.operands == [Operand::place_ref(result)])
        .map(|(at, _)| at)
        .collect();

    assert!(!loads.is_empty() && position(&calls, "B$ERAS") < *loads.last().unwrap());
}

/// The scalar reload must not reorder STRING results: B$SCPF runs before B$STDL
/// frees OTHER.
#[test]
fn test_a_string_function_copies_its_result_before_freeing_other_locals() {
    let directory = tempfile::TempDir::new().unwrap();
    let (_, function) = function_in(
        &directory,
        "pick.bas",
        "function pick as string\ndim other as string\nother = \"kept\"\npick = other\nend function\n",
        "PICK",
        "vbdos",
    );
    let exit = value_exit(&function);
    let calls = callees(exit);
    let copied = &exit.instructions[position(&calls, "B$SCPF")];

    assert!(position(&calls, "B$SCPF") < position(&calls, "B$STDL"));
    assert_eq!(exit.terminator.operands, [Operand::value_ref(copied.results[0])]);
}

// ---- the stage dump ------------------------------------------------------

fn capture_program() -> hir::Program {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let body = returning(1, vec![]);
    let function = hir::Function::new(1, "__main", 0, vec![], vec![], vec![body], 1);
    let statements = hir::DataObject { readonly: true, ..hir::DataObject::new(1, "$qb$statementTable", vec![]) };
    let module = hir::Module { data: vec![statements], ..hir::Module::new(1, "capture", vec![void], vec![function]) };
    vbdos(module)
}

/// qbstages used to lower manually, then assemble the same HIR again for its
/// final listing.
#[test]
fn test_stage_observer_sees_the_hir_once_and_preserves_object_bytes() {
    let program = capture_program();
    let codegen = llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone());
    let uncaptured = qb_compile::object_bytes(&program, Path::new("capture.bas"), None, &codegen).expect("emits");
    let mut seen: Vec<hir::Program> = Vec::new();
    let mut observer = |program: &hir::Program| {
        seen.push(program.clone());
        Ok(())
    };
    let captured =
        qb_compile::object_bytes(&program, Path::new("capture.bas"), Some(&mut observer), &codegen).expect("emits");

    assert_eq!(captured, uncaptured);
    assert_eq!(seen, [program]);
}

// ---- tests/test_qb_frontend_command.py -----------------------------------

/// Adding nib/freestanding to common HIR once made QB's driver advertise both.
#[test]
fn test_common_hir_profiles_do_not_become_qb_frontend_options() {
    let source = qb_driver::ROOT().join("not-read.bas");
    let syntax = |dialect: &str, runtime: &str| {
        qb_driver::syntax_checked(&source, &qb_driver::Frontend::new(dialect, runtime)).expect_err("refused").0
    };
    assert!(syntax("nib", "vbdos").contains("unknown QB dialect"));
    assert!(syntax("vbdos", "freestanding").contains("unknown QB runtime"));
}

const ROW_LOOP: &str = "DEFINT A-Z\r\nDECLARE SUB blit ()\r\nDIM SHARED sp(100), yy(3), xp(3), cd(100)\r\nblit\r\n\
SUB blit\r\nyp = 0\r\nFOR y = 0 TO 199\r\nDEF SEG = &HA000\r\nyp = yp + 20\r\nFOR x = 24 TO 295\r\n\
dn = sp(yy(1) + x - xp(1)) + sp(yy(2) + x - xp(2))\r\nPOKE x, cd(dn)\r\nNEXT x\r\nNEXT y\r\nDEF SEG\r\nEND SUB\r\n";

/// `name` optimized after the module's call memory effects, as compiled.
fn optimized_sub(
    text: &str,
    name: &str,
) -> String {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let source = parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45");
    function_text(&optimized_mir(&source), name)
}

fn optimized_blit(text: &str) -> String {
    optimized_sub(text, "BLIT")
}

/// `name`'s definition in the module's MIR text.
fn function_text(
    module: &str,
    name: &str,
) -> String {
    let start = module
        .lines()
        .position(|line| line.starts_with("define ") && line.contains(&format!("@{name}(")))
        .unwrap_or_else(|| panic!("no {name}\n{module}"));
    let lines: Vec<&str> = module.lines().skip(start).collect();
    let end = lines.iter().position(|line| *line == "}").expect("the closing brace");
    lines[..=end].join("\n")
}

/// Each basic block of a function's text.
fn blocks(function: &str) -> Vec<&str> {
    function.split("\n\n").collect()
}

/// The block holding the function's first POKE.
fn poke_block(function: &str) -> &str {
    blocks(function).into_iter().find(|block| block.contains("store volatile i8")).expect("a POKE")
}

/// The definition of `value` in `function`: its right-hand side.
fn definition<'t>(
    function: &'t str,
    value: &str,
) -> Option<&'t str> {
    function.lines().find_map(|line| line.trim().strip_prefix(&format!("{value} = ")))
}

/// For each POKE in `function`, whether its selector is reloaded from b$seg.
fn far_selectors_loaded(function: &str) -> Vec<bool> {
    let pointer = regex::Regex::new(r"(%[\w.]+)").expect("a pattern");
    function
        .lines()
        .filter_map(|line| line.trim().strip_prefix("store volatile i8 "))
        .map(|store| {
            // A constant address reloads nothing.
            let Some(address) = pointer.captures_iter(store).last() else { return false };
            let mut value = address[1].to_owned();
            // A phi can lead back to itself: stop on a name seen.
            let mut seen = BTreeSet::new();
            while seen.insert(value.clone())
                && let Some(defined) = definition(function, &value)
            {
                if defined.starts_with("load i16, ptr @b$seg") {
                    return true;
                }
                let Some(operand) = pointer.captures(defined.split_once(' ').map_or(defined, |(_, rest)| rest)) else {
                    return false;
                };
                value = operand[1].to_owned();
            }
            false
        })
        .collect()
}

/// Loads in the POKE's block whose address the block does not compute.
fn invariant_loads(function: &str) -> Vec<String> {
    let block = poke_block(function);
    let defined: BTreeSet<&str> =
        block.lines().filter_map(|line| line.trim().split_once(" = ")).map(|(name, _)| name).collect();
    block
        .lines()
        .filter(|line| line.contains(" = load "))
        .filter(|line| {
            let address = line
                .split(", ptr")
                .nth(1)
                .and_then(|rest| rest.split(',').next())
                .map(|one| one.trim().rsplit(' ').next().unwrap_or(""));
            address.is_some_and(|one| !defined.contains(one))
        })
        .map(str::to_owned)
        .collect()
}

/// deedlines' cycleblobs kept every local in memory: its POKE went through a
/// far pointer, and the frontend marked each local as addressed though none
/// had its address taken, so the store clobbered the loop counters.
#[test]
fn test_a_poke_does_not_reach_a_local_whose_address_is_never_taken() {
    let text = optimized_blit(ROW_LOOP);
    assert!(!text.contains("alloca"), "{text}");
}

/// The range check of EXP and LOG kept its operand in a cell of the operand's
/// own type, rounding an 80-bit chain to 64 bits before FEXP/FLN:
/// log_exp_digits printed .9048374180359595 for BC's ...596 (D-RFP5 again). The
/// operand goes from its arithmetic to FEXP/FLN, past the check, without a
/// cell.
#[test]
fn test_a_range_checked_operand_is_not_kept_in_a_cell() {
    use llrm_codegen::hir::model::{Op, Operand};
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "CHAIN.BAS",
        b"DIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE\r\nREAD a, b, c\r\nPRINT EXP(a * b + c); LOG(a * b + c)\r\nDATA 1.5, 2.5, 0.25\r\n",
    );
    let program = parsed_as(&basic, "qb45", "qb45");
    let function = program.modules[0].functions.iter().find(|one| one.name == "__main").expect("the main code");
    let instructions: Vec<_> = function.blocks.iter().flat_map(|block| &block.instructions).collect();
    let mut checked = 0;
    for one in instructions.iter().filter(|one| matches!(one.op, Op::Fexp | Op::Fln)) {
        let Some(Operand::ValueRef(argument)) = one.operands.first() else { panic!("FEXP/FLN of a value") };
        let made = instructions.iter().find(|other| other.results.contains(&argument.value)).expect("its definition");
        assert_eq!(made.op, Op::Fadd, "the operand was kept in a cell: {made:?}");
        checked += 1;
    }
    assert_eq!(checked, 2);
}

/// Rotation panicked ("stepping is not in list") on a loop whose counter
/// update is not in the latch block.
#[test]
fn test_rotation_skips_a_loop_whose_update_is_outside_the_latch() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let text = ROW_LOOP.replace("POKE x, cd(dn)", "sp(x - 200) = cd(dn)");
    records(&parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45"), "T.BAS");
}

/// Every POKE reloaded the DEF SEG word from b$seg: the far store might have
/// written b$seg, though the runtime never hands its address out. That hid
/// the segment from range analysis and kept the loop's globals in memory.
#[test]
fn test_a_poke_takes_its_segment_from_the_def_seg_before_it() {
    assert_eq!(far_selectors_loaded(&optimized_blit(ROW_LOOP)), [false]);
}

/// A call that may run DEF SEG must still make the next POKE reload b$seg.
#[test]
fn test_a_poke_after_a_call_reloads_its_segment() {
    let text = "DECLARE SUB a ()\r\nDECLARE SUB b ()\r\na\r\n\
SUB a\r\nDEF SEG = &HA000\r\nPOKE 1, 2\r\nb\r\nPOKE 3, 4\r\nEND SUB\r\n\
SUB b\r\nDEF SEG\r\nEND SUB\r\n";
    assert_eq!(far_selectors_loaded(&optimized_sub(text, "A")), [false, true]);
}

/// Every runtime call counted as a DEF SEG, so PLASMA's POKE reloaded b$seg
/// after INKEY$ and never saw &HA000. Only b$seg's listed writers write it.
#[test]
#[ignore = "row QB-POKE-1: ForgedSelectors after forwarding"]
fn test_a_poke_after_a_call_that_never_runs_def_seg_keeps_its_segment() {
    let text = "DECLARE SUB a ()\r\nDECLARE SUB b ()\r\na\r\n\
SUB a\r\nDEF SEG = &HA000\r\nPOKE 1, 2\r\nk$ = INKEY$\r\nPOKE 3, 4\r\nb\r\nPOKE 5, 6\r\nEND SUB\r\n\
SUB b\r\nPRINT \"b\"\r\nEND SUB\r\n";
    assert_eq!(far_selectors_loaded(&optimized_sub(text, "A")), [false, false, false]);
}

/// Code this module cannot see may run DEF SEG: another module's SUB, or an
/// error handler entered from inside a runtime call. Keeping the segment
/// across either POKEs the wrong memory.
#[test]
fn test_a_poke_after_code_that_may_run_def_seg_reloads_its_segment() {
    let elsewhere = "DECLARE SUB a ()\r\nDECLARE SUB other ()\r\na\r\n\
SUB a\r\nDEF SEG = &HA000\r\nPOKE 1, 2\r\nother\r\nPOKE 3, 4\r\nEND SUB\r\n";
    assert_eq!(far_selectors_loaded(&optimized_sub(elsewhere, "A")), [false, true]);
    let handled = "DECLARE SUB a ()\r\nON ERROR GOTO fail\r\na\r\nEND\r\nfail:\r\nDEF SEG = 0\r\nRESUME NEXT\r\n\
SUB a\r\nDEF SEG = &HA000\r\nPOKE 1, 2\r\nk$ = INKEY$\r\nPOKE 3, 4\r\nEND SUB\r\n";
    assert_eq!(far_selectors_loaded(&optimized_sub(handled, "A")), [false, true]);
}

/// Each function names b$seg through its own place. Keeping one object per
/// name lost the caller's, so fractaleffect's DEF SEG before fracline, which
/// PEEKs through it, was deleted as a dead store.
#[test]
fn test_a_def_seg_before_a_sub_that_peeks_is_kept() {
    let text = "DECLARE SUB a ()\r\nDECLARE SUB b ()\r\nDIM SHARED arr(10)\r\na\r\n\
SUB a\r\narr(1) = 2\r\nDEF SEG = &HA000\r\nb\r\nDEF SEG = 0\r\nb\r\nEND SUB\r\n\
SUB b\r\nPRINT PEEK(1)\r\nEND SUB\r\n";
    let optimized = optimized_sub(text, "A");
    assert!(optimized.contains("store i16 -24576, ptr @b$seg"), "{optimized}");
}

/// Each function numbered its own places, and a global's object was named by
/// that number, so b read a different b$seg than a wrote. a's DEF SEG before
/// calling b was deleted as dead, and b POKEd through a stale segment.
#[test]
fn test_a_global_is_one_object_in_every_function() {
    let text = "DECLARE SUB a ()\r\nDECLARE SUB b ()\r\nDIM SHARED arr%(100)\r\na\r\n\
SUB a\r\nn% = 5\r\nDEF SEG = VARSEG(arr%(0))\r\nb\r\nDEF SEG = &HA000\r\nPOKE 1, n%\r\nEND SUB\r\n\
SUB b\r\nPOKE 5, PEEK(4)\r\nEND SUB\r\n";
    let optimized = optimized_sub(text, "A");
    // b inlined, its PEEK reads through the segment a stored for it: VARSEG's,
    // not b's own stale b$seg. That store is then dead, a's last one the
    // only one kept.
    let loads = optimized.lines().filter(|line| line.contains("load i16") && line.contains("ptr @b$seg")).count();
    assert_eq!(loads, 0, "{optimized}");
    let kept =
        optimized.lines().filter(|line| line.trim().starts_with("store i16 ") && line.contains("ptr @b$seg")).count();
    // VARSEG's selector is a constant address, but the PEEK/POKE offsets are
    // relative to that segment's base, not to ARR%: they can reach any byte of
    // the data segment, b$seg included. A forged-selector pointer is therefore
    // no proven-disjoint access (188fd3905, DEVIATE C1, forging.rs), and a's
    // first b$seg store is kept, as the second is (+1 store, +3 instructions
    // against the earlier expectation that it was dead).
    assert!(optimized.contains("addrspacecast (ptr @\"ARR%\"") && kept == 2, "{optimized}");
}

#[test]
fn test_bases_past_the_registers_are_not_stepped_as_pointers() {
    // CYCLEBLOBS stepped one pointer per term; five spilled, each an
    // `add [bp-N],2` per pixel where its invariant base cost nothing.
    let text = optimized_sub(SEVEN_TERMS, "T");
    let steps = poke_block(&text).lines().filter(|line| line.contains(" = phi ")).count();
    assert!(steps <= 1, "{steps} pointer steps\n{text}");
}

const SEVEN_TERMS: &str = "DEFINT A-Z\r\nDECLARE SUB t ()\r\nDIM SHARED sp(8000), f(700), xp(7), yy(7), cd(2000)\r\nk = LEN(COMMAND$)\r\n\
sp(k) = 1: f(k) = 1: xp(k) = 1: yy(k) = 1: cd(k) = 1\r\nt\r\nSUB t\r\n\
DEF SEG = &HA000\r\nFOR x = 24 TO 295\r\ndn = sp(yy(1) + f(x - xp(1))) + sp(yy(2) + f(x - xp(2))) + \
sp(yy(3) + f(x - xp(3))) + sp(yy(4) + f(x - xp(4))) + sp(yy(5) + f(x - xp(5))) + sp(yy(6) + f(x - xp(6))) + \
sp(yy(7) + f(x - xp(7)))\r\nPOKE x, cd(dn)\r\nNEXT\r\nEND SUB\r\n";

/// On the default route CYCLEBLOBS stepped one pointer per `f(x - xp(k))`,
/// six of them in memory, an `add [bp-N], 2` each per pixel: no address was
/// spelled as its invariant base indexed by one shared `x * 2`.
#[test]
fn test_affine_addresses_of_one_counter_step_no_pointer_in_memory() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "T.BAS", SEVEN_TERMS.as_bytes());
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "T");
    assert!(!regex::Regex::new(r"add\s+word ptr \[bp-\d+\]").unwrap().is_match(&inner), "{inner}");
}

/// The seven terms shift one counter alike, `lea di, [esi+esi]` seven times in
/// the loop: MachineCSE kept them because its walk for a reader of the flags
/// gave up at an indexed load (#3075).
#[test]
fn test_the_shift_of_one_counter_is_made_once_per_trip() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "T.BAS", SEVEN_TERMS.as_bytes());
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "T");
    let doubled = regex::Regex::new(r"lea\s+\w+, \[(\w+)\+(\w+)\]").unwrap();
    let mut seen = llrm_support::hash::HashMap::default();
    for line in inner.lines().filter(|line| doubled.captures(line).is_some_and(|found| found[1] == found[2])) {
        *seen.entry(line.trim().to_owned()).or_insert(0) += 1;
    }
    let repeats = seen.values().copied().max().unwrap_or(0);
    assert!(repeats <= 1, "one doubling made {repeats} times\n{inner}");
}

/// cycleblobs computed each `f(x - xp(k))` index as `(x - xp(k)) * 2` per
/// pixel: seven subtracts and seven multiplies of the counter, where the
/// counter scaled once per term and a hoisted `-2 * xp(k)` base does, on the
/// one counter (`opt -passes=loop-reduce`, LLVM 20: one phi, seven `shl x, 1`;
/// the pin runs no IR CSE after LSR, SelectionDAG's merges them).
#[test]
fn test_affine_terms_of_one_counter_share_its_scaled_value() {
    let text = optimized_sub(SEVEN_TERMS, "T");
    let body = poke_block(&text);
    let counter =
        regex::Regex::new(r"(%[\w.]+) = phi ").expect("a pattern").captures(body).unwrap_or_else(|| panic!("{text}"))
            [1]
        .to_owned();
    let scaled = body
        .lines()
        .filter(|line| (line.contains(" = shl ") || line.contains(" = mul ")) && line.contains(&format!("{counter},")))
        .count();
    // One counter, as the pin's loop-reduce leaves it (`opt
    // -passes=loop-reduce` on the same IR, i386/i486: one phi, seven `shl
    // x, 1`; it runs no IR CSE after LSR, SelectionDAG's merges them, as
    // the other test checks in the listing).
    let phis = body.lines().filter(|line| line.contains(" = phi ")).count();
    assert_eq!((phis, scaled), (1, 7), "{text}");
}

/// A POKE into VGA memory counted as reaching a local array's descriptor, so
/// PLASMA reloaded three descriptors per pixel: the store carried provenance,
/// and provenance never asked where its segment points.
#[test]
fn test_a_poke_to_video_memory_leaves_a_local_arrays_descriptor_hoisted() {
    let text = optimized_sub(
        "DEFINT A-Z\r\nDECLARE SUB blit ()\r\nblit\r\nSUB blit\r\nDIM a(320)\r\nDEF SEG = &HA000\r\n\
FOR y = 0 TO 199\r\nFOR x = 0 TO 319\r\nPOKE x, a(x)\r\nNEXT x\r\nNEXT y\r\nEND SUB\r\n",
        "BLIT",
    );
    assert!(invariant_loads(&text).is_empty(), "{text}");
}

/// A POKE into VGA memory counted as reaching every global, so the row loop
/// reloaded yy() and xp() per pixel. A segment the machine keeps no program
/// data in reaches none.
#[test]
fn test_a_poke_to_video_memory_leaves_invariant_globals_hoisted() {
    let text = optimized_blit(ROW_LOOP);
    assert!(invariant_loads(&text).is_empty(), "{text}");
}

/// The listing of `name`'s loop around its POKE: its label through its back
/// edge.
fn poke_loop(
    text: &str,
    name: &str,
) -> String {
    loop_around(text, name, |line| line.contains("mov") && line.contains("byte ptr") && line.contains(":["))
}

/// The loop in `name` around the first line `marks`.
fn loop_around(
    text: &str,
    name: &str,
    marks: impl Fn(&str) -> bool,
) -> String {
    let function = between(text, &format!("{name} proc"), &format!("{name} endp"));
    let lines = function.lines().collect::<Vec<_>>();
    let store = lines.iter().position(|line| marks(line)).expect("the marked line");
    let top = lines[..store].iter().rposition(|line| line.ends_with(':')).expect("the loop's label");
    let label = lines[top].trim_end_matches(':');
    let bottom = store
        + lines[store..]
            .iter()
            .position(|line| line.trim_start().starts_with('j') && line.ends_with(label))
            .expect("the back edge");
    lines[top..=bottom].join("\n")
}

/// deedlines' RGBLIGHTS, CYCLEBLOBS and PLASMABLOBS reloaded every
/// invariant global per pixel: a POKE through `DEF SEG = &HA000 + yp`
/// counted as reaching all of DGROUP, since only a constant selector was
/// placed in video memory, not one the row counter's range bounds.
#[test]
fn test_a_poke_through_a_counted_selector_leaves_invariant_globals_hoisted() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(&directory, "T.BAS", ROW_LOOP.as_bytes());
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "BLIT");
    assert!(!inner.contains("YY%") && !inner.contains("XP%"), "{inner}");
}

/// A POKE of the BIOS data area counted as reaching every global, so each
/// shared element the loop reads was reloaded every iteration.
#[test]
fn test_a_poke_of_the_bios_data_area_leaves_invariant_globals_hoisted() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "T.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB t ()\r\nDIM SHARED yy(3), xp(3)\r\nt\r\nSUB t\r\nDEF SEG = &H40\r\n\
FOR x = 0 TO 99\r\nPOKE &H17, yy(1) + xp(2) + x\r\nNEXT\r\nDEF SEG\r\nEND SUB\r\n",
    );
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "T");
    assert!(!inner.contains("YY%") && !inner.contains("XP%"), "{inner}");
}

/// `t`'s loop: OUT to `port` a shared element plus the counter.
fn out_loop(port: &str) -> String {
    let directory = tempfile::tempdir().expect("a directory");
    let text = format!(
        "DEFINT A-Z\r\nDECLARE SUB t ()\r\nDIM SHARED yy(3)\r\nyy(LEN(COMMAND$)) = 1\r\nt\r\nSUB t\r\nFOR x = 0 TO 99\r\nOUT {port}, yy(1) + x\r\nNEXT\r\nEND SUB\r\n"
    );
    let source = written(&directory, "T.BAS", text.as_bytes());
    loop_around(&listing(&parsed_as(&source, "qb45", "qb45")), "T", |line| line.trim_start().starts_with("out "))
}

/// An OUT to the VGA DAC reaches no memory, so the shared element it
/// writes stays in a register; one to the 8237's mode register may start
/// a DMA transfer, which OUT counted as never writing memory, so the
/// element was never reloaded after it.
#[test]
fn test_an_out_reloads_memory_only_where_its_device_may_reach_it() {
    let vga = out_loop("&H3C9");
    assert!(!vga.contains("YY%"), "{vga}");
    let dma = out_loop("&HB");
    assert!(dma.contains("YY%"), "{dma}");
}

/// qbdemo's PLASMA scaled its masked `fuh` index with an `add` per pixel:
/// a dynamic array's far pointer starts at offset 0 of its selector, which
/// `exact_offsets` never counted, so the index was not proven exact and
/// could not fold into a 32-bit `[esi+esi]` address.
#[test]
fn test_a_bounded_index_into_a_dynamic_array_scales_in_its_address() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "T.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB t ()\r\nDIM SHARED k\r\nt\r\nSUB t\r\n'$DYNAMIC\r\nDIM a(320), b(320), c(128, 128)\r\n\
DEF SEG = &HA000\r\nFOR y = 0 TO 199\r\nFOR x = 0 TO 319\r\nPOKE o, c((a(x) + k) AND 127, (b(x) + y) AND 127)\r\n\
o = o + 1\r\nNEXT\r\nNEXT\r\nEND SUB\r\n",
    );
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "T");
    assert!(regex::Regex::new(r"\[e\w\w\+e\w\w\]").unwrap().is_match(&inner), "{inner}");
}

/// PLASMA read three local arrays and VRAM with three selector registers, so
/// one register took two of them and was reloaded for each every pixel. The
/// data segment register is the fourth, with the data group reached through
/// the stack segment while it holds one.
#[test]
fn test_a_loop_out_of_selectors_holds_one_in_the_data_segment() {
    let directory = tempfile::tempdir().expect("a directory");
    let source = written(
        &directory,
        "T.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB t ()\r\nDIM SHARED k\r\nt\r\nSUB t\r\nDIM a(320), b(320), c(128, 128)\r\n\
DEF SEG = &HA000\r\nFOR y = 0 TO 199\r\nFOR x = 0 TO 319\r\nPOKE o, c((a(x) + k) AND 127, (b(x) + y) AND 127)\r\n\
o = o + 1\r\nNEXT\r\nNEXT\r\nEND SUB\r\n",
    );
    let inner = poke_loop(&listing(&parsed_as(&source, "qb45", "qb45")), "T");
    assert!(!regex::Regex::new(r"\bl[efg]s\b|pushw\s+-?\d+\n\s+pop\s+[efg]s").unwrap().is_match(&inner), "{inner}");
    // While the data segment register holds a selector, the data group is
    // reached through the stack segment.
    if regex::Regex::new(r"pop\s+ds|mov\s+ds,|\blds\b").unwrap().is_match(&inner) {
        assert!(inner.lines().filter(|line| line.contains("K%")).all(|line| line.contains("ss:K%")), "{inner}");
    }
}

/// A far element store was taken to reach any descriptor: deedlines'
/// ROTATE3D reloaded a selector and origin for each of its 12 accesses.
#[test]
fn test_element_stores_leave_descriptors_unread() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "WALK.BAS",
        b"'$DYNAMIC\r\nDIM a(100) AS INTEGER, b(100) AS INTEGER\r\nFOR i% = 0 TO 99\r\na(i%) = b(i%) + 1\r\nb(i%) = a(i%) * 3\r\nNEXT\r\n",
    );
    let assembly = listing(&parsed_as(&basic, "qb45", "qb45"));
    let selectors = stripped_lines(&assembly)
        .into_iter()
        .filter(|line| {
            ["mov es, word ptr", "mov fs, word ptr", "mov gs, word ptr"].iter().any(|one| line.starts_with(one))
        })
        .count();
    // One selector per array, read once.
    assert_eq!(selectors, 2, "{assembly}");
}

/// A counter widened to a dword to index an array rebased the loop's word
/// offsets as dwords too: deedlines' COPPER selected `mov eax, ax` and the
/// compile failed.
#[test]
fn test_a_widened_counter_rebases_word_offsets_as_words() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "COPPER.BAS",
        b"DIM SHARED fsin1%(-48 TO 1083)\r\nDIM SHARED fsin2%(-640 TO 957)\r\nDIM SHARED fsin3%(-640 TO 871)\r\nSUB actions3d\r\nDIM y2%(0 TO 399)\r\nFOR i% = 17 TO 32\r\nOUT &H3C9, (i% - 33) * 4\r\nNEXT i%\r\nFOR a% = 0 TO 15\r\nFOR B% = 0 TO 15\r\nNEXT B%\r\nNEXT a%\r\nDIM c1%(0 TO 7), c2%(0 TO 7), c3%(0 TO 7)\r\nDO WHILE INKEY$ = \"\"\r\nDEF SEG = &HA000\r\nFOR y% = 0 TO y1%\r\nFOR i% = 0 TO 7\r\nPOKE fsin2%(y2%(y%) + l%) + fsin3%(y% - m%) + i%, c1%(i%)\r\nPOKE fsin2%(y% + l%) + fsin1%(y2%(y%) + k%) + i%, c2%(i%)\r\nPOKE fsin1%(y% + k%) + fsin2%(y% + l%) + fsin3%(y% + m%) + i% - 99, c3%(i%)\r\nNEXT i%\r\nNEXT y%\r\nLOOP\r\nEND SUB\r\n",
    );
    object_bytes(&parsed_as(&basic, "qb45", "qb45"), "COPPER.BAS").expect("encodes");
}

/// A merged array's shift was added under the origin, `origin + (i * 2 +
/// 1284)`, and cost qbdemo's PLASMA an `add` per access instead of a
/// displacement.
#[test]
fn test_a_merged_array_shift_is_a_displacement() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "MERGED.BAS",
        b"DECLARE SUB t (n%)\r\n'$DYNAMIC\r\nDIM SHARED a(10) AS INTEGER, b(1 TO 5) AS LONG\r\nt 9\r\nSUB t (n%)\r\nFOR i% = 1 TO n%\r\nb((i% AND 3) + 1) = a(i% AND 7)\r\nNEXT\r\nEND SUB\r\n",
    );
    let frontend = qb_driver::Frontend { array_merging: true, ..qb_driver::Frontend::new("qb45", "qb45") };
    let program = qb_driver::parsed(&basic, &frontend, None).expect("parses");
    let lines = stripped_lines(&listing(&program));
    // b's first element is at byte 24 of the group; `+ 1` cancels its lower
    // bound.
    assert!(lines.iter().any(|line| line.contains("+24]")), "{lines:#?}");
    assert!(!lines.iter().any(|line| line.starts_with("add") && line.ends_with(", 24")), "{lines:#?}");
}

#[test]
/// A selector the allocator put in DS was reloaded with the data group
/// before the float load that read through it: deedlines' 3D object
/// collapsed to a point.
fn test_a_float_load_reads_its_selector_after_the_data_group_restore() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "RESTORE.BAS",
        b"'$DYNAMIC\r\nDIM SHARED x(4096), y(4096), z(4096)\r\nDIM SHARED xs%(4096, 1), ys%(4096, 1)\r\nSUB t\r\nSHARED n%, zpr%\r\nFOR i% = 0 TO n% - 1\r\nxs%(i%, 1) = xs%(i%, 0)\r\nys%(i%, 1) = ys%(i%, 0)\r\nIF z(i%) <= zpr% THEN GOTO 10\r\nxs%(i%, 0) = (x(i%) * 256) / z(i%)\r\nys%(i%, 0) = (y(i%) * 256) / z(i%)\r\n10\r\nNEXT i%\r\nEND SUB\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let lines = stripped_lines(&listing(&program));
    let mut restored = false;
    for line in &lines {
        if line.contains("ds:") {
            assert!(!restored, "reads the data group as an array: {lines:#?}");
        }
        if line.starts_with("mov ds,") {
            restored = line.ends_with(", ss");
        }
        if line.ends_with(':') {
            restored = false;
        }
    }
}

#[test]
/// QB code was priced for a hard-coded 386, not the machine's CPU: `x * 100`
/// became a 26-clock 486 `imul` instead of shifts and adds.
fn test_code_is_priced_for_the_machines_cpu() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "PRICED.BAS",
        b"DEFINT A-Z\r\nDECLARE FUNCTION F (x)\r\nPRINT F(7)\r\nFUNCTION F (x)\r\nF = x * 100\r\nEND FUNCTION\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let lines = stripped_lines(&listing(&program));
    assert_eq!(llrm_x86_m16::machine::BUILT_IN.cpu, "486");
    assert!(!lines.iter().any(|line| line.starts_with("imul")), "{lines:#?}");
}

#[test]
/// B800:FFFF ends in the VGA BIOS ROM, which the machine did not list: a POKE
/// to text memory at an unbounded offset might have hit any descriptor, so
/// the array's selector and origin were reloaded every iteration.
fn test_text_memory_stores_leave_array_descriptors_invariant() {
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "TEXTPOKE.BAS",
        b"DEFINT A-Z\r\n'$DYNAMIC\r\nDIM SHARED t(63)\r\nDECLARE SUB Blit (w)\r\nBlit 40\r\nSUB Blit (w)\r\nDEF SEG = &HB800\r\no = 0\r\nFOR x = 0 TO w - 1\r\nPOKE o, t(x AND 63)\r\no = o + 2\r\nNEXT\r\nEND SUB\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("BLIT proc").expect("BLIT proc");
    let end = text.find("BLIT endp").expect("BLIT endp");
    let body = backward_loop(&text[start..end]);
    let loads = body
        .lines()
        .map(str::trim)
        .filter(|line| ["mov es,", "mov fs,", "mov gs,", "mov ds,"].iter().any(|one| line.starts_with(one)));
    assert_eq!(loads.count(), 0, "{body}");
}

#[test]
fn test_scalars_stored_in_a_loop_are_forwarded_across_its_array_store() {
    // On the 486 GVN kept `i` from crossing the `a(i)` store, so the index was
    // rebuilt (`lea bx,[edx+edx]`) and compared with 255 every trip: 18
    // instructions for 16.
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "STEPS.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB Steps (n, d)\r\nSteps 7, 3\r\nSUB Steps (n, d)\r\nDIM a(255)\r\nv = n\r\nFOR i = 0 TO 255\r\na(i) = v\r\ne = e + d\r\nIF e > 100 THEN e = e - 100: v = v + 1\r\nv = v + n\r\nNEXT\r\nPRINT a(9)\r\nEND SUB\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("STEPS proc").expect("STEPS proc");
    let end = text.find("STEPS endp").expect("STEPS endp");
    let body = backward_loop(&text[start..end]);
    assert!(!body.contains("lea "), "{body}");
    assert!(!body.contains("cmp dx, 255"), "{body}");
}

#[test]
fn test_a_def_seg_known_only_to_promotion_is_not_rebuilt_per_poke() {
    // Promotion carried `DEF SEG`'s selector from the entry across `Beat`, so
    // the POKE loop rebuilt it every trip: `push 0A000h / pop fs`.
    let directory = tempfile::tempdir().expect("tempdir");
    let basic = written(
        &directory,
        "GLOW.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB Glow (n)\r\nDECLARE SUB Beat (f)\r\nGlow 3\r\nSUB Glow (n)\r\nDIM w(255)\r\nDEF SEG = &HA000\r\nFOR i = 0 TO 255\r\nw(i) = SQR(i) * 4\r\nNEXT\r\nFOR f = 1 TO n\r\nFOR x = 0 TO 255\r\nPOKE x, w((x + f) AND 255)\r\nNEXT\r\nBeat f\r\nNEXT\r\nEND SUB\r\nSUB Beat (f)\r\nOUT &H3C8, f\r\nEND SUB\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("GLOW proc").expect("GLOW proc");
    let end = text.find("GLOW endp").expect("GLOW endp");
    let procedure = &text[start..end];
    let loops = jumps(procedure, true)
        .into_iter()
        .filter_map(|(start, end, label)| {
            procedure.find(&format!("{label}:\n")).filter(|at| *at < start).map(|at| &procedure[at..end])
        })
        .collect::<Vec<_>>();
    // Innermost call-free loops: the SQR fill and the POKE loop. The `f` loop
    // holds the POKE loop and, with `Beat` inlined, no call: the selector is
    // set once a trip of it, which is not per poke.
    let span = |one: &&str| one.as_ptr() as usize..one.as_ptr() as usize + one.len();
    let innermost = |one: &&&str| {
        !loops
            .iter()
            .any(
                |other| {
                    let (outer, inner) = (span(&**one), span(other));
                    inner != outer && inner.start >= outer.start && inner.end <= outer.end
                },
            )
    };
    for one in loops.iter().filter(innermost).filter(|one| !one.contains("call")) {
        assert!(!one.contains("push") && !one.contains("pop"), "{one}");
    }
}

#[test]
fn test_merged_fields_share_one_pointer() {
    // Merged, the six fields' starts differed by constants below a sum, so each
    // kept its own pointer and four lived in the frame: 18 instructions for 9.
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/qb/particle.bas");
    let frontend = qb_driver::Frontend { array_merging: true, ..qb_driver::Frontend::new("qb45", "qb45") };
    let program = qb_driver::parsed(&basic, &frontend, None).expect("parses");
    let text = listing(&program);
    let start = text.find("ADVANCE proc").expect("ADVANCE proc");
    let end = text.find("ADVANCE endp").expect("ADVANCE endp");
    let body = backward_loop(&text[start..end]);
    assert!(!body.contains("[bp"), "{body}");
}

/// Two far arrays copied and cleared, one element a trip.
const SHIFT: &[u8] = b"DEFINT A-Z\r\nDECLARE SUB Shift ()\r\n'$DYNAMIC\r\nDIM SHARED f1(32000&) AS INTEGER\r\nDIM SHARED f2(32000&) AS INTEGER\r\nShift\r\nPRINT f2(5)\r\nSUB Shift\r\nFOR x = 0 TO 32000\r\nf2(x) = f1(x)\r\nf1(x) = 0\r\nNEXT\r\nEND SUB\r\n";

#[test]
fn test_a_counter_crossing_the_sign_bit_still_counts_to_zero() {
    // With the far origin constant, the byte offset itself is the counter and
    // runs 0 to 64000; the signed wrap kept `cmp bx, 0FA02h` in every trip.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SHIFT.BAS", SHIFT);
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("SHIFT proc").expect("SHIFT proc");
    let end = text.find("SHIFT endp").expect("SHIFT endp");
    let body = backward_loop(&text[start..end]);
    assert!(!body.contains("cmp "), "{body}");
}

#[test]
fn test_a_cell_based_on_a_named_objects_address_is_that_object() {
    // A SHARED array's descriptor was read through a register holding its
    // relocated address: `mov bx, offset ...` then `[bx+2]`, costing a base
    // register, rematerialized inside DRAWBOB's loop.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SHIFT.BAS", SHIFT);
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("SHIFT proc").expect("SHIFT proc");
    let end = text.find("SHIFT endp").expect("SHIFT endp");
    assert!(!text[start..end].contains("offset"), "{}", &text[start..end]);
}

/// ring.bas's RingSum loop, from its backward jump.
fn ring_loop() -> String {
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/qb/ring.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = regex::Regex::new(r"RINGSUM\S* proc").unwrap().find(&text).expect("RINGSUM proc").start();
    let end = start + text[start..].find(" endp").expect("RINGSUM endp");
    backward_loop(&text[start..end])
}

#[test]
fn test_a_for_counter_to_a_symbolic_bound_counts_to_zero() {
    // `FOR i = 0 TO n - 1` could reach 32767 and wrap for all the analysis
    // knew, so the count was unproven and `cmp bx, cx` stayed in the loop.
    // FOR raises Overflow instead of wrapping: the frontend's promise.
    let body = ring_loop();
    assert!(!body.contains("cmp "), "{body}");
}

#[test]
fn test_a_loaded_addend_fuses_though_the_exit_splits_a_long() {
    // The exit's `shld edx, eax, 16` reads edx, so a load into edx inside the
    // loop looked live and stayed `mov edx, [..] / add eax, edx`.
    let body = ring_loop();
    assert!(body.contains("add eax, dword ptr"), "{body}");
}

#[test]
fn test_a_masked_subscripts_scale_steps_with_its_recurrence() {
    // `buf(((i * 5 + 3) AND 1023) + 1)` shifted the masked index every trip:
    // `and si, 1023 / inc si / shl si, 2`, 9 instructions for 7.
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/qb/ring.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = regex::Regex::new(r"RINGSUM\S* proc").unwrap().find(&text).expect("RINGSUM proc").start();
    let end = start + text[start..].find(" endp").expect("RINGSUM endp");
    let body = backward_loop(&text[start..end]);
    assert!(!body.contains("shl") && body.contains(", 20\n") && body.contains(", 4092\n"), "{body}");
}

#[test]
fn test_a_loop_to_a_symbolic_bound_keeps_its_counter_and_compare() {
    // `-n TO n` is not a constant bound, so no pointer takes over: the pin's
    // indvars leaves ADVANCE an up-counter tested `icmp eq %counter, n`
    // (`opt -passes='default<O2>'` on our early IR), which is the compare
    // against the bound below.
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/qb/particle.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("ADVANCE proc").expect("ADVANCE proc");
    let end = text.find("ADVANCE endp").expect("ADVANCE endp");
    let body = backward_loop(&text[start..end]);
    assert!(regex::Regex::new(r"\binc \w+\n").unwrap().is_match(&body) && body.contains("cmp cx, ax"), "{body}");
}

#[test]
fn test_a_masked_use_keeps_the_counter_and_its_end_test() {
    // `POKE o, ch + (o AND 15)` reads the counter through a mask, so FILL
    // keeps its end test every trip. The pin's loop-reduce leaves the same
    // loop (`opt -passes=loop-reduce` on our IR, i386/i486: one phi stepping
    // by 2, `icmp ult %phi, 3997`), and its i486 loop is 9 instructions, as
    // ours is.
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/qb/textfill.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("FILL proc").expect("FILL proc");
    let end = text.find("FILL endp").expect("FILL endp");
    let body = backward_loop(&text[start..end]);
    let instructions = body.lines().filter(|line| line.starts_with("    ")).count();
    assert!(body.contains("cmp ") && body.contains(", 3997") && body.contains("and ") && instructions <= 9, "{body}");
}

#[test]
fn test_a_counter_stepped_before_other_work_is_tested_against_its_end() {
    // rcflip's third RAMP loop is tested against its end, not against zero, and
    // is no longer than the pin's i486 loop (8 instructions). The pin's
    // loop-reduce keeps one counter and addresses by `2 * x`; the 16-bit
    // address has no scale, so ours steps the byte offset (`cmp bx, 674`)
    // beside the counter, for the same length (DEVIATE LU1).
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/qb/rcflip.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("RAMP proc").expect("RAMP proc");
    let end = text.find("RAMP endp").expect("RAMP endp");
    let procedure = &text[start..end];
    let body = jumps(procedure, true)
        .into_iter()
        .filter_map(|(start, end, label)| {
            let at = procedure.find(&format!("{label}:\n")).filter(|at| *at < start)?;
            Some(procedure[at..end].to_owned())
        })
        .find(|body| body.contains("imul"))
        .expect("the multiplying loop");
    let instructions = body.lines().filter(|line| line.starts_with("    ")).count();
    assert!(
        (body.contains("cmp ax, 357") || body.contains(", 674")) && !body.contains("or ax, ax") && instructions <= 8,
        "{body}"
    );
}

#[test]
fn test_a_dividend_two_instructions_require_is_copied_into_its_register_once() {
    // `cdq` and `idiv` each took their own copy of i \ d's dividend in EAX,
    // so the extended value sat in EDI across both and one of the three
    // recurrences spilled: `mov ax, [bp-2]` and `add word ptr [bp-2], 5`. A
    // divisor read from memory stays a division; a constant is a reciprocal.
    // The divisor is not a constant (RND * 3: with the scalars out of memory
    // `INT(RND * 1)` is 0) and the loop is long (to 100 it is fully unrolled).
    // CVP narrows `i \\ d(0)` to 16 bits as the pin does (`sdiv i16`): `cwd`.
    let source = "DEFINT A-Z\nDIM b(1000), d(1)\nDIM i, t\nd(0) = 5 + INT(RND * 3)\nt = 0\nFOR i = 0 TO 1000 STEP 5\n    b(i) = i \\ d(0)\n    t = t + b(i)\nNEXT i\nPRINT \"T=\"; t\n";
    let basic = std::env::temp_dir().join(format!("stride_variable_divisor_{}.bas", std::process::id()));
    std::fs::write(&basic, source).expect("writes the program");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let body = backward_loop(&listing(&program));
    assert!(!body.contains("[bp") && body.contains("cwd") && body.matches("idiv").count() == 1, "{body}");
}

#[test]
fn test_a_constant_divisor_of_a_non_negative_counter_is_folded_away() {
    // stride's `i \ 5` is a counter (0, 1, 2 ...), and with `t` and `b` out of
    // memory the sum is the constant 210: the program holds no loop and no
    // division of either kind. It once held a loop with a `mul` by the
    // reciprocal, then one with a spill (the next test keeps that shape).
    let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/qb/stride.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    assert!(found_backward_loop(&text).is_none(), "{text}");
    assert!(!text.contains("idiv") && !text.contains("mul"), "{text}");
}

#[test]
fn test_a_constant_divisor_of_a_non_negative_counter_is_a_reciprocal_without_a_spill() {
    // The same stride loop with a sum the optimizer cannot fold (`m` is read
    // at run time): `i` is DEFINT and counts up from 0, so `i \ 5` was
    // `cdq; idiv` and spilled one of three recurrences; it is a counter, and
    // the loop holds no division of either kind and no spill.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "STRIDEM.BAS",
        b"DEFINT A-Z\r\nDIM b(200)\r\nDIM i, t, m\r\nINPUT m\r\nt = 0\r\nFOR i = 0 TO m STEP 5\r\n    b(i) = i \\ 5\r\n    t = t + b(i)\r\nNEXT i\r\nPRINT \"T=\"; t\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let body = backward_loop(&listing(&program));
    assert!(!body.contains("[bp") && !body.contains("idiv") && !body.contains("mul"), "{body}");
}

#[test]
fn test_a_sum_read_after_its_loop_is_copied_out_where_the_loop_ends() {
    // segld's sum is printed after both loops, across a runtime call, so as
    // one value with the loop's it could only live in SI: the loop added into
    // SI and copied back to CX on a split back edge, `mov cx, si` and `jmp`.
    // The loops are too long to unroll: at segld's 5 by 20 they are folded
    // away.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "SEGLD.BAS",
        b"REM $DYNAMIC\nDEFINT A-Z\nDIM a(100)\nDIM i, j, t\nt = 0\nFOR j = 1 TO 2\n    FOR i = 1 TO 100\n        a(i) = i\n        t = t + a(i)\n    NEXT i\nNEXT j\nPRINT \"T=\"; t\nPRINT \"DONE\"\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let body = backward_loop(&listing(&program));
    let copies = regex::Regex::new(r"mov [a-z]{2}, [a-z]{2}\n").unwrap();
    assert!(!body.contains("jmp") && !copies.is_match(&body), "{body}");
}

#[test]
fn test_loops_count_to_zero_once_their_other_counters_are_settled() {
    // qbdemo's PLASMA: counting to zero inside strength reduction rotated the
    // first loop before its pointer was made, leaving `test bx, bx`; run
    // last, it kept the second loop's `x` because `x * 2` and `x * 8` each
    // covered only part of it, so `lea si, [eax+eax]` addressed both stores.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "PAIR.BAS",
        b"DEFINT A-Z
DECLARE SUB plasma (totalframes%)
plasma 2
SUB plasma (totalframes)
DIM unf(320), unfunf(320)
DIM sine(512)
DIM fuh(128, 128)
DEF SEG = &HA000
FOR x = 0 TO 512
sine(x) = SIN(x * 3.14 / 256) * 32 + 32
NEXT
FOR f = 1 TO totalframes
 FOR x = 0 TO 320
  unf(x) = sine((x + f) AND 511) + sine((3 * x + 7 * f + 3) AND 511)
 NEXT
 FOR x = 0 TO 320
  unf(x) = sine((x * 11 + f * 7) AND 511) + sine((3 * x + 7 * f + 3) AND 511)
  unfunf(x) = sine((x * 4 + f * 5) AND 511) + sine((9 * x + 2 * f + 371) AND 511)
 NEXT
NEXT
PRINT unf(5)
END SUB
",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = regex::Regex::new(r"\bPLASMA\S* proc").unwrap().find(&text).expect("PLASMA proc").start();
    let procedure = &text[start..start + text[start..].find(" endp").expect("PLASMA endp")];
    let loops = jumps(procedure, true)
        .into_iter()
        .filter_map(|(at, end, label)| {
            procedure.find(&format!("{label}:\n")).filter(|begun| *begun < at).map(|begun| &procedure[begun..end])
        })
        .collect::<Vec<_>>();
    let innermost =
        loops.iter().filter(|one| !loops.iter().any(|other| other.len() < one.len() && one.contains(*other)));
    let masked = innermost.filter(|one| one.contains("and ")).collect::<Vec<_>>();
    let [first, second] = masked.as_slice() else { panic!("{procedure}") };
    // The pin's loop-reduce on the same IR keeps these loops' two-step pointer
    // counter and its end test (`icmp eq %p.next, 642`). DEVIATE LU2 (#3646):
    // the offset beside two registers rides in the `lea`, so ours counts the
    // pointer up to zero and ends on the add's flags: no `test`, no `cmp`.
    assert!(
        !first.contains("test ")
            && !first.contains("cmp ")
            && regex::Regex::new(r"add bx, 2\n\s+jne \w+\s*$").unwrap().is_match(first),
        "{first}"
    );
    // The stores are addressed by the pointer that counts, not a scaled copy of
    // `x`.
    let lea = regex::Regex::new(r"lea (\w+),").unwrap();
    let scaled: BTreeSet<&str> = lea.captures_iter(second).map(|one| one.get(1).unwrap().as_str()).collect();
    let store = regex::Regex::new(r"mov word ptr \w+:\[(\w+)").unwrap();
    let bases: Vec<&str> = store.captures_iter(second).map(|one| one.get(1).unwrap().as_str()).collect();
    assert!(!bases.is_empty() && bases.iter().all(|base| !base.starts_with('e') && !scaled.contains(base)), "{second}");
}

#[test]
fn test_a_loop_reloads_the_invariants_the_registers_cannot_hold() {
    // Six invariants and a pointer need seven registers; m16 has six and BP.
    // LoopSlots' `promoted` step used to park two invariants in `di` and BP
    // (push before the loop, pop after); it is retired (its header, DEVIATE
    // LS1), as the pin keeps no such register: the loop reloads them from [bp]
    // each trip. Measured on this program (LLRM_DEBUG=cost, -O2): 819 ->
    // 1015 executed instructions, 1 -> 200 reloads; over the 164 QB
    // programs of bench/ and tests/run/qb (-O1, -O2, -Os) the retirement
    // changes 1 or 2 functions and executes -0.000%.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "SIX.BAS",
        b"DEFINT A-Z
DECLARE SUB s (k)
DIM SHARED a(99), b(99), c(99), d(99), e(99), f(99)
s 3
PRINT a(5)
SUB s (k)
 ua = k * 3
 ub = k * 4
 uc = k * 5
 ud = k * 6
 ue = k * 7
 uf = k * 8
 FOR i = 0 TO 99
  a(i) = a(i) + ua
  b(i) = b(i) + ub
  c(i) = c(i) + uc
  d(i) = d(i) + ud
  e(i) = e(i) + ue
  f(i) = f(i) + uf
 NEXT
END SUB
",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = regex::Regex::new(r"(?m)^S proc").unwrap().find(&text).unwrap_or_else(|| panic!("{text}")).start();
    let procedure = &text[start..start + text[start..].find(" endp").expect("S endp")];
    let body = backward_loop(procedure);
    assert!(body.contains("[bp"), "the premise: the invariants no register holds are reloaded: {body}");
    let (before, after) = procedure.split_at(procedure.find(&body).expect("the loop"));
    // The frame register carries a slot into the loop as `mov bp, [bp-d]`
    // (after the prologue's own push).
    assert!(!before.contains("mov bp, word ptr") && !after.contains("pop bp"), "BP is not parked: {procedure}");
}

#[test]
#[ignore = "row QB-SEL-1: segment-register class in RA"]
fn test_a_selector_the_loop_never_changes_is_loaded_before_it() {
    // qbdemo's FRACTALEFFECT: `tmpshit`'s selector, spilled across the calls
    // around it, was reloaded into fs from [bp] on every trip of the loop
    // that only reads that array.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "FRACTAL.BAS", b"DEFINT A-Z\r\nDECLARE SUB fracline (y%, y1#, y2#, x1#, x2#, distthr#)\r\nDECLARE SUB render (x1%, y1%, x2%, y2%)\r\nDECLARE SUB fractaleffect (totalframes%)\r\nCONST XCENTRE = -.577816001047738#\r\nCONST YCENTRE = -.6311212235178052#\r\n'$DYNAMIC\r\nDIM SHARED totalframecount AS INTEGER\r\nDIM SHARED fractal1(32000&) AS INTEGER\r\nDIM SHARED fractal2(32000&) AS INTEGER\r\nfractaleffect 30\r\nEND\r\nSUB fractaleffect (totalframes)\r\nDIM tmpshit(160, 100)\r\nly = -100:  f# = 1\r\nFOR f = 1 TO totalframes\r\n  ly0 = ly\r\n  ly1 = ly + 1\r\n  ly2 = ly + 2\r\n  ly3 = ly + 3\r\n  ly0# = ly0 / f# + YCENTRE\r\n  ly1# = ly1 / f# + YCENTRE\r\n  ly2# = ly2 / f# + YCENTRE\r\n  ly3# = ly3 / f# + YCENTRE\r\n  lx1# = XCENTRE - 160 / f#\r\n  lx2# = XCENTRE + 160 / f#\r\n  DEF SEG = VARSEG(fractal1(0))\r\n  distthr# = 4\r\n  fracline ly0 + 100, ly0#, ly0#, lx1#, lx2#, distthr#\r\n  fracline ly2 + 100, ly2#, ly2#, lx1#, lx2#, distthr#\r\n  fracline ly1 + 100, ly1#, ly1#, lx1#, lx2#, distthr#\r\n  fracline ly3 + 100, ly3#, ly3#, lx1#, lx2#, distthr#\r\n ly = ly + 4\r\n IF ly >= 100 THEN\r\n  ly = -100\r\n  f# = f# * 2\r\n  o = 16080\r\n  FOR y = 0 TO 99\r\n   FOR x = 0 TO 159\r\n    tmpshit(x, y) = PEEK(o)\r\n    o = o + 1\r\n   NEXT\r\n   o = o + 160\r\n  NEXT\r\n  o = 0\r\n  FOR x = 0 TO 32000\r\n   fractal2(x) = fractal1(x)\r\n   fractal1(x) = 0\r\n  NEXT\r\n  FOR y = 0 TO 99\r\n   FOR x = 0 TO 159\r\n    POKE x * 2 + y * 640, tmpshit(x, y)\r\n   NEXT\r\n  NEXT\r\n  \r\n END IF\r\n DEF SEG = VARSEG(fractal2(0))\r\n f50 = (f) MOD 50\r\n x1 = f50 * 1.6: y1 = f50\r\n x2 = 320 - f50 * 1.6: y2 = 200 - f50\r\n render x1, y1, x2, y2\r\n IF INKEY$ > \"\" THEN EXIT SUB\r\n totalframecount = totalframecount + 1\r\nNEXT\r\nEND SUB\r\nSUB fracline (y%, y1#, y2#, x1#, x2#, distthr#)\r\nfractal1(y%) = y1# + x2#\r\nEND SUB\r\nSUB render (x1%, y1%, x2%, y2%)\r\nfractal2(x1%) = y2%\r\nEND SUB\r\n");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start =
        regex::Regex::new(r"(?m)^FRACTALEFFECT proc").unwrap().find(&text).unwrap_or_else(|| panic!("{text}")).start();
    let procedure = &text[start..start + text[start..].find(" endp").expect("FRACTALEFFECT endp")];
    let loops = jumps(procedure, true)
        .into_iter()
        .filter_map(|(at, end, label)| {
            procedure.find(&format!("{label}:\n")).filter(|begun| *begun < at).map(|begun| &procedure[begun..end])
        })
        .collect::<Vec<_>>();
    // Direct (`mov es, [bp-N]`) or through a register (`mov cx, [bp-N]; mov es,
    // cx`); the regex crate has no backreference, so the two names compare
    // here.
    let direct = regex::Regex::new(r"mov [c-gs]s, word ptr \[bp").unwrap();
    let through = regex::Regex::new(r"mov (\w+), word ptr \[bp[^\n]*\n\s*mov [c-gs]s, (\w+)\b").unwrap();
    let reloads = |one: &str| direct.is_match(one) || through.captures_iter(one).any(|found| found[1] == found[2]);
    for one in loops.iter().filter(|one| !loops.iter().any(|other| other.len() < one.len() && one.contains(*other))) {
        assert!(!reloads(one), "{one}");
    }
}

#[test]
fn test_a_loop_doubles_an_index_with_add_not_the_three_clock_shift() {
    // qbdemo's CYCLEBLOBS scaled up to nine indexes a trip with `shl r,1`,
    // three clocks each on the 486 where `add r,r` takes one.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "LOOKUP.BAS", b"DEFINT A-Z\r\n'$DYNAMIC\r\nDIM t(32, 32), k(32)\r\nFOR i = 0 TO 32: k(i) = (i * 7) AND 31: NEXT\r\nFOR i = 0 TO 32: s = s + t(i, k(i)): NEXT\r\nPRINT s\r\n");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let body = jumps(&text, true)
        .into_iter()
        .filter_map(|(at, end, label)| {
            text.find(&format!("{label}:\n")).filter(|begun| *begun < at).map(|begun| &text[begun..end])
        })
        // The lookup loop is the last: its sum stays in a register, so `S%`
        // no longer names it.
        .last()
        .unwrap_or_else(|| panic!("{text}"));
    assert!(!regex::Regex::new(r"s[ah]l \w+, 1\n").unwrap().is_match(body), "{body}");
}

#[test]
fn test_a_slot_an_x87_store_writes_stays_in_memory() {
    // Parking a register around qbdemo's sprite loop gave its `fistp` slot a
    // register home, and `fistp ax` does not encode: the compile failed.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SPRITE.BAS", b"DEFINT A-Z\r\n'$DYNAMIC\r\nDIM SHARED b(32, 32) AS INTEGER\r\nFOR x = 0 TO 32\r\nFOR y = 0 TO 32\r\nb(x, y) = 16 - SQR((16 - x) ^ 2 + (16 - y) ^ 2)\r\nIF b(x, y) < 0 THEN b(x, y) = 0\r\nNEXT\r\nNEXT\r\nPRINT b(3, 4)\r\n");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    assert!(!regex::Regex::new(r"fistp [a-z]{2}\n").unwrap().is_match(&text), "{text}");
}

#[test]
fn test_a_register_live_through_a_loop_is_parked_for_its_counters() {
    // qbdemo's PLASMA kept two counters in [bp] every trip: di held a value
    // the loop never touched, and BP alone could take only one of them.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "PLASMA.BAS", b"DEFINT A-Z\r\nDECLARE SUB plasma (totalframes%)\r\nDECLARE SUB updpalplasma (f%)\r\nDIM SHARED totalframecount AS INTEGER\r\nplasma 2\r\nEND\r\nSUB plasma (totalframes)\r\nDIM unf(320), unfunf(320)\r\nDIM sine(512)\r\nDIM fuh(128, 128)\r\nDEF SEG = &HA000\r\nFOR x = 0 TO 512\r\nsine(x) = SIN(x * 3.14 / 256) * 32 + 32\r\nNEXT\r\nFOR f = 1 TO totalframes\r\n FOR x = 0 TO 320\r\n  unf(x) = sine((x + f) AND 511) + sine((3 * x + 7 * f + 3) AND 511)\r\n NEXT\r\n o = 0\r\n FOR y = 0 TO 128\r\n  unf2 = sine((y * 7 + f * 5) AND 511) + sine((y * 14 + f * 11 + 1943) AND 511)\r\n  FOR x = 0 TO 128\r\n   fuh(x, y) = unf(x) + unf2\r\n   o = o + 1\r\n  NEXT\r\n NEXT\r\n FOR x = 0 TO 320\r\n  unf(x) = sine((x * 11 + f * 7) AND 511) + sine((3 * x + 7 * f + 3) AND 511)\r\n  unfunf(x) = sine((x * 4 + f * 5) AND 511) + sine((9 * x + 2 * f + 371) AND 511)\r\n NEXT\r\n o = 0\r\n FOR y = 0 TO 199\r\n  unf2 = sine((y * 11 + f * 6) AND 511) + sine((y * 14 + f * 11 + 1943) AND 511)\r\n  unf3 = sine((y * 9 + f * 4) AND 511) + sine((y * 17 + f * 23 + 1943) AND 511)\r\n  FOR x = 0 TO 319\r\n   POKE o, fuh((unf(x) + unf2) AND 127, (unfunf(x) + unf3) AND 127)\r\n   o = o + 1\r\n  NEXT\r\n NEXT\r\n updpalplasma f\r\n totalframecount = totalframecount + 1\r\n IF INKEY$ > \"\" THEN EXIT SUB\r\nNEXT\r\nEND SUB\r\nSUB updpalplasma (f%)\r\nOUT &H3C8, f%\r\nEND SUB\r\n");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("PLASMA proc").unwrap_or_else(|| panic!("{text}"));
    let procedure = &text[start..start + text[start..].find(" endp").expect("PLASMA endp")];
    let body = jumps(procedure, true)
        .into_iter()
        .filter_map(|(at, end, label)| {
            procedure.find(&format!("{label}:\n")).filter(|begun| *begun < at).map(|begun| &procedure[begun..end])
        })
        .find(|one| one.matches(", 1022").count() == 4)
        .unwrap_or_else(|| panic!("{procedure}"));
    // The pin's loop-reduce leaves this loop five counters too (steps 2, 6, 8,
    // 18 and 22, ending at 642), on a machine of six: one of them is parked,
    // in one slot, a store and the reloads that use it.
    let slots: BTreeSet<&str> =
        regex::Regex::new(r"\[bp[-+]\d+\]").unwrap().find_iter(body).map(|found| found.as_str()).collect();
    assert!(slots.len() <= 1 && body.matches("[bp").count() <= 3, "{body}");
}

#[test]
fn test_a_call_after_a_loop_that_moved_ds_gets_the_data_group_back() {
    // qbdemo's PLASMA hung: a call's contract read only its arguments, so the
    // `mov ds, ss` before it looked dead, and the callee ran with DS still
    // naming the array the loop had read through it.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "DSR.BAS", b"DEFINT A-Z\r\nDECLARE SUB p (a)\r\n'$DYNAMIC\r\nDIM SHARED a(99), b(99), c(99)\r\nDEF SEG = &HA000\r\nFOR r = 1 TO 3\r\nFOR i = 0 TO 99: POKE i, a(i) + b(i) + c(i): NEXT\r\np r\r\nNEXT\r\nSUB p (a)\r\nSHARED t\r\nt = t + a\r\nEND SUB\r\n");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = text.find("proc far").unwrap_or_else(|| panic!("{text}"));
    let procedure = &text[start..start + text[start..].find(" endp").expect("endp")];
    // The loop moves DS; every call, in layout order, finds it back on the data
    // group.
    assert!(procedure.contains("mov ds, "), "{procedure}");
    let mut moved = false;
    for line in procedure.lines().map(str::trim) {
        if let Some(source) = line.strip_prefix("mov ds, ") {
            moved = source != "ss";
        } else if line.starts_with("call ") {
            assert!(!moved, "{line} with DS moved\n{procedure}");
        }
    }
}

#[test]
fn test_a_pointer_from_a_symbolic_start_leaves_control_to_the_counter() {
    // deedlines' MOV3DPOS: the array pointer took control from the counter, so
    // each address became `[bx+si]`, and with both taken one more by-reference
    // pointer was reloaded from [bp] every trip.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "MOVE.BAS",
        b"DEFINT A-Z\r\nDECLARE SUB m (xp!, yp!, zp!)\r\n'$DYNAMIC\r\nDIM SHARED x(4096) AS SINGLE, y(4096) AS SINGLE, z(4096) AS SINGLE, n\r\n\
n = 64: a! = 1: b! = 2: c! = 3\r\nm a!, b!, c!\r\n\
SUB m (xp!, yp!, zp!)\r\nFOR i = 0 TO n - 1\r\nx(i) = x(i) + xp\r\ny(i) = y(i) + yp\r\nz(i) = z(i) + zp\r\nNEXT\r\nEND SUB\r\n",
    );
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    let start = regex::Regex::new(r"(?m)^M proc").unwrap().find(&text).unwrap_or_else(|| panic!("{text}")).start();
    let end = start + text[start..].find(" endp").expect("M endp");
    let body = backward_loop(&text[start..end]);
    assert!(!regex::Regex::new(r"\[[a-z]{2}\+[a-z]{2}").unwrap().is_match(&body), "{body}");
}

/// Float constants load as GCC's do: a readonly single each (`x - 1` is the
/// pin's `fadd` of -1.0, InstCombine's canonical form, so `fld1` no longer
/// appears). Each literal was stored to a frame temporary at its use and
/// read back by `fild`, as BC does.
#[test]
fn test_an_integer_literal_in_a_float_expression_is_a_constant() {
    let source = "DEFDBL A-Z\r\nDECLARE SUB f (a, b)\r\nf 1.5, 2\r\nSUB f (a, b)\r\nx = a * .5 + 3\r\ny = x - 1\r\n\
z = y * 320\r\nPRINT x, y, z\r\nEND SUB\r\n";
    let listing = procedure_listing("CONSTANT.BAS", source.as_bytes(), "F");
    assert!(!listing.contains("fild") && !listing.contains("fiadd"), "{listing}");
    assert!(listing.contains("fadd dword ptr") && !listing.contains("320"), "{listing}");
}

/// A module through the rich MIR and isel: a string comparison branches on
/// the flags B$SCMP leaves, and ports are `in` and `out`.
#[test]
fn test_a_module_compiles_through_the_rich_mir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = tmp.path().join("RICH.BAS");
    std::fs::write(&source, "DECLARE SUB Fade (level%)\r\nSUB Fade (level%)\r\nOUT &H3C8, 0\r\nv% = INP(&H3C9)\r\nIF COMMAND$ > \"A\" THEN OUT &H3C9, level% + v%\r\nEND SUB\r\n").unwrap();
    let program = parsed_as(&source, "qb45", "qb45");
    let module =
        qb_compile::assembled(&program, None, &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()))
            .expect("assembles");
    let text = masm::text(&module).expect("prints");
    let fade = between(&text, "FADE proc", "endp");
    let call = fade.find("call far ptr B$SCMP").expect("the comparison's call");
    assert!(fade[call..].lines().nth(1).is_some_and(|line| line.trim().starts_with('j')), "{fade}");
    assert!(fade.contains("in al, dx") && fade.contains("out dx, al"), "{fade}");
}

/// B$ENRA zero-fills a SUB's locals, so zeroing them again is dead: the
/// rich route filled runtime-frame-stack's 4096-byte string with `rep stosd`,
/// 265 instructions the old route never ran.
#[test]
fn test_the_runtime_frame_zeroes_the_locals() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "zeroed.bas", b"DECLARE SUB Report (n AS LONG)\nCALL Report(1)\nSUB Report (n AS LONG)\nDIM buffer AS STRING * 4096\nDIM counts(3) AS INTEGER\nbuffer = \"X\"\ncounts(n) = 1\nPRINT buffer; counts(1)\nEND SUB\n");
    let program = qb_driver::parsed(
        &source,
        &qb_driver::Frontend { runtime_frames: true, ..qb_driver::Frontend::new("qb45", "qb45") },
        None,
    )
    .expect("parses");
    let module =
        qb_compile::assembled(&program, None, &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()))
            .expect("assembles");
    let text = masm::text(&module).expect("prints");
    let report = between(&text, "REPORT proc", "endp");
    assert!(report.contains("call far ptr B$ENRA"), "{report}");
    assert!(
        !report.contains("stos")
            && !report.lines().any(|line| line.contains("ptr [bp-") && line.trim().ends_with(", 0")),
        "{report}"
    );
}

/// Seven suite programs refused DATA on the rich route.
#[test]
fn test_data_statements_compile_through_the_rich_mir() {
    for name in ["fpcalc", "fpcsex", "fpi2cs", "fpicse", "hotlpx", "lngmxx", "pressx"] {
        let basic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../../tests/run/qb/{name}.bas"));
        let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("vbdos", "vbdos"), None).expect("parses");
        qb_compile::assembled(&program, None, &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

/// RESTORE to a label and DATA after END were refused on the rich route:
/// their DATA blocks were code entries, BC keying a row by its code. B$RSTB
/// only compares keys, so the rich route keys a row by its position.
#[test]
fn test_rich_route_keys_data_rows_by_position() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "rstend.bas", b"DEFINT A-Z\nDATA 1, 2\nREAD a, b\nRESTORE again\nREAD c\nRESTORE\nREAD d\nPRINT a; b; c; d\nEND\nagain:\nDATA 3, 4\n");
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("vbdos", "vbdos"), None).expect("parses");
    let rich =
        qb_compile::assembled(&program, None, &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()))
            .expect("compiles");
    let text = masm::text(&rich).expect("prints");
    let rows = between(&text, "$QB$DS label byte\n", "BC_DS ends");
    assert_eq!(
        rows,
        "db 000h,000h\ndb 020h,031h,02ch,020h,032h,000h\ndb 001h,000h\ndb 020h,033h,02ch,020h,034h,000h\ndb 0ffh,0ffh,001h\n"
    );
    assert!(text.contains("pushw 1\n    call far ptr B$RSTB"), "{text}");
}

/// B$ASSN fills a fixed-length destination: its zero fill was stored,
/// then overwritten, on every call (runtime-frame-stack's 4096 bytes).
#[test]
fn a_fixed_length_assignment_fills_its_destination() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "fixed.bas", b"SUB s\nDIM t AS STRING * 8\nt = \"X\"\nPRINT t\nEND SUB\n");
    let program = parsed(&source);
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).swap_remove(0);
    let text = llrm_mir::print::module(&emitted.module);
    assert!(
        text.contains("@llrm.qb.B$ASSN(ptr addrspace(1) %")
            // (VBDOS's far strings: the literal goes as its descriptor, source
            // length 0)
            && text.contains(", i16 0, ptr addrspace(1) nocapture writeonly initializes((0, 8)) %"),
        "{text}"
    );
}

/// A FUNCTION AS SINGLE or AS DOUBLE returns as VBDOS's do: stored through
/// the hidden destination its caller passes last, that pointer returned. The
/// rich route left the result in ST(0), so a caller compiled by the old
/// route read its frame time through the FPU status word fnstsw left in AX:
/// qrender's dt read 0 and it never ticked.
#[test]
fn test_a_floating_function_returns_through_its_hidden_destination() {
    for (suffix, mir, bytes) in [("!", "float", 4), ("#", "double", 8)] {
        let directory = tempfile::TempDir::new().unwrap();
        let basic = format!(
            "DECLARE FUNCTION Half{suffix} (x AS {kind})\nDIM y AS {kind}\ny = Half{suffix}(3)\nPRINT y\nFUNCTION Half{suffix} (x AS {kind})\nHalf{suffix} = x / 2\nEND FUNCTION\n",
            kind = if bytes == 4 { "SINGLE" } else { "DOUBLE" }
        );
        let source = written(&directory, "fret.bas", basic.as_bytes());
        let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("vbdos", "vbdos"), None).expect("parses");
        let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).swap_remove(0);
        let text = llrm_mir::print::module(&emitted.module);
        let half = between(
            &text,
            &format!("define cc 1000 ptr @\"HALF{suffix}\"(ptr dereferenceable({bytes}) %0, ptr %1)"),
            "\n}",
        );
        assert!(
            half.lines()
                .any(|line| line.trim().starts_with(&format!("store {mir}")) && line.trim().ends_with(", ptr %1")),
            "{half}"
        );
        assert!(half.lines().any(|line| line.trim() == "ret ptr %1"), "{half}");
        let call = text
            .lines()
            .find(|line| line.contains(&format!("call cc 1000 addrspace(1) ptr @\"HALF{suffix}\"(")))
            .unwrap_or_else(|| panic!("{text}"));
        let returned = call.trim().split(' ').next().expect("a named result");
        assert!(text.contains(&format!("load {mir}, ptr {returned}")), "{text}");
    }
}

/// UBOUND's "subscript out of range" call may RESUME, so its block jumps on
/// rather than stopping: only the frontend's `cold` says it never runs.
/// The MIR emitter dropped it, and noreturn found no such block cold.
#[test]
fn test_a_frontend_cold_block_stays_cold_in_the_rich_mir() {
    use llrm_analysis::noreturn;
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "bound.bas", b"N = 5\nREDIM A(N)\nPRINT UBOUND(A)\n");
    let checked = qb_driver::Frontend { checked_arrays: true, ..qb_driver::Frontend::new("vbdos", "vbdos") };
    let program = qb_driver::parsed(&source, &checked, None).expect("parses");
    let expected: BTreeSet<String> = program.modules[0]
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| block.cold)
        .map(|block| format!("b{}", block.id))
        .collect();
    assert!(!expected.is_empty(), "no cold HIR block");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).swap_remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let declarations = emitted.module.declarations();
    let mut found = BTreeSet::new();
    for (_, _, function) in emitted.module.functions() {
        for at in noreturn::cold(&emitted.module.context, &declarations, function, &BTreeSet::new()) {
            found.extend(function.block(llrm_analysis::cfg::block(at)).name.clone());
        }
    }
    assert!(expected.is_subset(&found), "cold {expected:?}, noreturn found {found:?}");
}

/// The handler is __main's one landing pad, each call that may raise before
/// it an invoke, and the rich route selects it; it was refused as "an ON
/// ERROR handler".
#[test]
fn an_error_handler_is_the_main_bodys_landing_pad() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "HANDLED.BAS",
        b"DEFINT A-Z\nON ERROR GOTO h\nERROR 5\nPRINT c\nEND\nh:\nc = ERR\nRESUME NEXT\n",
    );
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    assert!(llrm_mir::verify::verify(&emitted.module).is_empty());
    let main =
        emitted.module.functions().find(|(_, global, _)| global.name.as_deref() == Some("__main")).expect("__main").2;
    let opcodes: Vec<&llrm_mir::Opcode> = main.walk().map(|(_, inst)| &main.instruction(inst).opcode).collect();
    assert!(main.personality.is_some());
    assert_eq!(opcodes.iter().filter(|one| matches!(one, llrm_mir::Opcode::LandingPad { .. })).count(), 1);
    assert!(opcodes.iter().any(|one| matches!(one, llrm_mir::Opcode::Invoke(_))));
}

/// ERL in the handler reads the faulting statement's line from a table by
/// the site RESUME switches on; it was refused, as the recompiled object
/// keeps no line table for the runtime's.
#[test]
fn erl_is_the_faulting_statements_line() {
    let directory = tempfile::TempDir::new().unwrap();
    let source =
        written(&directory, "ERL.BAS", b"DEFINT A-Z\nON ERROR GOTO h\n100 ERROR 5\nEND\nh:\nc = ERL\nRESUME NEXT\n");
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    let text = llrm_mir::print::module(&emitted.module);
    let table = text
        .lines()
        .find(|line| line.starts_with("@\"$QB$ERL$__main\" =") || line.starts_with("@$QB$ERL$__main ="))
        .expect("the ERL table");
    assert!(table.contains("i16 100"), "{table}");
    assert!(
        text.contains("@$QB$ERL$__main, i16 0, i16 %") || text.contains("@\"$QB$ERL$__main\", i16 0, i16 %"),
        "{text}"
    );
}

/// ON ERROR GOTO names another handler: the pad goes to whichever was named
/// last. Was refused as "a second ON ERROR GOTO handler".
#[test]
fn the_pad_goes_to_the_handler_named_last() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "TWO.BAS", b"ON ERROR GOTO first\nERROR 5\nON ERROR GOTO second\nERROR 6\nEND\nfirst:\nPRINT 1\nRESUME NEXT\nsecond:\nPRINT 2\nRESUME NEXT\n");
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    let main =
        emitted.module.functions().find(|(_, global, _)| global.name.as_deref() == Some("__main")).expect("__main").2;
    let pad = main
        .layout()
        .iter()
        .copied()
        .find(|&block| {
            main.iter_block(block)
                .any(|one| matches!(
                    main.instruction(one).opcode,
                    llrm_mir::Opcode::LandingPad { .. }
                ))
        })
        .expect("the pad");
    let last = main.block(pad).last().expect("a terminator");
    assert!(
        matches!(main.instruction(last).opcode, llrm_mir::Opcode::Switch),
        "{}",
        llrm_mir::print::module(&emitted.module)
    );
}

/// ON LOCAL ERROR: the SUB's own handler is its landing pad, registered by
/// the local intrinsic. Was refused as "ON LOCAL ERROR, whose handler is a
/// procedure's".
#[test]
fn a_procedures_own_handler_is_its_landing_pad() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "LOCAL.BAS",
        b"DECLARE SUB s ()\nCALL s\nEND\nSUB s\nON LOCAL ERROR GOTO h\nERROR 53\nEXIT SUB\nh:\nRESUME NEXT\nEND SUB\n",
    );
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("pds71", "pds71"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("@llrm.qb.onlocalerror(i1 true)") && !text.contains("@llrm.qb.onerror("), "{text}");
    assert!(text.contains("landingpad"), "{text}");
}

/// An error a SUB raises lands on the SUB's own pad, which calls the module
/// handler, run as its own function, and switches on the RESUME it answers;
/// that function turns trapping back on however few pads it serves. Was
/// refused as "errors raised inside SUBs"; with the handler function's
/// registration erased, POWRES's second error was fatal.
#[test]
fn a_subs_error_lands_on_its_own_pad_and_runs_the_module_handler() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "SUBERR.BAS",
        b"DECLARE SUB r ()\nON ERROR GOTO h\nCALL r\nEND\nh:\nRESUME NEXT\nSUB r\nERROR 5\nPRINT 1\nEND SUB\n",
    );
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let mut emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    let sub = emitted.module.functions().find(|(_, global, _)| global.name.as_deref() == Some("R")).expect("R").2;
    assert!(sub.walk().any(|(_, inst)| matches!(sub.instruction(inst).opcode, llrm_mir::Opcode::LandingPad { .. })));
    // As the runtime's promises mark it when the program is linked.
    emitted
        .module
        .function_mut("llrm.qb.B$CEND")
        .expect("END")
        .1
        .attrs
        .push(llrm_mir::Attribute::Flag("nounwind".to_owned()));
    llrm_codegen::backend::ehprepare::prepared(&mut emitted.module, &llrm_x86_m16::layout()).expect("prepared");
    let text = llrm_mir::print::module(&emitted.module);
    let handler = &text[text.find("@__main$handler(i16 %0").expect("the handler function")..];
    let handler = &handler[..handler.find("\n}\n").expect("its end")];
    assert!(handler.contains("@llrm.qb.B$OEGA(ptr addrspace(1) @$QB$LANDING)"), "{handler}");
}

/// Outside the handler ERL is the line the module handler last took, kept
/// by the handler: the runtime's knows no line of the recompiled code. Once
/// refused, which also refused qb-qrender's main.bas. RESUME keeps it on
/// QB 4.5, as BC's code prints 100 here, and clears it on VBDOS, 0.
#[test]
fn erl_outside_the_handler_is_the_line_the_handler_took() {
    let directory = tempfile::TempDir::new().unwrap();
    let source =
        written(&directory, "ERL.BAS", b"DEFINT A-Z\nON ERROR GOTO h\n100 ERROR 5\nPRINT ERL\nEND\nh:\nRESUME NEXT\n");
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = text.split("define ").find(|one| one.contains("void @__main()")).expect("the body");
    // Lined, as a module handling errors is: `, !dbg`.
    assert!(body.contains("load i16, ptr @$QB$ERL\n") || body.contains("load i16, ptr @$QB$ERL, !dbg"), "{body}");
}

/// DEF SEG stored to a b$seg the rich route defined in the module's own
/// data, so every BSAVE saved from the runtime's b$seg: DGROUP.
#[test]
fn test_def_seg_stores_the_runtime_segment_cell() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "defseg.bas", b"DEF SEG = &HA000\nBSAVE \"V.BIN\", 0, 64000\n");
    let program = parsed_as(&source, "qb45", "qb45");
    let module =
        qb_compile::assembled(&program, None, &llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone()))
            .expect("assembles");
    assert!(module.externs.iter().any(|(name, _)| name == "b$seg"), "{}", masm::text(&module).expect("prints"));
}

/// Load speculation takes a word array to lie at an even offset; placed
/// after three bytes, it lay at an odd one. The array is read at another
/// index than it is written: read back at `k`, the load folds to the store and
/// the array, written and never read, is deleted.
#[test]
fn test_a_word_array_after_an_odd_object_lies_at_an_even_offset() {
    let directory = tempfile::tempdir().expect("a directory");
    let text = "DEFINT A-Z\r\nDIM SHARED s AS STRING * 3, f3(10)\r\ns = \"abc\"\r\nk = LEN(COMMAND$)\r\nf3(k) = LEN(s)\r\nPRINT f3(k + 1), s\r\n";
    let program = parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45");
    let codegen = llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone());
    let module = qb_compile::assembled(&program, None, &codegen).expect("assembles");
    let (_, items) = module.data.iter().find(|(name, _)| name == "BC_DATA").expect("BC_DATA");
    let mut offset = 0;
    for item in items {
        match item {
            masm::Datum::Label(label) | masm::Datum::Object(label) if label.name == "F3%" => break,
            masm::Datum::Bytes(bytes) => offset += bytes.len() as i64,
            masm::Datum::Pointer(pointer) => offset += if pointer.far { 4 } else { 2 },
            masm::Datum::SegmentWord(_) => offset += 2,
            masm::Datum::Fill(fill) => offset += fill.size,
            masm::Datum::Align(align) => offset += (-offset).rem_euclid(align.to),
            _ => {}
        }
    }
    assert!(
        items.iter().any(|item| matches!(item, masm::Datum::Label(label) if label.name == "F3%")),
        "{}",
        masm::text(&module).expect("prints")
    );
    assert_eq!(offset % 2, 0, "{}", masm::text(&module).expect("prints"));
}

/// deedlines' PLASMABLOBS read `fsin3%(y% - k%)` again on every pixel that
/// took the rare branch: hoist runs a load the loop may skip only where it
/// cannot fault, and a word read crossing offset FFFFh faults. A word
/// array's element is word aligned, so it cannot cross.
#[test]
fn test_a_word_array_read_the_loop_may_skip_leaves_the_loop() {
    let directory = tempfile::tempdir().expect("a directory");
    let text = "DEFINT A-Z\r\nDECLARE SUB blobs ()\r\nDIM SHARED cd(100), f3(100), xp(3)\r\nblobs\r\nSUB blobs\r\nk = xp(1)\r\nFOR y = 0 TO 99\r\nDEF SEG = &HA000 + y\r\nFOR x = 0 TO 99\r\n\
d = cd(x)\r\nIF d < 50 THEN POKE x, d ELSE POKE x, f3(y - k)\r\nNEXT x\r\nNEXT y\r\nDEF SEG\r\nEND SUB\r\n";
    let listing = listing(&parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45"));
    let lines = between(&listing, "BLOBS proc", "BLOBS endp").lines().collect::<Vec<_>>();
    let store = lines.iter().position(|line| line.contains("byte ptr es:[")).expect("the POKE");
    // The innermost loop: the nearest label above the POKE a later jump returns
    // to.
    let (top, bottom) = (0..store)
        .rev()
        .filter(|&at| lines[at].ends_with(':'))
        .find_map(|at| {
            let label = lines[at].trim_end_matches(':');
            lines[store..]
                .iter()
                .position(|line| line.trim_start().starts_with('j') && line.ends_with(label))
                .map(|back| (at, store + back))
        })
        .expect("the pixel loop");
    let inner = lines[top..=bottom].join("\n");
    // Besides the frame, the loop reads only through a named array: `cd(x)`.
    assert!(!inner.lines().any(|line| line.contains("ptr [") && !line.contains("ptr [bp")), "{inner}");
}

/// llrm-qb -S printed the native BP shell the object strips where B$ENRA
/// owns the frame, so a SUB read its parameters 2 bytes off from what ran.
#[test]
fn test_the_listing_is_the_code_the_object_holds() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "shell.bas",
        b"DECLARE SUB Keep (x AS INTEGER)\nSUB Keep (x AS INTEGER)\nx = x + 1\nEND SUB\n",
    );
    let program = qb_driver::parsed(
        &source,
        &qb_driver::Frontend { runtime_frames: true, ..qb_driver::Frontend::new("vbdos", "vbdos") },
        None,
    )
    .expect("parses");
    let codegen = llrm_driver::m16_options_old(llrm_x86_m16::machine::BASIC.clone());
    let module = qb_compile::assembled(&program, None, &codegen).expect("assembles");
    let text = llrm_codegen::driver::basic::text(&module).unwrap();
    let keep = between(&text, "KEEP proc far", "KEEP endp");
    assert!(!keep.contains("push bp") && !keep.contains("pop bp") && keep.contains("call far ptr B$ENRA"), "{keep}");
}

/// A fixed-length string passed to a STRING parameter goes as a copy that BC
/// assigns back with B$ASSN after the call; llrm-qb only released the copy,
/// so pFix's caller kept 'abcdef' where the callee wrote 'XY'.
#[test]
fn test_a_fixed_length_argument_is_assigned_back_after_the_call() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "fixback.bas", b"DECLARE SUB Fill (s AS STRING)\nDIM f AS STRING * 6\nf = \"abcdef\"\nFill f\nPRINT f\nSUB Fill (s AS STRING)\ns = \"XY\"\nEND SUB\n");
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("vbdos", "vbdos"), None).expect("parses");
    let text = listing(&program);
    // FILL is inlined into the main body, so the last assignment back and the
    // last release are the call's.
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let assigned = main.rfind("call far ptr B$ASSN").expect("an assignment back");
    assert!(main.rfind("call far ptr B$STDL").is_none_or(|released| assigned < released), "{main}");
}

/// qb-qrender's main.bas runs from its body into its ON ERROR handler's
/// label, and the rich route refused it: "@__main: block 2, which both the
/// body and its error handler run". The shared code is in both: in the
/// body its RESUME raises 20, "RESUME without error"; the handler run to the
/// module's end raises 19, "No RESUME".
#[test]
fn test_a_body_that_falls_into_its_handler_compiles_on_the_rich_route() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = "10 ON ERROR GOTO 100\n20 ERROR 5\n30 PRINT \"body\"; ERL\n100 PRINT \"h\"; ERR; ERL\n110 IF ERR = 0 THEN RESUME NEXT\n";
    let path = written(&directory, "fall.bas", source.as_bytes());
    let text = listing_calls_kept(&parsed_as(&path, "vbdos", "vbdos"));
    let body = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let handler = between(&text, "__main$handler proc", "__main$handler endp");
    assert!(body.contains("pushw 20\n    call far ptr B$SERR"), "{body}");
    assert!(handler.contains("pushw 19\n    call far ptr B$SERR"), "{handler}");
    for code in [body, handler] {
        assert!(code.contains("call far ptr B$PSI2"), "{code}");
    }
}

/// Whether `instruction` calls B$SERR with constant `number`.
fn raises(
    instruction: &llrm_codegen::hir::model::Instruction,
    number: i64,
) -> bool {
    instruction.callee.as_deref() == Some("B$SERR")
        && matches!(
            instruction.operands.as_slice(),
            [llrm_codegen::hir::model::Operand::Constant(one)] if one.value == llrm_codegen::hir::model::Number::Int(number.into())
        )
}

/// `k = 10 \ k` traps in the processor's divide, which names no statement:
/// the rich route kept the last call's, and `checked-division.bas` looped
/// printing `h 11 22` where BC prints `h 11 42`. `--checked-division`
/// raises BC's error 11 in code, as ERROR does, and wraps a LONG MIN by -1
/// as BC's software divide does; without it nothing changes.
#[test]
fn checked_division_raises_error_11_where_bcs_divide_traps() {
    let hir = |checked_division| {
        let frontend = qb_driver::Frontend { checked_division, ..qb_driver::Frontend::new("vbdos", "vbdos") };
        qb_driver::parsed(&fixture("checked-division.bas"), &frontend, None).expect("parses")
    };
    let instructions = |program: &llrm_codegen::hir::model::Program| -> Vec<llrm_codegen::hir::model::Instruction> {
        let main = program.modules[0].functions.iter().find(|one| one.name == "__main").expect("the module body");
        main.blocks.iter().flat_map(|block| block.instructions.clone()).collect()
    };
    // Premise: divisions by variables, in a body whose errors land.
    let plain = hir(false);
    assert!(plain.modules[0].functions.iter().any(|one| one.name == "__main" && one.error_handler.is_some()));
    let divisions = instructions(&plain)
        .iter()
        .filter(|one| {
            matches!(
                one.op,
                llrm_codegen::hir::model::Op::Div | llrm_codegen::hir::model::Op::Rem
            )
                && !matches!(
                    one.operands[1],
                    llrm_codegen::hir::model::Operand::Constant(_)
                )
        })
        .count();
    assert!(divisions >= 3, "{divisions}");
    // The zero divisor is code with and without the option (MIR's sdiv by zero
    // is undefined: -O2 deleted an unused quotient and no error 11 was raised,
    // QB-DIVZERO-1); the option no longer adds the zeros.
    let raised = |program: &llrm_codegen::hir::model::Program| {
        instructions(program).iter().filter(|one| raises(one, 11)).count()
    };
    let checked = hir(true);
    assert!(raised(&plain) >= divisions, "{} {divisions}", raised(&plain));
    assert!(raised(&checked) >= divisions, "{checked:?}");
}

/// The same on the rich route: the error is a call the landing pad names.
#[test]
fn test_checked_division_raises_on_the_rich_route() {
    let frontend = qb_driver::Frontend { checked_division: true, ..qb_driver::Frontend::new("vbdos", "vbdos") };
    let text = listing(&qb_driver::parsed(&fixture("checked-division.bas"), &frontend, None).expect("parses"));
    assert!(text.contains("pushw 11\n    call far ptr B$SERR"), "{text}");
    // Without the option a division by a variable is raised as code too, with
    // or without a handler (QB-DIVZERO-1: the processor's trap is undefined in
    // MIR and -O2 deleted it).
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "plain.bas", b"DEFINT A-Z\nINPUT d\nPRINT 10 \\ d\n");
    assert!(listing(&parsed_as(&path, "vbdos", "vbdos")).contains("pushw 11"));
}

/// An error raised in a FUNCTION called from a SUB, with the handler in the
/// module body, resumed at the failing divide for ever (#79): the divide's
/// trap names no statement, so the pad resumed the last call's, and the
/// optimizer saw no edge from the divide to the pad. Where errors land, a
/// divide by a variable raises error 11 as code, in the FUNCTION's own pad.
#[test]
fn test_a_division_in_a_called_function_raises_to_its_pad() {
    let source = b"DECLARE SUB caller (d AS INTEGER)\nDECLARE FUNCTION inner% (d AS INTEGER)\nON ERROR GOTO h\ncaller 0\nEND\nh:\nRESUME NEXT\nSUB caller (d AS INTEGER)\nDIM b AS INTEGER\nb = inner(d)\nEND SUB\nFUNCTION inner% (d AS INTEGER)\ninner% = 10 \\ d\nEND FUNCTION\n";
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "called.bas", source);
    let text = listing(&parsed_as(&path, "vbdos", "vbdos"));
    let inner = &text[text.find("INNER").expect("the function")..];
    assert!(inner.contains("pushw 11\n    call far ptr B$SERR"), "{text}");
}

/// BC's LONG divide is software: MIN by -1 wraps where the processor's
/// traps, and `checked-division.bas` printed nothing past it. Under
/// `--checked-division` it prints BC's -2147483648 and 0.
#[test]
fn checked_division_wraps_a_long_min_by_minus_one() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(
        &directory,
        "wrap.bas",
        b"DIM a AS LONG, b AS LONG\na = -2147483647 - 1: b = -1\nPRINT a \\ b; a MOD b\n",
    );
    let frontend = qb_driver::Frontend { checked_division: true, ..qb_driver::Frontend::new("vbdos", "vbdos") };
    let program = qb_driver::parsed(&path, &frontend, None).expect("parses");
    let run = llrm_codegen::hir::execute::run(&program, "__main", &[]).expect("runs");
    assert_eq!((run.output.as_str(), run.panic), ("-2147483648  0 \n", None));
}

/// The module body's listing on the rich route.
fn module_body(program: &llrm_codegen::hir::model::Program) -> String {
    let text = listing(program);
    between(&text, "$QB$MAIN proc", "$QB$MAIN endp").to_owned()
}

/// A module body that needs frame storage (here, the error site of its
/// two ERRORs) was framed by B$ENRA, and QB 4.5 read that frame's missing
/// return address as the error's: `ERROR 7` reported "Out of stack space in
/// line 49152 of module $ p at address 0000:0824". BC's module-level code
/// has no frame; the body's is now a static data object, BP the runtime's.
#[test]
fn a_module_body_keeps_its_frame_in_static_data() {
    // `10 ON ERROR GOTO 100: 20 ERROR 5: 25 ERROR 6: 27 ON ERROR GOTO 0`
    // `28 ERROR 7: 30 END: 100 PRINT "h"; ERR: 110 RESUME NEXT`
    let program = qb_driver::decoded(include_str!("fixtures/static_main_frame.json"), false, 0).expect("decodes");
    let body = module_body(&program);
    // Premise: the body stores into a frame, BP's or the static one.
    assert!(body.contains("[bp-") || body.contains("$QB$FRAME"), "{body}");
    assert!(!body.contains("B$ENRA") && !body.contains("[bp"), "{body}");
    assert!(body.contains("$QB$FRAME"), "{body}");
}

/// The same from source, on each runtime.
#[test]
fn test_a_module_body_with_two_error_sites_takes_no_runtime_frame() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = "10 ON ERROR GOTO 100\n20 ERROR 5\n25 ERROR 6\n30 END\n100 PRINT \"h\"; ERR\n110 RESUME NEXT\n";
    let path = written(&directory, "frame.bas", source.as_bytes());
    for runtime in ["qb45", "pds71", "vbdos"] {
        let body = module_body(&parsed_as(&path, runtime, runtime));
        assert!(!body.contains("B$ENRA") && body.contains("$QB$FRAME"), "{runtime}: {body}");
    }
}
/// What `source` prints and the error it ends with, under `frontend`.
fn run_as(
    source: &[u8],
    frontend: &qb_driver::Frontend,
) -> (String, Option<String>) {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "run.bas", source);
    let program = qb_driver::parsed(&path, frontend, None).expect("parses");
    let run = llrm_codegen::hir::execute::run(&program, "__main", &[]).expect("runs");
    (run.output, run.panic)
}

/// BC's /D raises Overflow where INTEGER and LONG + - and negation, INTEGER
/// *, and LONG to INTEGER overflow; LONG * (software) and floating to
/// integer it leaves. Measured on VBDOS BC /D; without the check each of
/// these wrapped, `i = 32767 + 1` printing -32768.
#[test]
fn checked_overflow_raises_error_6_where_bcs_debug_code_does() {
    let checked = qb_driver::Frontend { checked_overflow: true, ..qb_driver::Frontend::new("vbdos", "vbdos") };
    let plain = qb_driver::Frontend::new("vbdos", "vbdos");
    let declared = "DIM i AS INTEGER, a AS LONG, x AS SINGLE\n";
    for statement in [
        "i = 32767: i = i + 1",
        "i = -32768: i = i - 1",
        "i = 200: i = i * i",
        "i = -32768: i = -i",
        "a = 2147483647: a = a + 1",
        "a = -2147483647: a = a - 2",
        "a = -2147483647 - 1: a = -a",
        "a = 40000: i = a",
    ] {
        let source = format!("{declared}{statement}\nPRINT \"wrapped\"\n");
        assert_eq!(run_as(source.as_bytes(), &plain), ("wrapped\n".to_owned(), None), "{statement}");
        assert_eq!(run_as(source.as_bytes(), &checked), (String::new(), Some("error 6".to_owned())), "{statement}");
    }
    for statement in ["a = 65536: a = a * a", "x = 40000: i = x"] {
        let source = format!("{declared}{statement}\nPRINT \"wrapped\"\n");
        assert_eq!(run_as(source.as_bytes(), &checked), ("wrapped\n".to_owned(), None), "{statement}");
    }
}

/// `checked-overflow.bas` is BC /D's overflow cases: each checked operation
/// raises error 6 in the HIR only under the check.
#[test]
fn checked_overflow_raises_in_the_hir_of_each_overflowing_operation() {
    let hir = |checked_overflow| {
        let frontend = qb_driver::Frontend { checked_overflow, ..qb_driver::Frontend::new("vbdos", "vbdos") };
        let program = qb_driver::parsed(&fixture("checked-overflow.bas"), &frontend, None).expect("parses");
        let main =
            program.modules[0].functions.iter().find(|one| one.name == "__main").expect("the module body").clone();
        main.blocks.iter().flat_map(|block| block.instructions.clone()).collect::<Vec<_>>()
    };
    use llrm_codegen::hir::model::Op;
    // Premise: adds, subtracts, a multiply, negations and a narrowing.
    let plain = hir(false);
    for op in [Op::Add, Op::Sub, Op::Mul, Op::Neg, Op::Convert] {
        assert!(plain.iter().any(|one| one.op == op), "{op:?}");
    }
    assert!(!plain.iter().any(|one| raises(one, 6)));
    // Lines 20 to 38: LONG + - and negation, INTEGER - * and negation, CINT
    // of a LONG; LONG * and the floating conversions stay unchecked.
    assert!(hir(true).iter().filter(|one| raises(one, 6)).count() >= 7);
}

/// llrm-qb takes gcc's sanitizer names for BC's /D checks.
#[test]
fn sanitizers_select_bcs_debug_checks() {
    let frontend =
        |flag: &str| super::cli::parse_args(&["x.bas".to_owned(), flag.to_owned()]).expect("parses").frontend;
    let all = frontend("-fsanitize=undefined");
    assert!(all.checked_arrays && all.checked_division && all.checked_overflow);
    assert!(frontend("-fsanitize=bounds").checked_arrays && !frontend("-fsanitize=bounds").checked_overflow);
    assert!(frontend("-ftrapv").checked_overflow);
    assert!(!frontend("-O2").checked_arrays);
}

/// Each call in a body that handles errors comes after a statement row:
/// the row locates an error the call raises, and RESUME continues from it.
fn rows_precede_calls(program: &llrm_codegen::hir::model::Program) -> Vec<(String, i64)> {
    let module = &program.modules[0];
    let rows = module.statements().expect("a statement table");
    let mut orphans = Vec::new();
    for function in &module.functions {
        let first = rows.iter().filter(|one| one.function == function.id).map(|one| one.instruction).min();
        let Some(first) = first else { continue };
        for call in function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|one| one.op == llrm_codegen::hir::model::Op::Call && one.id < first)
        {
            orphans.push((function.name.clone(), call.id));
        }
    }
    orphans
}

/// GORILLA.BAS DIMs its dynamic arrays before InitVars sets ON ERROR, and
/// the rich route refused it: "@__main: a call before the first
/// statement". Rows began only once ON ERROR was compiled, and DIM, though
/// a dynamic one allocates, was no statement.
#[test]
fn a_dynamic_dim_before_on_error_is_a_statement() {
    let program = parsed_as(&fixture("dim-before-on-error.bas"), "qb45", "qb45");
    // Premise: B$DDIM runs in the module body and in S, both handled.
    let calls = |name: &str| {
        program.modules[0]
            .functions
            .iter()
            .find(|one| one.name == name)
            .expect("the function")
            .blocks
            .iter()
            .flat_map(|block| block.instructions.clone())
            .filter(|one| one.callee.as_deref() == Some("B$DDIM"))
            .count()
    };
    assert!(calls("__main") > 0 && calls("S") > 0);
    assert_eq!(rows_precede_calls(&program), Vec::<(String, i64)>::new());
    listing(&program);
}

/// The DATA rows' markers leave the rich route's input, and their ABI with
/// them: left behind, the HIR no longer encodes ("ABI site 33 is not a call").
#[test]
fn positional_data_drops_the_markers_abi_too() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "data.bas", b"READ a\nPRINT a\nEND\nDATA 1\n");
    let program = super::compile::_positional_data(&parsed_as(&path, "qb45", "qb45")).expect("lays DATA out");
    llrm_codegen::hir::codec::encode(&program, None).expect("encodes");
}

/// The statement table's rows of `listing`: each label and BASIC line.
fn statement_rows(listing: &str) -> Vec<(String, u16)> {
    let table = between(listing, "$QB$STAT proc", "$QB$STAT endp");
    let lines: Vec<&str> = table.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter_map(|pair| {
            let label = pair[0].strip_prefix("dw offset ")?;
            let bytes: Vec<u8> = pair[1]
                .strip_prefix("db ")?
                .split(',')
                .map(|one| u8::from_str_radix(one.trim_end_matches('h'), 16).expect("a byte"))
                .collect();
            Some((label.to_owned(), u16::from_le_bytes([bytes[0], bytes[1]])))
        })
        .collect()
}

/// The lines the outlined handler's rows state.
fn handler_lines(program: &llrm_codegen::hir::model::Program) -> Vec<u16> {
    let listing = listing(program);
    // Premise: the handler is its own procedure, assembled second.
    assert!(listing.contains("__main$handler proc"), "{listing}");
    statement_rows(&listing).into_iter().filter(|(label, _)| label.starts_with("L1_")).map(|(_, line)| line).collect()
}

/// Under `--error-lines`, an error in the outlined module handler (`110
/// ERROR 7`, or its end without RESUME) reported "in line 0": the handler
/// had only its line-0 entry row. BC reports lines 110 and 100. Off by
/// default: the rows cost 4 bytes a line.
#[test]
fn an_error_in_the_outlined_handler_reports_its_line() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let raising = "10 ON ERROR GOTO 100\n20 ERROR 5\n30 END\n100 PRINT \"h\"; ERR\n110 ERROR 7\n";
    let unresumed = "10 ON ERROR GOTO 100\n20 ERROR 5\n30 END\n100 PRINT \"h\"; ERR\n";
    for (source, lines) in [(raising, vec![100, 110]), (unresumed, vec![100])] {
        let path = written(&directory, "handler.bas", source.as_bytes());
        for runtime in ["qb45", "pds71", "vbdos"] {
            let lined = qb_driver::Frontend { error_lines: true, ..qb_driver::Frontend::new(runtime, runtime) };
            let found = handler_lines(&qb_driver::parsed(&path, &lined, None).expect("parses"));
            assert!(lines.iter().all(|one| found.contains(one)), "{runtime}: {found:?}");
            assert_eq!(handler_lines(&parsed_as(&path, runtime, runtime)), [0], "{runtime}: off by default");
        }
    }
}

/// The same, pinned at the backend's input.
#[test]
fn an_outlined_handler_takes_a_row_per_line() {
    let program = qb_driver::decoded(include_str!("fixtures/outlined_handler_lines.json"), false, 0).expect("decodes");
    // Premise: the frontend states each line's BASIC number.
    assert!(program.modules[0].line_numbers.contains(&(5, 110)));
    let found = handler_lines(&program);
    assert!(found.contains(&100) && found.contains(&110), "{found:?}");
}

/// END and SYSTEM call `B$CEND`, which "does not return" (rt/rtterm.asm): the
/// runtime description says so, but its declaration did not, and the call's
/// block ended only by the `unreachable` after it. The program's promises now
/// name the routines that never come back, and their declarations are
/// `noreturn`.
#[test]
fn the_runtime_routines_that_never_return_are_declared_noreturn() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "end.bas", b"DEFINT A-Z\r\nINPUT a\r\nIF a THEN END\r\nPRINT 1\r\n");
    let mir = emitted_mir(&parsed_as(&path, "qb45", "qb45"));
    let declaration = |name: &str| {
        mir.lines()
            .find(|one| one.starts_with("declare") && one.contains(&format!("@llrm.qb.{name}(")))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert!(declaration("B$CEND").contains("noreturn"), "{}", declaration("B$CEND"));
    assert!(!declaration("B$PEI2").contains("noreturn"), "{}", declaration("B$PEI2"));
}

/// A FOR counter's add was `nsw` whatever the counter's type: an UNSIGNED
/// INTEGER passes 32767 legitimately, so `nsw` was poison there. An unsigned
/// counter's add does not wrap as unsigned (BC hangs if it would), `nuw`.
#[test]
fn a_for_counters_add_states_the_wrap_its_type_cannot_do() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = |declared: &str| {
        format!(
            "DIM c AS {declared}, n AS {declared}, s AS LONG\r\nn = 30100\r\nFOR c = 30000 TO n\r\ns = s + 1\r\nNEXT c\r\nPRINT s\r\n"
        )
    };
    let adds = |declared: &str| {
        let mir =
            emitted_mir(&parsed_as(&written(&directory, "count.bas", source(declared).as_bytes()), "quickr", "qb45"));
        mir.lines().filter(|one| one.contains(" = add ") && one.contains("i16")).map(str::to_owned).collect::<Vec<_>>()
    };
    let unsigned = adds("UNSIGNED INTEGER");
    assert!(
        unsigned.iter().any(|one| one.contains("add nuw i16")) && !unsigned.iter().any(|one| one.contains("nsw")),
        "{unsigned:?}"
    );
    let signed = adds("INTEGER");
    assert!(
        signed.iter().any(|one| one.contains("add nsw i16")) && !signed.iter().any(|one| one.contains("nuw")),
        "{signed:?}"
    );
}

/// A POKE writes only the byte it addresses. Volatile was a barrier, so
/// after #257 TEXTFILL's Fill loop read `ch`, `at` and `b$seg` (twice)
/// again every trip, 4 loads, though DEF SEG had put the segment at video
/// memory. Through `llrm-qb -O2 -march=i486 -S`, as shipped.
#[test]
fn a_poke_to_video_memory_leaves_the_loop_its_variables_and_segment() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let out = directory.path().join("textfill.asm");
    let source = root().join("tests/inputs/qb/textfill.bas");
    let argv = [source.to_str().unwrap(), "-O2", "-march=i486", "-S", "-o", out.to_str().unwrap()].map(str::to_owned);
    assert_eq!(crate::cli::main(&argv), 0);
    let text = std::fs::read_to_string(&out).expect("the listing");
    let fill = between(&text, "FILL proc", "FILL endp").lines().map(str::trim).collect::<Vec<_>>();
    let at = fill.iter().rposition(|one| one.starts_with('j') && !one.starts_with("jmp")).expect("the loop's branch");
    let top = fill
        .iter()
        .position(|one| *one == format!("{}:", fill[at].split_whitespace().nth(1).unwrap()))
        .expect("its label");
    let body = &fill[top..=at];
    let loads = body.iter().filter(|one| one.contains("ptr [") || one.contains("b$seg")).count();
    let pokes = body.iter().filter(|one| one.starts_with("mov byte ptr es:[")).count();
    // The pin's output: `opt -passes='loop-mssa(licm)'` and `default<O2>` hoist
    // neither cell past a volatile far store (4f82e22e1, #3994 states that
    // video memory holds no program data). Both POKEs still land every
    // trip.
    assert_eq!((loads, pokes), (2, 2), "{body:#?}");
}

/// The listing of `source` through `llrm-qb -O2 -march=i486 -S`, as shipped.
fn listing_of(
    name: &str,
    source: &str,
) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let out = directory.path().join(format!("{name}.asm"));
    let input = written(&directory, &format!("{name}.bas"), source.as_bytes());
    let argv = [input.to_str().unwrap(), "-O2", "--isel=old", "-march=i486", "-S", "-o", out.to_str().unwrap()]
        .map(str::to_owned);
    assert_eq!(crate::cli::main(&argv), 0);
    std::fs::read_to_string(&out).expect("the listing")
}

/// READ keeps nothing of its destination's address, so a later PRINT cannot
/// write the cell: `n` was loaded again after `PRINT x#;` though nothing
/// stored to it (B$RDI2 was a pointer that might be kept).
#[test]
fn test_a_cell_read_into_is_not_reloaded_after_a_print() {
    let text = listing_of(
        "read_then_print",
        "DEFINT A-Z\r\nCOMMON SHARED n\r\nDIM x AS DOUBLE\r\nREAD n\r\nDATA 7\r\nx = 2.5\r\na = n\r\nPRINT x;\r\nb = n\r\nPRINT a; b\r\nEND\r\n",
    );
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let loads = main.lines().map(str::trim).filter(|one| one.starts_with("mov") && one.ends_with("ptr N%")).count();
    assert_eq!(loads, 1, "{main}");
}

/// `DEF SEG = VARSEG(x)` then `POKE VARPTR(x) + k, ...` writes into x: the
/// pointer `ADDRESS(x) + k`, not a segment loaded from `b$seg` and an offset,
/// which a loop's cells and `b$seg` itself could not be kept in registers
/// across (far_poke_loop_variable.bas, +33 B and +266 clocks against main).
#[test]
fn a_poke_after_def_seg_varseg_is_a_store_into_the_variable() {
    let text = listing_of(
        "varseg_poke",
        "DEFINT A-Z\r\nDIM SHARED x AS INTEGER, y AS LONG\r\nx = 0\r\nDEF SEG = VARSEG(x)\r\nFOR i = 0 TO 3000\r\nPOKE VARPTR(x), (i * 37) AND 255\r\nPOKE VARPTR(x) + 1, (i * 11) AND 127\r\ny = y + x\r\nNEXT i\r\nDEF SEG\r\nPRINT x; y\r\nEND\r\n",
    );
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let lines = main.lines().map(str::trim).collect::<Vec<_>>();
    let at = lines.iter().rposition(|one| one.starts_with('j') && !one.starts_with("jmp")).expect("the loop's branch");
    let top = lines
        .iter()
        .position(|one| *one == format!("{}:", lines[at].split_whitespace().nth(1).unwrap()))
        .expect("its label");
    let body = &lines[top..=at];
    assert!(!body.iter().any(|one| one.contains("b$seg")), "{body:#?}");
}

/// A POKE after a literal DEF SEG wrote memory no variable lives in, yet
/// the loop below reloaded `b$seg` (and the segment register from it) before
/// every byte: the volatile store was a barrier to every cell.
#[test]
fn a_poke_after_a_literal_def_seg_leaves_the_segment_cell_in_a_register() {
    let text = listing_of(
        "literal_segment",
        "SUB Fill\r\nDEFINT A-Z\r\nDEF SEG = &HB800\r\nFOR i = 0 TO 3998 STEP 2\r\nPOKE i, 65\r\nNEXT i\r\nDEF SEG\r\nEND SUB\r\nFill\r\nEND\r\n",
    );
    let fill = between(&text, "FILL proc", "FILL endp").lines().map(str::trim).collect::<Vec<_>>();
    let at = fill.iter().rposition(|one| one.starts_with('j') && !one.starts_with("jmp")).expect("the loop's branch");
    let top = fill
        .iter()
        .position(|one| *one == format!("{}:", fill[at].split_whitespace().nth(1).unwrap()))
        .expect("its label");
    let body = &fill[top..=at];
    assert!(!body.iter().any(|one| one.contains("b$seg")), "{body:#?}");
}

/// DEF SEG = VARSEG(x) points the segment at the program's own data, so the
/// POKE below changes `x`: PRINT must read it again, not print the 1 stored
/// before. The absolute-memory tag is for literal segments only.
#[test]
fn a_poke_through_varseg_keeps_the_variable_it_can_write() {
    let text = listing_of(
        "varseg_poke",
        "DEFINT A-Z\r\nDIM x AS INTEGER\r\nx = 1\r\nDEF SEG = VARSEG(x)\r\nPOKE VARPTR(x), 7\r\nPRINT x\r\nEND\r\n",
    );
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let poke = lines.iter().position(|one| one.starts_with("mov byte ptr es:[")).expect("the POKE");
    assert!(lines[poke..].iter().any(|one| one.contains("word ptr X%")), "{text}");
}

/// A WAIT's masks sat in two byte cells of the data group and were reloaded
/// every poll at -m32 (`xor al, byte ptr W$D1+1; mov cl, byte ptr W$D1; and al,
/// cl`, or one word store of both): the flat target's machine took every port
/// for one that may reach memory, so the `in` kept the loads from being
/// forwarded across it, and -m16 (whose table says the VGA status port reaches
/// none) did not. A target's ports are its platform's.
#[test]
fn a_port_poll_does_not_reload_a_cell_stored_before_it_at_either_target() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = written(&directory, "wait.bas", b"WAIT &H3DA, 8, 8\r\nEND\r\n");
    for target in ["-m16", "-m32"] {
        let out = directory.path().join(format!("wait{target}.asm"));
        let argv = [source.to_str().unwrap(), target, "-O2", "-march=i486", "-S", "-o", out.to_str().unwrap()]
            .map(str::to_owned);
        assert_eq!(crate::cli::main(&argv), 0, "{target}");
        let text = std::fs::read_to_string(&out).expect("the listing");
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let poll = lines.iter().position(|one| *one == "in al, dx").expect("the port read");
        // The premise: the masks are known constants at the poll, in the
        // instruction and not in a cell. `(x ^ 8) & 8` is nonzero when bit 3 is
        // clear, one `test al, 8`: the pin's code for the same loop (clang
        // 4b787ddb, i486: `in al, dx; test al, 8; jne`).
        assert_eq!(lines[poll + 1], "test al, 8", "{target}: {text}");
        assert!(!text.contains("W$D"), "{target}: a cell held the masks: {text}");
    }
}

/// A WAIT stored each mask into a cell of the data group and loaded it again in
/// the poll, whatever the mask: a constant is the same in the poll block, and
/// needs no cell to get there (the port already was used so). Unoptimized, as
/// shipped for -O0; a mask that is not constant is still read once, before the
/// loop, and kept in a cell.
#[test]
fn a_constant_wait_mask_goes_to_the_poll_as_it_is_and_a_variable_one_through_a_cell() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let listing_of = |name: &str, text: &[u8], target: &str| {
        let source = written(&directory, name, text);
        let out = directory.path().join(format!("{name}{target}.asm"));
        let argv = [source.to_str().unwrap(), target, "-O0", "-march=i486", "-S", "-o", out.to_str().unwrap()]
            .map(str::to_owned);
        assert_eq!(crate::cli::main(&argv), 0, "{target}");
        std::fs::read_to_string(&out).expect("the listing")
    };
    for target in ["-m16", "-m32"] {
        let constant = listing_of("constant.bas", b"WAIT &H3DA, 8, 8\r\nEND\r\n", target);
        // The test of the masked byte may be a `test` for the `and` it needs no
        // result of.
        assert!(
            (constant.contains("xor al, 8") || constant.contains("cmp al, 8") || constant.contains("not al"))
                && (constant.contains("and al, 8") || constant.contains("test al, 8")),
            "{target}: {constant}"
        );
        assert!(!constant.contains("ptr W$D"), "{target}: a cell held a constant mask: {constant}");
        let variable = listing_of("variable.bas", b"DEFINT A-Z\r\nm = 8\r\nWAIT &H3DA, m\r\nEND\r\n", target);
        let lines: Vec<&str> = variable.lines().map(str::trim).collect();
        let poll = lines.iter().position(|one| *one == "in al, dx").expect("the port read");
        assert!(
            lines[poll + 1].contains("byte ptr"),
            "{target}: the mask is read from its cell in the poll: {variable}"
        );
    }
}

/// PEEK reads and POKE writes memory every time, whatever DEF SEG says: the
/// BIOS tick wait `DO: LOOP UNTIL PEEK(&H6C) <> t` compiled to an infinite
/// loop (`cmp ax, ax`), the second PEEK taken for the first's value, and a
/// second POKE of one address was dropped as dead.
#[test]
fn peek_and_poke_touch_memory_every_time() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = b"DEFINT A-Z\r\nDEF SEG = &H40\r\nt = PEEK(&H6C)\r\nDO\r\nLOOP UNTIL PEEK(&H6C) <> t\r\nPOKE &H6C, 0\r\nPOKE &H6C, 1\r\nPRINT 1\r\n";
    let path = written(&directory, "tick.bas", source);
    let text = listing(&parsed_as(&path, "qb45", "qb45"));
    assert!(source.windows(5).any(|one| one == b"PEEK("), "the shape that was folded");
    // The loop re-reads: a label, a byte read of the tick (loaded, or compared
    // in place), and a jump back to it. The address is 108 or, hoisted into bx
    // (a shorter encoding in the loop), `bx`.
    let read = regex::Regex::new(
        r"(L\d+_\d+):\n\s+(?:movzx \w+, byte ptr es:\[(?:108|bx)\]|cmp byte ptr es:\[(?:108|bx)\], \w+|cmp \w+, byte ptr es:\[(?:108|bx)\])\n",
    )
    .unwrap();
    let back = |label: &str| regex::Regex::new(&format!(r"\s+j\w+ {label}\n")).unwrap().is_match(&text);
    assert!(read.captures_iter(&text).any(|found| back(&found[1])), "{text}");
    assert_eq!(regex::Regex::new(r"mov byte ptr es:\[(?:108|bx)\],").unwrap().find_iter(&text).count(), 2, "{text}");
}

/// Every dynamic array shared one `allocation` tag, so a store to `a(i)` was
/// a write of `b(i)`: `b(i)` was loaded again for `c(i) = a(i) + b(i)` (#113).
/// Each array a function owns is a type of its own under `allocation`; a
/// parameter's array and a BYREF element keep no tag, and so alias them all.
#[test]
fn two_dynamic_arrays_are_apart_but_a_parameters_array_is_not() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = |fill: &str| {
        format!(
            "DECLARE SUB fill (z() AS INTEGER, n AS INTEGER)\r\nDEFINT A-Z\r\nREM $DYNAMIC\r\nDIM SHARED a(100) AS INTEGER, b(100) AS INTEGER\r\nDIM c(100) AS INTEGER\r\nINPUT n\r\nFOR i = 1 TO n\r\na(i) = b(i) + 1\r\nc(i) = a(i) + b(i)\r\nNEXT\r\n{fill}PRINT c(3)\r\nSUB fill (z() AS INTEGER, n AS INTEGER)\r\nz(1) = n\r\nEND SUB\r\n"
        )
    };
    let loads_of_b = |text: &str| {
        let mir = optimized_mir(&parsed_as(&written(&directory, "arrays.bas", text.as_bytes()), "qb45", "qb45"));
        let at = mir.find("define internal cc 1000 void @__main").expect("the body");
        let body = &mir[at..at + mir[at..].find("\n}\n").expect("its end")];
        // b's tag is the first array's: the one loaded first.
        let first = body
            .lines()
            .find(|one| one.contains("load i16") && one.contains("addrspace(1)"))
            .and_then(|one| one.split("!tbaa ").nth(1))
            .map(str::to_owned)
            .expect("a far load");
        body.lines().filter(|one| one.contains("load i16") && one.ends_with(&format!("!tbaa {first}"))).count()
    };
    let apart = loads_of_b(&source(""));
    assert_eq!(apart, 1, "b(i) is loaded once");
    // The SUB's array is any caller's: its accesses carry no array's own tag.
    let mir = emitted_mir(&parsed_as(
        &written(&directory, "param.bas", source("fill c(), n\r\n").as_bytes()),
        "qb45",
        "qb45",
    ));
    let at = mir.find("define cc 1000 void @FILL(").expect("the SUB");
    let sub = &mir[at..at + mir[at..].find("\n}\n").expect("its end")];
    let far = sub
        .lines()
        .filter(|one| one.contains("addrspace(1)") && (one.contains("store i16") || one.contains("load i16")))
        .collect::<Vec<_>>();
    // The generic `allocation` tag: every array's ancestor, so it aliases them
    // all.
    let tag = |one: &str| one.split("!tbaa ").nth(1).map(str::to_owned);
    let node = |name: &str| mir.lines().find(|line| line.starts_with(&format!("!{name} = "))).map(str::to_owned);
    let generic = |one: &&str| {
        tag(one)
            .and_then(|id| node(&id[1..]))
            .and_then(|access| access.split("!{!").nth(1).map(|rest| rest.split(',').next().unwrap_or("").to_owned()))
            .and_then(|ty| node(&ty))
            .is_some_and(|ty| ty.contains("\"allocation\","))
    };
    assert!(!far.is_empty() && far.iter().all(generic), "{sub}");
}

/// The `llrm.qb.frame` calls (the variables handed to the handler) of `name`.
fn handed_variables(
    source: &[u8],
    name: &str,
) -> usize {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(&directory, "HANDED.BAS", source);
    let program = qb_driver::parsed(&source, &qb_driver::Frontend::new("pds71", "pds71"), None).expect("parses");
    let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert!(emitted.refused.is_empty(), "{:?}", emitted.refused);
    let text = llrm_mir::print::module(&emitted.module);
    let start = text
        .find(&format!("define "))
        .and_then(
            |_| text.match_indices("define ")
                .map(|(at, _)| at)
                .find(|at| text[*at..].lines().next().is_some_and(|line| line.contains(&format!("@{name}(")))),
        );
    let start = start.expect("the function");
    let body = &text[start..];
    let body = &body[..body.find("\n}\n").expect("its end")];
    body.matches("@llrm.qb.frame(").count()
}

/// A local read only at a label that only RESUME reaches is handed to the
/// handler; one not read after any statement is not (premise: the same SUB
/// without the read of `c%` hands one fewer).
#[test]
fn test_a_local_read_at_a_resume_label_is_handed_to_the_handler() {
    let reads = handed_variables(
        b"DECLARE SUB Lab ()\nLab\nEND\nSUB Lab\nON LOCAL ERROR GOTO lh\nc% = 41 + INT(RND * 0)\nERROR 7\nEXIT SUB\ndone:\nPRINT c%\nEXIT SUB\nlh:\nRESUME done\nEND SUB\n",
        "LAB",
    );
    let unread = handed_variables(
        b"DECLARE SUB Lab ()\nLab\nEND\nSUB Lab\nON LOCAL ERROR GOTO lh\nc% = 41 + INT(RND * 0)\nERROR 7\nEXIT SUB\ndone:\nPRINT 0\nEXIT SUB\nlh:\nRESUME done\nEND SUB\n",
        "LAB",
    );
    assert_eq!(reads, unread + 1, "handed with the read {reads}, without it {unread}");
}

/// `(a$ < b$) <> -1` made an `i1` add of a compare of a flags-returning call,
/// which combine left alone and the selector refused: conformance case
/// qb45/relational-operators did not compile at -O1 and above.
#[test]
fn a_string_compare_tested_against_minus_one_compiles_optimized() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = written(
        &directory,
        "relation.bas",
        b"DIM a AS STRING, b AS STRING\r\na = \"ABC\": b = \"ABD\"\r\nIF (a < b) <> -1 OR (a = b) <> 0 THEN PRINT \"FAIL\"\r\nEND\r\n",
    );
    for level in ["-O1", "-O2"] {
        let out = directory.path().join("relation.asm");
        let argv = [source.to_str().unwrap(), level, "-S", "-o", out.to_str().unwrap()].map(str::to_owned);
        assert_eq!(crate::cli::main(&argv), 0, "{level}");
    }
}

/// Every cell of the module frame carried an `ss:` override in a program with
/// ON ERROR (range_on_error: 24 bytes at -Os): the body never loads DS, so the
/// data group is DS throughout.
#[test]
fn test_an_on_error_program_without_far_data_has_no_ss_override() {
    let basic = root().join("tests/run/qb/range_on_error.bas");
    let program = qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).expect("parses");
    let text = listing(&program);
    assert!(!text.contains("ss:"), "{text}");
}

/// The words of the table `label` names in `listing`.
fn table_words(
    listing: &str,
    label: &str,
) -> Option<usize> {
    let mut rows = listing.lines().skip_while(|one| !one.starts_with(&format!("{label} label byte"))).skip(1);
    let mut bytes = 0;
    for row in rows.by_ref() {
        match row.trim_start().split_once(' ') {
            Some(("db", list)) => bytes += list.split(',').count(),
            Some(("dw", _)) => bytes += 2,
            _ => break,
        }
    }
    Some(bytes / 2).filter(|_| listing.contains(&format!("{label} label byte")))
}

/// ERL read a table of one word per statement (sixteen, 32 bytes at -Os), all
/// zero where the program numbers no line.
#[test]
fn test_erl_without_line_numbers_has_no_table() {
    let text = listing_of(
        "erl_plain",
        "ON ERROR GOTO h\r\nFOR i% = 1 TO 3\r\nx# = EXP(30000# * i%)\r\nPRINT i%\r\nNEXT\r\nEND\r\nh:\r\nPRINT ERL\r\nRESUME NEXT\r\n",
    );
    assert!(!text.contains("$QB$ERL$"), "{text}");
}

/// ERL read a word per statement, though only the statements that can raise
/// are ever the site: ten statements here, two of them PRINTs.
#[test]
fn test_erl_table_numbers_only_the_statements_that_raise() {
    let text = listing_of(
        "erl_sites",
        "10 ON ERROR GOTO 100\r\n20 a% = 1: b% = 2: c% = a% + b%\r\n30 d% = c% * 3: e% = d% - a%\r\n40 PRINT e%\r\n50 f% = e% + 1: g% = f% * 2\r\n60 PRINT g%\r\n70 END\r\n100 PRINT ERL\r\n110 RESUME NEXT\r\n",
    );
    let words = table_words(&text, "$QB$ERL$__main").unwrap_or_else(|| panic!("no table\n{text}"));
    assert!(words <= 4, "{words} words\n{text}");
}

/// Whether a listing compares something with -1: BC's LONG divide wraps MIN \
/// -1 where the processor traps, so the front guards a divisor of -1.
fn tests_against_minus_one(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| line.starts_with("cmp") && line.ends_with(", -1"))
}

/// `PULL \ d` with a constant dividend carried the divisor-is--1 guard
/// (`cmp edi, -1 / je`, 6 B) though only MIN \ -1 traps: nbody_fixed.bas was 9
/// B larger than main at -Os m16.
#[test]
fn a_constant_dividend_that_is_not_long_min_has_no_minus_one_guard() {
    let text = listing_of(
        "const_dividend",
        "DEFINT A-Z\r\nDIM d AS LONG\r\nINPUT d\r\nPRINT 512& \\ d\r\nPRINT 512& MOD d\r\n",
    );
    assert!(!tests_against_minus_one(&text), "{text}");
}

/// The guard stays where the dividend may be LONG's minimum and the divisor -1.
#[test]
fn a_variable_dividend_keeps_the_minus_one_guard() {
    let text = listing_of(
        "variable_dividend",
        "DEFINT A-Z\r\nDIM d AS LONG, x AS LONG\r\nINPUT d\r\nINPUT x\r\nPRINT x \\ d\r\n",
    );
    assert!(tests_against_minus_one(&text), "{text}");
}

/// A constant dividend of LONG's minimum still wraps.
#[test]
fn a_dividend_of_long_min_keeps_the_minus_one_guard() {
    let text =
        listing_of("min_dividend", "DEFINT A-Z\r\nDIM d AS LONG\r\nINPUT d\r\nPRINT (-2147483647& - 1&) \\ d\r\n");
    assert!(tests_against_minus_one(&text), "{text}");
}

/// A bounds-checked fill saved SI and DI under the DAG selector although its
/// body names neither: the error exit `call B$SERR` never returns, but its
/// contract's clobbers counted as a disturbance a function must save for its
/// caller (+8 B a SUB: qb/idioms.bas +70 B at -Os with -fsanitize=bounds).
#[test]
fn test_a_call_that_never_returns_saves_no_register_under_the_dag_selector() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "fillpal.bas",
        b"DEFINT A-Z\r\nDIM SHARED pal(63)\r\nDECLARE SUB SetPal (n, w)\r\nSetPal 10, 5\r\nPRINT pal(3)\r\nSUB SetPal (n, w)\r\nFOR i = 0 TO n - 1\r\npal(i) = w\r\nNEXT\r\nEND SUB\r\n",
    );
    let program = parsed_with(&source, "qb45", "qb45", "column-major", false, true);
    let mut options = llrm_driver::m16_options(llrm_x86_m16::machine::BASIC.clone());
    options.isel = llrm_codegen::backend::shadow::Selector::Dag;
    let module = qb_compile::assembled(&program, None, &options).expect("assembles");
    let text = masm::text(&module).expect("prints");
    let fill = between(&text, "SETPAL proc", "endp");
    assert!(fill.contains("call far ptr B$SERR"), "the bounds exit: {fill}");
    assert!(!fill.contains("push si") && !fill.contains("push di"), "{fill}");
}

/// CLEAR was refused ("cannot generate code for it yet"): it is
/// B$SCLR(stack, stack given), two zero words for the bare statement.
#[test]
fn test_clear_calls_the_runtime_clear() {
    let text = listing_in("qb45", &[], "clearcall", "x% = 1\r\nCLEAR\r\nPRINT x%\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$SCLR").expect("the call");
    assert!(main[..call].trim_end().ends_with("pushd 0"), "two zero words: {main}");
}

/// A variable set before CLEAR and printed after it was folded to its old
/// value: the call zeroes BC_DATA behind the optimiser's back.
#[test]
fn test_a_variable_is_read_again_after_clear() {
    let text = listing_in("qb45", &[], "clearreads", "x% = 5\r\nCLEAR\r\nPRINT x%\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let after = &main[main.find("call far ptr B$SCLR").expect("the call")..];
    assert!(after.contains("ptr X%") || after.contains("[X%"), "x% is loaded after the call: {main}");
}

/// A string compare in one block read by a select in the next (`WHILE a$(j%) <
/// "C" AND j% < 3`) had its compare sunk away from B$SCMP's flags: the
/// program was refused at -O2 (`string_compare_shapes.bas`).
#[test]
fn test_a_string_compare_and_an_integer_compare_joined_by_and_compiles() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let text = "DIM a$(3), b$(3)\r\nIF a$ < b$ THEN\r\nk% = 2\r\nELSE\r\nk% = 3\r\nEND IF\r\n\
SELECT CASE a$\r\nEND SELECT\r\nFOR i% = 1 TO 3\r\nNEXT\r\n\
WHILE a$(j%) < \"C\" AND j% < 3: j% = j% + 1: WEND\r\nPRINT k%; j%\r\n";
    records(&parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45"), "T.BAS");
}

/// BSAVE and BLOAD called B$BSAV and B$BLOD, rows of no table: "runtime call
/// B$BSAV has no complete stack-cleanup contract" refused `vgabsave.bas`.
#[test]
fn test_bsave_and_bload_have_a_stack_cleanup_contract() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    for text in ["BSAVE \"V.BIN\", 0, 16\r\n", "BLOAD \"V.BIN\", 0\r\n"] {
        records(&parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "vbdos", "vbdos"), "T.BAS");
    }
}

/// FIELD was refused ("valid, but this compiler cannot generate code for it
/// yet"): it is B$FLDP(channel), then B$FIEL(width, variable) per pair.
#[test]
fn test_field_is_a_preamble_and_one_call_per_variable() {
    let text = listing_of(
        "fieldcalls",
        "OPEN \"R\" FOR RANDOM AS #1 LEN = 16\r\nFIELD #1, 8 AS n$, 8 AS m$\r\nLSET n$ = \"x\"\r\nCLOSE\r\nEND\r\n",
    );
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    assert_eq!(main.matches("call far ptr B$FLDP").count(), 1, "{main}");
    assert_eq!(main.matches("call far ptr B$FIEL").count(), 2, "{main}");
    assert!(main.find("B$FLDP") < main.find("B$FIEL"), "{main}");
}

/// GET and PUT without a record variable were refused ("cannot generate code
/// for it yet"): they transfer the FIELD buffer, B$GET1/B$PUT1 for the next
/// record and B$GET2/B$PUT2 for a numbered one.
#[test]
fn test_get_and_put_without_a_variable_call_the_buffer_routines() {
    let text = listing_of(
        "getputbuffer",
        "OPEN \"R\" FOR RANDOM AS #1 LEN = 16\r\nFIELD #1, 16 AS n$\r\nGET #1\r\nGET #1, 3\r\nLSET n$ = \"x\"\r\nPUT #1\r\nPUT #1, 4\r\nCLOSE\r\nEND\r\n",
    );
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    for routine in ["B$GET1", "B$GET2", "B$PUT1", "B$PUT2"] {
        assert_eq!(main.matches(&format!("call far ptr {routine}")).count(), 1, "{routine}: {main}");
    }
}

/// `END n` and `SYSTEM n` (PDS 7.1, VBDOS) were refused ("END with a return
/// code"). BC pushes the code as a word and calls `B$CENC` (measured in BC 7.1
/// and VBDOS objects), which never returns.
#[test]
fn end_and_system_with_a_code_call_b_cenc() {
    for (statement, dialect) in [("END 3", "pds71"), ("SYSTEM 3", "vbdos")] {
        let directory = tempfile::tempdir().expect("creates a directory");
        let source = format!("DEFINT A-Z\r\nINPUT a\r\nIF a THEN {statement}\r\nPRINT 1\r\n");
        let path = written(&directory, "endn.bas", source.as_bytes());
        let text = listing(&parsed_as(&path, dialect, dialect));
        assert!(text.contains("pushw 3") && text.contains("call far ptr B$CENC"), "{statement}:\n{text}");
        assert!(!text.contains("B$CEND"), "{statement}:\n{text}");
    }
}

/// The listing of `source` under one dialect and its runtime.
fn listing_in(
    dialect: &str,
    flags: &[&str],
    name: &str,
    source: &str,
) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let out = directory.path().join(format!("{name}.asm"));
    let input = written(&directory, &format!("{name}.bas"), source.as_bytes());
    let argv = [
        input.to_str().unwrap(),
        "--dialect",
        dialect,
        "--runtime",
        dialect,
        "-O2",
        "-march=i486",
        "-S",
        "-o",
        out.to_str().unwrap(),
    ]
    .into_iter()
    .chain(flags.iter().copied())
    .map(str::to_owned)
    .collect::<Vec<_>>();
    assert_eq!(crate::cli::main(&argv), 0);
    std::fs::read_to_string(&out).expect("the listing")
}

/// SSEG(a$) was refused ("cannot generate code for it yet"): it is
/// B$SSEG(descriptor), the string data's segment in AX.
#[test]
fn test_sseg_asks_the_runtime_for_a_strings_segment() {
    let text = listing_in("vbdos", &[], "ssegcall", "DIM t AS STRING\r\nt = \"ABCD\"\r\nPRINT SSEG(t)\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    assert_eq!(main.matches("call far ptr B$SSEG").count(), 1, "{main}");
}

/// CCUR(x) was refused ("cannot generate code for it yet"): it converts as an
/// assignment to a CURRENCY does, so both listings hold the same conversion.
#[test]
fn test_ccur_converts_as_an_assignment_to_currency_does() {
    let flags = ["--isel=dag"];
    let assigned = listing_in(
        "pds71",
        &flags,
        "ccurassign",
        "DIM c AS CURRENCY, d AS DOUBLE\r\nINPUT d\r\nc = d\r\nPRINT c\r\nEND\r\n",
    );
    let converted = listing_in(
        "pds71",
        &flags,
        "ccurcall",
        "DIM c AS CURRENCY, d AS DOUBLE\r\nINPUT d\r\nc = CCUR(d)\r\nPRINT c\r\nEND\r\n",
    );
    let body = |text: &str| {
        between(text, "$QB$MAIN proc", "$QB$MAIN endp").replace("CCURASSIGN", "X").replace("CCURCALL", "X")
    };
    assert_eq!(body(&assigned), body(&converted));
}

/// `CLEAR ,,n` passes the stack size and says it was given: B$SCLR(n, -1).
#[test]
fn test_clear_with_a_stack_size_passes_it_and_the_flag() {
    let text = listing_in("qb45", &[], "clearstack", "CLEAR , , 4096\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$SCLR").expect("the call");
    assert!(main[..call].trim_end().ends_with("pushd 268500991"), "stack 4096, flag -1: {main}");
}

/// CHAIN was refused ("cannot generate code for it yet"): BC pushes the
/// file name's descriptor and calls B$SCHN (measured on BC 4.50's object).
#[test]
fn test_chain_calls_the_chain_routine_with_the_file_name() {
    let text = listing_in("qb45", &[], "chaincall", "CHAIN \"NEXT\"\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$SCHN").expect("the call");
    assert!(main[..call].trim_end().rsplit('\n').next().unwrap().trim_start().starts_with("push offset"), "{main}");
}

/// DATESERIAL was refused ("cannot generate code for it yet"): VBDOS BC pushes
/// year, month and day and calls B$DTSR, which returns the near address of an
/// R8 (measured on BC's object).
#[test]
fn test_dateserial_calls_the_date_routine_with_year_month_day() {
    let text =
        listing_in("vbdos", &[], "dateser", "DIM d AS DOUBLE\r\nd = DATESERIAL(1992, 2, 29)\r\nPRINT d\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$DTSR").expect("the call");
    let before = &main[..call];
    // Year and month are one pushed dword (1992 << 16 | 2), then the day.
    assert!(before.trim_end().ends_with("pushd 130547714\n    pushw 29"), "{main}");
}

/// COMMON variables sat in BC_DATA with the rest, so a CHAINed program found
/// nothing: BC puts them, in declaration order, in the COMMON segment (10
/// bytes for an integer, a long and a string; BC 4.50's object).
#[test]
fn test_common_variables_go_in_the_common_segment_in_order() {
    let text = listing_in(
        "qb45",
        &[],
        "commonseg",
        "COMMON a AS INTEGER, b AS LONG, s AS STRING\r\nDIM z AS INTEGER\r\na = 1\r\nb = 2\r\ns = \"q\"\r\nz = 3\r\nPRINT a; b; s; z\r\nCHAIN \"NEXT\"\r\nEND\r\n",
    );
    let common = between(&text, "COMMON segment", "COMMON ends");
    let names: Vec<&str> = common.lines().filter_map(|line| line.strip_suffix(" label byte")).collect();
    assert_eq!(names, ["$QB$COMMON", "A%", "B&", "S$"], "{text}");
    let data = between(&text, "BC_DATA segment", "BC_DATA ends");
    assert!(!data.contains("A%") && !data.contains("S$"), "COMMON variables leave BC_DATA: {text}");
}

/// A COMMON variable set before CHAIN was a private cell the optimiser could
/// drop the store to: the chained program read 0.
#[test]
fn test_a_common_variable_is_still_stored_before_a_chain() {
    let text = listing_in("qb45", &[], "commonchain", "COMMON x AS INTEGER\r\nx = 4\r\nCHAIN \"NEXT\"\r\nEND\r\n");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$SCHN").expect("the call");
    assert!(main[..call].contains("mov word ptr X%, 4"), "x% is stored: {main}");
}

/// PCOPY was refused ("cannot generate code for it yet"): it is
/// B$PCPY(from page, to page), two pushed words in QB 4.5.
#[test]
fn test_pcopy_calls_the_page_copy_routine() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let out = directory.path().join("pcopypages.asm");
    let input = written(&directory, "pcopypages.bas", b"SCREEN 7\r\nPCOPY 1, 2\r\nEND\r\n");
    let argv = [
        input.to_str().unwrap(),
        "--dialect",
        "qb45",
        "--runtime",
        "qb45",
        "-O2",
        "-march=i486",
        "-S",
        "-o",
        out.to_str().unwrap(),
    ]
    .map(str::to_owned);
    assert_eq!(crate::cli::main(&argv), 0);
    let text = std::fs::read_to_string(&out).expect("the listing");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    let call = main.find("call far ptr B$PCPY").expect("the call");
    assert!(main[..call].trim_end().ends_with("pushd 65538"), "pages 1 and 2 are two pushed words: {main}");
}

/// An object linked with BRUN45.LIB crashed at start ("no stack segment", entry
/// at 0:0): BC without /O marks the header `bm` and names `$AC`, `$DAC` and
/// `b$seg`, which make LINK pull in the loader of BRUN45.EXE. Ours named none.
#[test]
fn the_run_time_module_flag_marks_the_header_and_names_the_loader_externals() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "rtm.bas", b"PRINT \"hello\"\r\nEND\r\n");
    let build = |run_time_module: bool| {
        let frontend = qb_driver::Frontend { run_time_module, ..qb_driver::Frontend::new("qb45", "qb45") };
        let program = qb_driver::parsed(&path, &frontend, None).expect("parses");
        let bytes = object_bytes(&program, "RTM.BAS").expect("emits");
        let records = omf::parse(&bytes).expect("parses");
        let externals = omf::externals(&records);
        let marker =
            bytes.windows(2).any(|pair| pair == b"bm") as u8 + bytes.windows(2).any(|pair| pair == b"bl") as u8 * 2;
        (marker, externals)
    };
    let (marker, externals) = build(true);
    assert_eq!(marker & 1, 1, "header marker bm");
    for name in ["$AC", "$DAC", "b$seg"] {
        assert!(externals.iter().any(|one| one == name), "{name} in {externals:?}");
    }
    let (_, plain) = build(false);
    assert!(!plain.iter().any(|one| one == "$AC"), "{plain:?}");
}

/// `PRINT 1` at -m32 with the QB 4.5 runtime was refused by the DAG selector
/// ("QB runtime routine whose stack cleanup the convention does not model"):
/// the table's 16-bit block pops 2 bytes, the call pushes a dword and the
/// callee pops all 4, which is what the old selector does. After: the call
/// compiles, with no `add esp` after it.
#[test]
fn test_a_32_bit_call_of_a_callee_popped_routine_selects_under_the_dag_selector() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = written(&directory, "p1.bas", b"PRINT 1\r\n");
    let out = directory.path().join("p1.asm");
    let argv = [source.to_str().unwrap(), "-m32", "--runtime", "qb45", "--isel=dag", "-S", "-o", out.to_str().unwrap()]
        .map(str::to_owned);
    assert_eq!(crate::cli::main(&argv), 0);
    let text = std::fs::read_to_string(&out).expect("the listing");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    assert!(main.contains("pushd 1\n    call B$PEI2"), "{main}");
    assert!(!main.contains("add esp"), "{main}");
}

/// A BASIC program whose last statement is a runtime call ended without
/// `call B$CENP` under the DAG selector at -m32: the call before the return
/// became a sibling `jmp`, and `ends_program` found no return to spell as the
/// exit, so `$QB$MAIN` fell out of its end into garbage (fib: "6765", then
/// DOS/32A "exception 06h" at EIP 12345678).
#[test]
fn test_a_program_ending_in_a_runtime_call_still_calls_the_exit_under_the_dag_selector() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = written(&directory, "p1.bas", b"PRINT 1\r\n");
    let out = directory.path().join("p1.asm");
    let argv = [
        source.to_str().unwrap(),
        "-m32",
        "-fqb-runtime=llrm",
        "-O2",
        "-march=i486",
        "--isel=dag",
        "-S",
        "-o",
        out.to_str().unwrap(),
    ]
    .map(str::to_owned);
    assert_eq!(crate::cli::main(&argv), 0);
    let text = std::fs::read_to_string(&out).expect("the listing");
    let main = between(&text, "$QB$MAIN proc", "$QB$MAIN endp");
    assert!(main.trim_end().ends_with("call B$CENP"), "{main}");
}

/// A loop over a dynamic array's elements, the array a parameter: the
/// descriptor words are read in the loop only if an element store might write
/// them.
const ARRAY_LOOP: &str = "DEFINT A-Z\r\nDECLARE SUB fill (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)\r\n\
DIM v(0 TO 9) AS INTEGER\r\nfill v(), 0, 9\r\n\
SUB fill (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)\r\nDIM j AS INTEGER, p AS INTEGER\r\np = a(hi)\r\n\
FOR j = lo TO hi - 1\r\nIF a(j) < p THEN a(j) = p\r\nNEXT\r\nEND SUB\r\n";

/// The loads of a descriptor's words (offsets 2 and 10 of the array pointer
/// `%0`) in the function's text.
fn descriptor_loads(function: &str) -> usize {
    function.lines().filter(|line| line.contains("getelementptr") && line.contains("ptr %0, i16 ")).count()
}

/// SORT of the quicksort bench read its array's descriptor on every trip
/// (bq16 108282 memory operands against 85305 for the same loop in Nib): the
/// element accesses and the descriptor loads carried no `!tbaa`, so the
/// element store kept the loads in the loop. They are read once, before it.
#[test]
fn test_an_array_parameters_descriptor_is_read_once_outside_its_loop() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let source = parsed_as(&written(&directory, "T.BAS", ARRAY_LOOP.as_bytes()), "qb45", "qb45");
    let function = function_text(&optimized_mir(&source), "FILL");
    assert_eq!(descriptor_loads(&function), 2, "{function}");
}

/// A callee that REDIMs the array may move it and rewrite its descriptor: a
/// loop that calls one reads the descriptor words again after the call.
#[test]
fn test_a_call_that_may_redim_the_array_keeps_the_descriptor_loads_in_the_loop() {
    let source = |call: &str| {
        format!(
            "DEFINT A-Z\r\nDECLARE SUB fill (a() AS INTEGER, n AS INTEGER)\r\nDECLARE SUB regrow (a() AS INTEGER)\r\n\
REM $DYNAMIC\r\nDIM v(0 TO 9) AS INTEGER\r\nfill v(), 8\r\n\
SUB fill (a() AS INTEGER, n AS INTEGER)\r\nDIM j AS INTEGER\r\nFOR j = 0 TO n\r\na(j) = j\r\n{call}NEXT\r\nEND SUB\r\n\
SUB regrow (a() AS INTEGER)\r\nREDIM a(0 TO 9) AS INTEGER\r\nEND SUB\r\n"
        )
    };
    let in_loop = |text: String| {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let source = parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45");
        let function = function_text(&optimized_mir(&source), "FILL");
        let looped =
            blocks(&function).into_iter().find(|block| block.contains(" = phi ")).expect("the loop").to_owned();
        (looped.contains("ptr %0, i16 2"), function)
    };
    let (loaded, function) = in_loop(source("regrow a()\r\n"));
    assert!(loaded, "{function}");
    let (loaded, function) = in_loop(source(""));
    assert!(!loaded, "{function}");
}

/// A parameter bound to a static array the callee also names was tagged
/// `allocation` against that array's own `place` accesses, so `PRINT g(1)`
/// after `a(1) = ...` printed the value read before the store (3, not 8).
#[test]
fn test_a_parameter_bound_to_a_static_array_may_alias_its_direct_accesses() {
    let text = "DEFINT A-Z\r\nDECLARE SUB s (a() AS INTEGER)\r\nDIM SHARED g(0 TO 9) AS INTEGER\r\ng(1) = 3\r\n\
s g()\r\nSUB s (a() AS INTEGER)\r\nx = g(1)\r\na(1) = x + 5\r\nPRINT g(1)\r\nEND SUB\r\n";
    let directory = tempfile::tempdir().expect("a temporary directory");
    let source = parsed_as(&written(&directory, "T.BAS", text.as_bytes()), "qb45", "qb45");
    let function = function_text(&optimized_mir_inlining(&source, llrm_transforms::inline::Threshold::none()), "S");
    let loads = function
        .lines()
        .filter(|line| line.contains("load i16, ptr getelementptr") && line.contains("@\"G%\""))
        .count();
    assert_eq!(loads, 2, "{function}");
}
