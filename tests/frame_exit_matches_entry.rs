//! The epilogue undoes what the prologue did.

mod common;

const KERNEL: &str = "
extern void arguments(const int *block, int *value, int n);
extern void fail(int);
extern void (*set_mode)(int);
extern int (*set_pages)(int, int);
void mode(const int *block)
{
    int value[4];

    arguments(block, value, 4);
    if (value[0] == -1 && value[2] == -1 && value[3] == -1)
        return;
    if (value[0] != -1 && !(value[0] == 0 && !set_mode)) {
        if (!set_mode)
            fail(5);
        set_mode(value[0]);
    }
    if (value[2] != -1 || value[3] != -1) {
        int none = (value[2] == -1 || value[2] == 0) && (value[3] == -1 || value[3] == 0);

        if (set_pages ? !set_pages(value[2], value[3]) : !none)
            fail(5);
    }
}
";

fn operands(
    lines: &[&str],
    mnemonic: &str,
) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| line.trim().strip_prefix(mnemonic).and_then(|rest| rest.strip_prefix(' ')))
        .map(str::to_owned)
        .collect()
}

/// `mode` at m32 -Os: `exits_inserted` made the exit for an omitted frame, and
/// the printer, deciding again by size on the body that held it, set the entry
/// up with `push ebp; mov ebp, esp`. The exit was `add esp, 20; pop ebx; pop
/// ecx; pop edx; pop esi; ret`: no `pop ebp`, every register restored from its
/// neighbour's slot. B$CSCN's `screen_mode` returned with EBP clobbered and
/// SCREEN 13 corrupted the caller's value (llrm_varptr m32 printed -28752).
#[test]
fn test_a_size_build_with_a_shared_return_pops_what_it_pushed() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let body: Vec<&str> = listing
        .lines()
        .skip_while(|line| !line.starts_with("mode_ proc"))
        .take_while(|line| !line.starts_with("mode_ endp"))
        .collect();
    let entry = &body[..body.iter().position(|line| line.starts_with("L0_1:")).unwrap()];
    let ret = body.iter().position(|line| line.trim() == "ret").unwrap();
    let exit = &body[body[..ret].iter().rposition(|line| line.ends_with(':')).unwrap()..ret];
    let (pushed, popped) = (operands(entry, "push"), operands(exit, "pop"));
    let reversed: Vec<String> = pushed.iter().rev().cloned().collect();
    assert_eq!(popped, reversed, "{listing}");
    assert_eq!(operands(entry, "sub"), operands(exit, "add"), "{listing}");
}
