//! INTEGER `\` and MOD at m16 leave -32768 by -1 to the processor's divide
//! fault (`Target::idiv_overflow_traps`).

use std::process::Command;

const PLAIN: &str = "DEFINT A-Z
READ a, b
DATA -32768, -1
PRINT a \\ b
PRINT a MOD b
";

fn listing(
    source: &str,
    args: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["a.bas", "--dialect", "qb45", "-Os", "-S", "-o", "a.s"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `-32768 \ -1` was checked in code (`lea [b+1]`, `lea [a+32768]`, `or`, a
/// branch to `B$SERR 11`) and divided as a LONG (`cdq; idiv ebx`): +48 bytes a
/// divide over BC's `cwd; idiv word`, which faults and reaches the runtime's
/// handler as error 11 (oracle: BC 4.50, qbmeta/oracle/idiv.png).
#[test]
fn test_an_integer_divide_at_m16_is_the_bare_16_bit_idiv_without_the_overflow_check() {
    let text = listing(PLAIN, &["--runtime", "qb45"]);
    assert!(text.contains("cwd"), "{text}");
    assert!(!text.contains("32768"), "{text}");
    assert!(!text.contains("cdq") && !text.contains("idiv e"), "{text}");
}

/// ON ERROR with RESUME NEXT after the fault resumed at the first statement row
/// after the faulting address, which our block layout does not keep in order:
/// differential overflow-ops ran the divide again to the 120 s limit (a million
/// `err 11` lines). A program that handles errors keeps the check, whose error
/// names its statement.
#[test]
fn test_a_program_that_handles_errors_keeps_the_overflow_check() {
    let source = format!("ON ERROR GOTO h\n{PLAIN}END\nh:\nRESUME NEXT\n");
    for runtime in ["qb45", "llrm"] {
        let text = listing(&source, &["--runtime", runtime]);
        assert!(text.contains("32768"), "{runtime}: {text}");
    }
}

/// The DOS extender's runtime takes exception 0 (runtime/qb/dos/m32/trap.asm,
/// DPMI 0203h), so m32 is the same 16-bit divide.
#[test]
fn test_a_dos_32_bit_target_leaves_the_overflow_to_the_fault_too() {
    let text = listing(PLAIN, &["-fqb-runtime=llrm", "-m32", "-march=i486"]);
    assert!(text.contains("cwd") && !text.contains("32768"), "{text}");
}

/// A host's object has no handler of the runtime's for the fault (no SIGFPE
/// handler in runtime/qb/posix): the check stays.
#[test]
fn test_a_hosted_object_keeps_the_overflow_check() {
    let text = listing(PLAIN, &["-fqb-runtime=llrm", "-m32", "-march=i486", "-fobject-format=elf"]);
    assert!(text.contains("32768"), "{text}");
}
