//! A constant that several edges of a phi carry is made once.

mod common;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// D_DRAW_FACES -Os lines 562-567: shorts zeroed at the top of a loop and
/// assigned under nested conditions were zeroed again on every edge of the join
/// (four times, and stored four times where they lived in a slot); gcc writes
/// the variable at the top and in the arm that assigns it.
#[test]
fn a_variable_zeroed_at_the_top_and_assigned_in_an_arm_is_zeroed_once() {
    let source = "extern void other(int);
extern int c1(void);
extern void use3(short, short, short);
void f(int n, int a, short *g)
{
    short x, y, z;
    int i;
    for (i = 0; i < n; i++) {
        x = 0; y = 0; z = 0;
        other(i);
        if (a) {
            if (c1() > 1) {
                if (g[0] >= 0) { x = g[1]; y = g[2]; }
            }
        }
        other(i + 1);
        use3(x, y, z);
    }
}
";
    for level in ["-Os", "-O2"] {
        let text = listing(source, level);
        let zeroings =
            text.lines().filter(|line| line.trim_start().starts_with("xor ") && line.contains(" di, di")).count()
                + text.lines().filter(|line| line.trim_start().starts_with("xor ") && line.contains(" bx, bx")).count();
        assert!(zeroings <= 2, "{level}: {zeroings} zeroings of the two variables\n{text}");
    }
}
