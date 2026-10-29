//! ON LOCAL ERROR at -m32.

use std::process::Command;

const SOURCE: &str = "' dialect: pds71
DECLARE SUB top ()
top
END
SUB top
ON LOCAL ERROR GOTO th
PRINT \"x\"
EXIT SUB
th:
RESUME NEXT
END SUB
";

/// The handler's offset was `ptrtoint` of the landing to the runtime's 16-bit
/// word, though a pointer is 32 bits at -m32: no pattern selected the
/// narrowing, so every program with ON LOCAL ERROR failed with "@TOP: ptrtoint"
/// (#2672), and one that selected it would have passed a flat address cut to 16
/// bits. The offset is the pointer's width: m32 pushes the address, m16 a word.
#[test]
fn test_on_local_error_registers_the_handlers_offset_at_the_pointers_width() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.bas"), SOURCE).unwrap();
    let listing = |mode: &str, level: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .current_dir(directory)
            .args([mode, level, "-S", "-o", "a.s", "a.bas"])
            .output()
            .unwrap();
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{mode} {level}: {errors}");
        std::fs::read_to_string(directory.join("a.s")).unwrap()
    };
    for level in ["-O0", "-O2"] {
        let flat = listing("-m32", level);
        // -O0 stores the offset into the call's argument area, -O2 pushes it.
        assert!(
            (flat.contains("push offset $QB$LANDING") || flat.contains("[esp], offset $QB$LANDING"))
                && !flat.contains("movzx"),
            "{level}: {flat}"
        );
        // The pad's `cld` is laid down at the pseudo-call, which names no
        // routine to link.
        assert!(!flat.contains("extern __LANDING"), "{level}: {flat}");
        let real = listing("-m16", level);
        // `mov ax, offset X; push ax` is `push offset X` (#5135), at -O0 too
        // now that the DAG route selects it.
        let pushed = real.contains("push offset $QB$LANDING");
        assert!(pushed || real.contains("mov ax, offset $QB$LANDING"), "{level}: {real}");
    }
}
