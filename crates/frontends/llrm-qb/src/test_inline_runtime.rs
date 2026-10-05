//! #456: ASC(MID$(s, i, 1)) made a string temporary and two far calls per byte. A routine the runtime
//! description states is defined in the module, for the inliner to price against its call.

use super::driver as qb_driver;
use super::test_hir::{emitted_mir, optimized_mir, written};

/// Five loops over the bytes of a string, as a program reads one, and two reads of runtime temporaries.
const SOURCE: &str = "DEFINT A-Z
DECLARE FUNCTION A& (s AS STRING)
DECLARE FUNCTION B& (s AS STRING)
DECLARE FUNCTION C& (s AS STRING)
DECLARE FUNCTION D& (s AS STRING)
DECLARE FUNCTION E& (s AS STRING)
DECLARE FUNCTION T% (s AS STRING)
DIM p AS STRING
p = \"Oliver\"
PRINT A&(p) + B&(p) + C&(p) + D&(p) + E&(p) + T%(p)

FUNCTION A& (s AS STRING)
    FOR i = 1 TO LEN(s): A& = A& + ASC(MID$(s, i, 1)): NEXT
END FUNCTION
FUNCTION B& (s AS STRING)
    FOR i = 1 TO LEN(s): B& = B& + ASC(MID$(s, i, 1)) * 2: NEXT
END FUNCTION
FUNCTION C& (s AS STRING)
    FOR i = 1 TO LEN(s): C& = C& + ASC(MID$(s, i, 1)) * 3: NEXT
END FUNCTION
FUNCTION D& (s AS STRING)
    FOR i = 1 TO LEN(s): D& = D& + ASC(MID$(s, i, 1)) * 4: NEXT
END FUNCTION
FUNCTION E& (s AS STRING)
    FOR i = 1 TO LEN(s): E& = E& + ASC(MID$(s, i, 1)) * 5: NEXT
END FUNCTION
FUNCTION T% (s AS STRING)
    T% = ASC(LEFT$(s, 2)) + ASC(s + \"x\")
END FUNCTION
";

fn mir(dialect: &str, checked: bool, optimized: bool) -> String {
    mir_of(SOURCE, dialect, checked, optimized)
}

fn mir_of(source: &str, dialect: &str, checked: bool, optimized: bool) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "bytes.bas", source.as_bytes());
    let frontend = qb_driver::Frontend { checked_arrays: checked, ..qb_driver::Frontend::new(dialect, dialect) };
    let program = qb_driver::parsed(&path, &frontend, None).expect("compiles");
    if optimized { optimized_mir(&program) } else { emitted_mir(&program) }
}

/// The text of `function`'s definition.
fn body<'t>(mir: &'t str, function: &str) -> &'t str {
    let start = mir.lines().scan(0, |at, line| { let here = *at; *at += line.len() + 1; Some((here, line)) }).find(|(_, line)| line.starts_with("define") && line.contains(&format!("@\"{function}\"("))).unwrap_or_else(|| panic!("no @{function}\n{mir}")).0;
    let rest = &mir[start..];
    &rest[..rest.find("\n}\n").expect("ends")]
}

fn calls(text: &str, routine: &str) -> usize {
    text.lines().filter(|line| line.contains("call ") && line.contains(&format!("@llrm.qb.{routine}("))).count()
}

/// Each loop kept a MID$ temporary and a far call per byte: A&'s loop was `B$FMID` then `B$FASC`
/// (210 instructions a byte against C's 26 in bench/grep). Where the runtime says how it reads a
/// string, five sites of each callee leave no call and no descriptor in the loops; the two temporaries
/// the runtime frees keep their call.
#[test]
fn an_asc_of_mid_loop_has_no_temporary_or_far_call() {
    for dialect in ["qb45", "pds71"] {
        let mir = mir(dialect, false, true);
        for function in ["A&", "B&", "C&", "D&", "E&"] {
            let text = body(&mir, function);
            for routine in ["B$FMID", "B$FASC", "B$FLEN"] {
                assert_eq!(calls(text, routine), 0, "{dialect} {function}: {routine} stays\n{text}");
            }
            assert!(!text.contains("alloca") && text.contains("load i8"), "{dialect} {function}: a descriptor or no byte load\n{text}");
        }
        let text = body(&mir, "T%");
        assert_eq!(calls(text, "B$FASC"), 2, "{dialect}: a runtime temporary is freed by the runtime\n{text}");
    }
}

