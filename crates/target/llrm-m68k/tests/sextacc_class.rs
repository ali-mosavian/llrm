//! `sextacc` gave the result of a data-only `sub.l` an address register (the
//! high word of a 64-bit subtract with the borrow added as a sign-extended
//! byte in `__udivmod_w`, the 64-bit divide's worker). The text read `sub.l
//! %d4, %a0`, which gas turns into `suba.l`, but the encoder wrote `sub.l %d4,
//! %d0`: at 68000 `~0 / 2` had a high word of 0xd5555554 for 0x7fffffff, and
//! every 64-bit division through it was wrong.

use llrm_m68k::classcheck::misplaced;
use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

/// `__udivmod_w` as the mid-end leaves it (`llrm-m68k-llc -O2 --dump`, last
/// stage).
const UDIVMOD: &str = include_str!("fixtures/udivmod_w.ll");

const PAIRS: [(u64, u64); 6] = [
    (u64::MAX, 2),
    (100_000_000_000, 1000),
    (0x0123_4567_89AB_CDEF, 0x1_0000_0000),
    (7, 123_456_789_012),
    (u64::MAX, 0xFF_FFFF_FFFF),
    (1, 1),
];

/// `f` calls the worker for each pair and folds its four output words into one
/// byte.
fn module() -> llrm_mir::module::Module {
    let mut ir = UDIVMOD.to_owned();
    ir.push_str("\ndefine i32 @f() {\n  %o = alloca [16 x i8], align 4\n");
    let mut sum = "0".to_owned();
    for (at, (n, d)) in PAIRS.iter().enumerate() {
        let words = [*n as u32, (n >> 32) as u32, *d as u32, (d >> 32) as u32];
        ir.push_str(&format!(
            "  call void @__udivmod_w(i32 {}, i32 {}, i32 {}, i32 {}, ptr %o)\n",
            words[0] as i32, words[1] as i32, words[2] as i32, words[3] as i32
        ));
        for k in 0..4 {
            ir.push_str(&format!(
                "  %p{at}_{k} = getelementptr i8, ptr %o, i32 {}\n  %w{at}_{k} = load i32, ptr %p{at}_{k}\n",
                4 * k
            ));
            // sum = sum * 31 + w, without the multiply libcall
            ir.push_str(&format!(
                "  %s{at}_{k} = shl i32 {sum}, 5\n  %t{at}_{k} = sub i32 %s{at}_{k}, {sum}\n  %h{at}_{k} = add i32 %t{at}_{k}, %w{at}_{k}\n"
            ));
            sum = format!("%h{at}_{k}");
        }
    }
    ir.push_str(&format!(
        "  %a = lshr i32 {sum}, 8\n  %b = lshr i32 {sum}, 16\n  %c = lshr i32 {sum}, 24\n  %x = xor i32 {sum}, %a\n  %y = xor i32 %x, %b\n  %z = xor i32 %y, %c\n  %r = and i32 %z, 255\n  ret i32 %r\n}}\n"
    ));
    llrm_mir::parse::module(&ir).expect("parses")
}

/// The byte `f` returns, from the quotient and remainder of each pair.
fn host() -> i32 {
    let mut sum = 0u32;
    for (n, d) in PAIRS {
        let (q, r) = if d == 0 { (u64::MAX, n) } else { (n / d, n % d) };
        for word in [q as u32, (q >> 32) as u32, r as u32, (r >> 32) as u32] {
            sum = sum.wrapping_mul(31).wrapping_add(word);
        }
    }
    ((sum ^ (sum >> 8) ^ (sum >> 16) ^ (sum >> 24)) & 255) as i32
}

#[test]
fn no_operand_is_in_the_other_bank_than_its_class() {
    let module = module();
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for size in [false, true] {
            let items = items_with(&module, "__udivmod_w", cpu, Options { size, mid_end: false, ..Options::default() })
                .expect("compiles");
            assert_eq!(misplaced(&items), Vec::<String>::new(), "{cpu:?}, size {size}");
        }
    }
}

#[test]
fn the_worker_divides_as_the_host_does_at_68000() {
    let Some(qemu) = support::qemu() else { return };
    let module = module();
    let mut all = start("f", &[]);
    for name in ["f", "__udivmod_w"] {
        all.extend(
            items_with(&module, name, Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles"),
        );
    }
    let path = support::unique_path("sextacc_class");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    // 68000 code reads `move %sr`, which traps on the default CPU
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(host()));
}

/// Every function of the bench kernels and library, at both ends of the family
/// and both goals: the same check over code no one wrote for it.
#[test]
fn no_bench_function_has_an_operand_in_the_other_bank() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench");
    for set in ["kernels", "library"] {
        let mut files: Vec<_> = std::fs::read_dir(root.join(set))
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
            .collect();
        files.sort();
        for path in files {
            let module = llrm_mir::parse::module(&std::fs::read_to_string(&path).unwrap()).expect("parses");
            let names: Vec<String> = module
                .functions()
                .filter(|(.., function)| !function.is_declaration())
                .filter_map(|(_, global, _)| global.name.owned())
                .collect();
            for name in names {
                for cpu in [Cpu::M68000, Cpu::M68020] {
                    for size in [false, true] {
                        let items =
                            items_with(&module, &name, cpu, Options { size, ..Options::default() }).expect("compiles");
                        assert_eq!(
                            misplaced(&items),
                            Vec::<String>::new(),
                            "{set}/{} @{name}, {cpu:?}, size {size}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}
