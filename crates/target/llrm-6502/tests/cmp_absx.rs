//! A byte compared with a global byte indexed by a byte is `cmp g,x`: the load is not made into a cell first.

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

fn compile(body: &str, name: &str) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n@buf = external global [16 x i8]\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
    function_text(&module, name, &selection).expect("emits")
}

fn compare(cc: &str) -> String {
    let body = format!(
        "define i1 @f(i8 %c, i8 %i) {{\n %z = zext i8 %i to i16\n %p = getelementptr i8, ptr @buf, i16 %z\n %v = load i8, ptr %p\n %r = icmp {cc}\n ret i1 %r\n}}"
    );
    compile(&body, "f")
}

/// The argument stayed in a cell and the byte was loaded into `a` first
/// (`sta rc2; lda buf,x; cmp rc2`, 2 B and 3 clocks more) wherever a global
/// byte was compared with a byte already in `a`.
#[test]
fn load_on_the_right_of_an_equality_is_a_memory_compare() {
    for cc in ["eq i8 %c, %v", "eq i8 %v, %c", "ne i8 %v, %c"] {
        let text = compare(cc);
        assert!(text.contains("cmp buf,x"), "{cc}:\n{text}");
        assert!(!text.contains("lda buf,x"), "{cc}:\n{text}");
    }
}

/// An order flips with the operands (`TranslateX86CC`): `c < v` is `cmp c, v`.
#[test]
fn load_on_the_right_of_an_order_is_a_memory_compare() {
    let text = compare("ult i8 %c, %v");
    assert!(text.contains("cmp buf,x"), "{text}");
    assert!(text.contains("bcs"), "{text}");
}

/// `v < c` needs `a` to hold `v`: the load stays a load.
#[test]
fn load_on_the_left_of_an_order_stays_in_a() {
    let text = compare("ult i8 %v, %c");
    assert!(text.contains("lda buf,x"), "{text}");
}

/// `find`: the byte sought stays in `a` for the whole loop.
#[test]
fn a_search_loop_compares_in_memory() {
    let body = "define i8 @f(i8 %c) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %n, %loop ]
  %z = zext i8 %i to i16
  %p = getelementptr i8, ptr @buf, i16 %z
  %v = load i8, ptr %p
  %hit = icmp eq i8 %v, %c
  %n = add i8 %i, 1
  br i1 %hit, label %out, label %loop
out:
  ret i8 %i
}";
    let text = compile(body, "f");
    assert!(text.contains("cmp buf,x"), "{text}");
    assert!(!text.contains("sta rc2"), "the byte sought is saved to a cell:\n{text}");
}
