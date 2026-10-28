//! The frame's exit sequence as instructions of the body
//! (`masm::exits_inserted`).

/// The listing of `source` at `level` for 32-bit x86.
fn listing(
    source: &str,
    level: &str,
) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let (input, output) = (dir.path().join("in.c"), dir.path().join("out.s"));
    std::fs::write(&input, source).expect("the source");
    let argv: Vec<String> = ["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o"]
        .iter()
        .map(|one| one.to_string())
        .chain([output.display().to_string(), input.display().to_string()])
        .collect();
    assert_eq!(llrm_c::compile::main(&argv), 0);
    std::fs::read_to_string(output).expect("the listing")
}

/// The exit's `pop ebp` named the frame register in the body, so the saved set
/// took EBP as a callee-saved register too: a second `push ebp` in the entry
/// of the caller (+1 byte in bintree, nbody and quicksort, m32 -Os).
#[test]
fn a_frame_pointer_is_pushed_once() {
    let text = listing(
        "static void sort(int *a, int lo, int hi) { int pivot, i, j, t;\n\
         if (lo >= hi) return; pivot = a[hi]; i = lo;\n\
         for (j = lo; j < hi; ++j) if (a[j] < pivot) { t = a[i]; a[i] = a[j]; a[j] = t; ++i; }\n\
         t = a[i]; a[i] = a[hi]; a[hi] = t; sort(a, lo, i - 1); sort(a, i + 1, hi); }\n\
         long f(unsigned short seed) { int values[1024]; int i; unsigned short x = seed; long sum = 0;\n\
         for (i = 0; i < 1024; ++i) { x = x * 25173u + 13849u; values[i] = x & 0x7FFF; }\n\
         sort(values, 0, 1023);\n\
         for (i = 1; i < 1024; ++i) if (values[i - 1] > values[i]) return -1;\n\
         for (i = 0; i < 1024; ++i) sum += (long)values[i] * ((i & 15) + 1);\n\
         return sum; }\n",
        "-Os",
    );
    // The pin (`clang -target i386-unknown-linux-gnu -march=i486 -Os`) pushes
    // `ebp` once, as a saved register of `sort`; `f` keeps no frame register.
    assert_eq!(text.matches("push ebp").count(), 1, "sort's saved ebp only: {text}");
}

/// The pops of the epilogue were no instructions when the tails were merged,
/// so three returns that share them kept three copies (`pop`, `pop`, `ret`
/// each). The pin's `BranchFolder` runs after the epilogue and shares them.
#[test]
fn returns_that_share_an_epilogue_share_its_tail() {
    let text = listing(
        "int g(int);\n\
         int f(int a, int b, int c) { int x = g(a);\n\
         if (x == 1) return g(b) + x;\n\
         if (x == 2) return g(c) + x * 3;\n\
         return g(a + b) + x * c; }\n",
        "-O2",
    );
    assert_eq!(text.matches("ret\n").count(), 1, "{text}");
}