/// A runtime whose strings are far, the length in the payload rather than at an offset in the
/// descriptor, states no layout, and keeps every call.
#[test]
fn a_runtime_that_states_no_descriptor_keeps_its_calls() {
    let mir = mir("vbdos", false, true);
    let text = body(&mir, "A&");
    assert_eq!((calls(text, "B$FMID"), calls(text, "B$FASC")), (1, 1), "{text}");
    assert!(!mir.contains("available_externally"), "no body of the runtime is defined\n{mir}");
}

/// At the pass's input the routines are defined, held to inline from; none reaches the output.
#[test]
fn the_routines_are_defined_at_the_input_and_gone_from_the_output() {
    let input = mir("qb45", false, false);
    for routine in ["B$FMID", "B$FASC", "B$FLEN"] {
        assert!(input.lines().any(|line| line.starts_with("define available_externally") && line.contains(&format!("@llrm.qb.{routine}("))), "{routine}\n{input}");
    }
    let output = mir("qb45", false, true);
    assert!(!output.lines().any(|line| line.starts_with("define available_externally")), "{output}");
}

/// The checks are `-fsanitize=bounds`'s: without it a copy of ASC is a load and nothing raises; with it
/// the copy tests for the empty string and raises error 5 in a cold block, as the runtime's does.
#[test]
fn the_checks_are_emitted_only_where_the_program_asks_for_them() {
    let asc = |mir: &str| -> String { body_of(mir, "B$FASC").to_owned() };
    let plain = asc(&mir("qb45", false, false));
    assert!(!plain.contains("B$SERR"), "{plain}");
    let checked = asc(&mir("qb45", true, false));
    assert!(checked.contains("@llrm.qb.B$SERR(i16 5) cold"), "{checked}");
    let mid = body_of(&mir("pds71", true, false), "B$FMID").to_owned();
    assert!(mid.contains("@llrm.qb.B$SERR(i16 5) cold"), "{mid}");
}

fn body_of<'t>(mir: &'t str, routine: &str) -> &'t str {
    let start = mir.lines().scan(0, |at, line| { let here = *at; *at += line.len() + 1; Some((here, line)) }).find(|(_, line)| line.starts_with("define available_externally") && line.contains(&format!("@llrm.qb.{routine}("))).unwrap_or_else(|| panic!("no body of {routine}\n{mir}")).0;
    let rest = &mir[start..];
    &rest[..rest.find("\n}\n").expect("ends")]
}

/// A character made of a byte, as a loop makes one.
const CHARACTERS: &str = "DEFINT A-Z
DECLARE FUNCTION S& (n AS INTEGER)
PRINT S&(5)

FUNCTION S& (n AS INTEGER)
    FOR i = 1 TO n: S& = S& + ASC(CHR$(i + 64)) + LEN(CHR$(i)): NEXT
END FUNCTION
";

/// CHR$ allocated a runtime string temporary for every byte: each trip was `B$FCHR`, `B$FASC` and
/// `B$FLEN`. A one-byte view over a frame byte leaves the loop with neither call nor descriptor, and
/// raises error 5 for a value past a byte only where checks are asked for.
#[test]
fn a_chr_of_a_byte_has_no_temporary_or_call() {
    for dialect in ["qb45", "pds71"] {
        let text = mir_of(CHARACTERS, dialect, false, true);
        let text = body(&text, "S&");
        for routine in ["B$FCHR", "B$FASC", "B$FLEN", "B$SERR"] {
            assert_eq!(calls(text, routine), 0, "{dialect}: {routine} stays\n{text}");
        }
        assert!(!text.contains("alloca"), "{dialect}: a descriptor stays\n{text}");
    }
    let text = mir_of(CHARACTERS, "vbdos", false, true);
    assert_eq!(calls(body(&text, "S&"), "B$FCHR"), 2, "{text}");
    let plain = body_of(&mir_of(CHARACTERS, "qb45", false, false), "B$FCHR").to_owned();
    let checked = body_of(&mir_of(CHARACTERS, "qb45", true, false), "B$FCHR").to_owned();
    assert!(!plain.contains("B$SERR") && checked.contains("@llrm.qb.B$SERR(i16 5) cold"), "{plain}\n{checked}");
}
