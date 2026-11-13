//! x_transpose -O1 m32 ran 139,335 clocks against the pin clang's 125,356: its
//! last loop kept `n` live and tested `lea esi,[edi+edx-1]; cmp esi,-1` (6
//! instructions) where the pin counts `n` down to zero (`dec esi; jne`, 4), and
//! `n` held in a register around the inner copy loop cost it two copies there.
//! LSR met the users of a value in program order, the pin in use-list order.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int M[32][32], T[32][32];
long f(int n)
{
    int i, j, r; long sum = 0;
    for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = i * 37 + j;
    for (r = 0; r < 8; ++r) { for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) T[j][i] = M[i][j] + r; for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = T[i][j]; }
    for (i = 0; i < n; ++i) sum += M[i][n - 1 - i];
    return sum;
}
";

#[test]
fn a_loop_over_an_index_and_its_complement_counts_n_down() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsrdown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args(["-O1", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let step = text.lines().position(|line| line.contains(", 124")).expect(&text);
    let lines: Vec<&str> = text.lines().collect();
    let head = lines[..step].iter().rposition(|line| line.trim_end().ends_with(':')).unwrap();
    let tail = step + lines[step..].iter().position(|line| line.trim_start().starts_with("jne")).unwrap();
    let body = &lines[head + 1..=tail];
    assert!(body.iter().all(|line| !line.contains("lea")), "an address computed per trip:\n{}", body.join("\n"));
    assert_eq!(body.len(), 4, "the loop is a load-add, a step, a decrement and a branch:\n{}", body.join("\n"));
}
