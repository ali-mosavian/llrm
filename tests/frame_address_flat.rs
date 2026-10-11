//! A local's address in flat code is a frame address, never a 16-bit register.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .env("LLRM_VERIFY", "1")
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `lea eax, [bx-52]` took the address of `l` for the first call: the frame
/// base in the address was renamed to `bx` after `mov ebp, ebx`, as if it read
/// register `bp`. The callee got `y - 52`.
#[test]
fn the_address_of_a_local_after_a_write_to_ebp_stays_a_frame_address() {
    let source = "typedef struct L { void *dst; unsigned count; int d, m4, s4, steps[2]; unsigned phase, xm, bit, px; \
                  const unsigned char *tab; unsigned color; } L;\n\
                  extern void terms(L *l, unsigned dx, unsigned dy);\n\
                  extern void kernel(L *l);\n\
                  extern unsigned char *at(unsigned x, unsigned y);\n\
                  static unsigned pitch = 320;\n\
                  void line(const void *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)\n\
                  { L l; (void)fill; terms(&l, dx, dy); l.dst = at(x, y);\n\
                    l.steps[0] = l.steps[1] = step_y > 0 ? (int)pitch : -(int)pitch;\n\
                    l.phase = 0; kernel(&l); }\n";
    for level in ["-Os", "-O2"] {
        let text = listing(source, &["-m32", level]);
        let leas: Vec<&str> = text.lines().filter(|line| line.trim_start().starts_with("lea ")).collect();
        assert_eq!(leas.len(), 2, "{level}:\n{text}");
        for lea in leas {
            assert!(lea.contains("[esp+"), "{level}: {lea}\n{text}");
        }
    }
}
