//! A static is placed on its natural alignment: a float after a short sat at
//! 2 mod 4 and every access to it cost a 486 3 clocks (Intel486 data sheet
//! 240440-002, Table 2.1). d_faces -O2: 148 misaligned operands, all of them
//! `fp_area`, `fp_tex` and `fp_ofs` after `static short fp_cnt`.

use std::process::Command;

/// The addresses of `names` in the linked image of `source`.
fn addresses(
    source: &str,
    names: &[&str],
) -> Vec<u64> {
    let scratch = tempfile::tempdir().unwrap();
    let at = |name: &str| scratch.path().join(name);
    std::fs::write(at("a.c"), source).unwrap();
    let run = |command: &mut Command| {
        let output = command.current_dir(scratch.path()).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    run(Command::new(env!("CARGO_BIN_EXE_llrm-c")).args([
        "-m32",
        "-mabi=sysv",
        "-O2",
        "-march=i486",
        "-fobject-format=elf",
        "-o",
        "a.o",
        "a.c",
    ]));
    run(Command::new("ld").args([
        "-m",
        "elf_i386",
        "-e",
        "0",
        "-Ttext=0x10000",
        "-Tdata=0x20003",
        "-o",
        "a.elf",
        "a.o",
    ]));
    let table = run(Command::new("nm").arg("a.elf"));
    names
        .iter()
        .map(|name| {
            let line =
                table.lines().find(|line| line.ends_with(&format!(" _{name}")) || line.ends_with(&format!(" {name}")));
            u64::from_str_radix(line.unwrap_or_else(|| panic!("{name} in {table}")).split(' ').next().unwrap(), 16)
                .unwrap()
        })
        .collect()
}

#[test]
fn a_float_after_a_short_is_on_a_dword() {
    let found = addresses(
        "short n = 1; float area = 2; int tex = 3; char c = 4; double d = 5;\n\
         int f(void) { return n + area + tex + c + d; }\n",
        &["n", "area", "tex", "d"],
    );
    assert_eq!(found[1] % 4, 0, "{found:x?}");
    assert_eq!(found[2] % 4, 0, "{found:x?}");
    assert_eq!(found[3] % 8, 0, "{found:x?}");
}
