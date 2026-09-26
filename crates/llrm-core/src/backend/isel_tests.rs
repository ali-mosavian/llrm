use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::Register;

use crate::backend::cpu::ProfileOrName;
use crate::backend::isel::{self, Convention, Home, Unselected};
use crate::backend::{addressvalues, frame, masm};
use crate::flow;

const LAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16\"\n";

/// cdecl, far: the first parameter at [bp+6], the result in AX.
fn cdecl(parameters: usize) -> Convention {
    Convention { parameters: (0..parameters).map(|at| Home::Frame(6 + 2 * at as i64)).collect(), returns: vec![Register::EAX] }
}

/// QB's contracts: the runtime's measured ones, and conservative ones for
/// what it does not know.
fn contracts(name: &str, pops: bool, pushed: i64) -> Result<crate::abi::runtime::Contract, String> {
    let cleanup = if pops { crate::hir::model::StackCleanup::Callee } else { crate::hir::model::StackCleanup::Caller };
    let name = name.strip_prefix("llrm.qb.").unwrap_or(name);
    crate::abi::qb::_contract(name, cleanup, pushed, crate::hir::model::RuntimeProfile::Qb45).map_err(|error| error.0)
}

fn selected(text: &str, name: &str, convention: &Convention) -> Result<isel::Selected, Unselected> {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses");
    isel::selected(&module, name, convention, &contracts)
}

/// The procedure's instructions, through every machine phase.
fn listing(text: &str, name: &str, convention: &Convention) -> Vec<String> {
    let parsed = llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses");
    let names = crate::backend::globals::names(&parsed).expect("names");
    let isel::Selected { body, calls, far } = selected(text, name, convention).expect("selects");
    let mut body = flow::verified(body, "isel", true).expect("verified");
    let frame = Rc::new(RefCell::new(frame::of(&body, Some(&calls), "", None).expect("a frame")));
    let pinned = body.pins.clone();
    let mut in_ssa = true;
    for mut phase in flow::machine(&pinned, Some(Rc::clone(&frame)), Some(&calls), false, ProfileOrName::Name("486")).expect("phases") {
        if phase.class_name() == "Prologue" {
            continue;
        }
        if phase.class_name() == "PhiElimination" {
            in_ssa = false;
        }
        body = flow::checked(body, phase.as_mut(), in_ssa).expect("a phase");
        if std::env::var_os("ISEL_DUMP").is_some() {
            println!("{}", crate::tools::stages::lir_stage(phase.class_name(), &[(name.to_owned(), body.clone())]));
        }
    }
    let body = masm::cleaned_returns(&addressvalues::converted(&body), 0).expect("returns");
    let callees = calls.iter().map(|(at, callee)| (*at, masm::Callee::new(callee.clone(), far.contains(at)))).collect();
    let reserve = {
        let frame = frame.borrow();
        -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)
    };
    let procedure =
        masm::Procedure { name: name.to_owned(), public: true, far: true, body, reserve, callees, interrupt: None };
    let module = masm::Module {
        code: "T_TEXT".to_owned(),
        names,
        externs: Vec::new(),
        publics: Vec::new(),
        data: Vec::new(),
        procedures: vec![procedure],
        private: BTreeSet::new(),
        requests: BTreeSet::new(),
    };
    let text = masm::text(&module).expect("prints");
    let from = text.find(&format!("{name} proc far")).expect("the procedure");
    text[from..].lines().skip(1).take_while(|line| !line.ends_with("endp")).map(|line| line.trim().to_owned()).collect()
}

/// A return reads only its result and the epilogue's registers: read as
/// reading every register, it kept bx live and its load unfolded.
#[test]
fn test_arithmetic_on_stack_parameters() {
    let got = listing("define i16 @f(i16 %a, i16 %b) {\n  %s = sub i16 %a, %b\n  %t = add i16 %s, 3\n  ret i16 %t\n}\n", "f", &cdecl(2));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "sub ax, word ptr [bp+8]",
            "add ax, 3",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_a_counted_loop() {
    let text = "define i16 @sum(i16 %n) {
entry:
  br label %test
test:
  %s = phi i16 [ 0, %entry ], [ %s1, %body ]
  %i = phi i16 [ 0, %entry ], [ %i1, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %s1 = add i16 %s, %i
  %i1 = add i16 %i, 1
  br label %test
done:
  ret i16 %s
}
";
    let got = listing(text, "sum", &cdecl(1));
    // The comparison stays flags beside its branch; the phis' zeros are made in the entry.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "xor ax, ax",
            "xor cx, cx",
            "jmp L0_1",
            "L0_5:",
            "add ax, cx",
            "inc cx",
            "L0_1:",
            "cmp cx, bx",
            "jl L0_5",
            "L0_8:",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_locals_live_in_frame_slots() {
    let text = "define i16 @f(i16 %a) {
  %x = alloca [2 x i16]
  %y = getelementptr inbounds [2 x i16], ptr %x, i16 0, i16 1
  store i16 %a, ptr %y
  store volatile i16 7, ptr %x
  %v = load i16, ptr %y
  ret i16 %v
}
";
    let got = listing(text, "f", &cdecl(1));
    // The load reads what the store left in ax; the volatile store stays.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 4",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "mov word ptr [bp-2], ax",
            "mov word ptr [bp-4], 7",
            "leave",
            "retf",
        ]
    );
}

#[test]
fn test_a_pointer_parameter_is_a_base_register() {
    let text = "define i16 @f(ptr %p) {
  %q = getelementptr inbounds i8, ptr %p, i16 4
  %v = load i16, ptr %q
  %w = trunc i16 %v to i8
  %x = sext i8 %w to i16
  ret i16 %x
}
";
    let got = listing(text, "f", &cdecl(1));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "mov ax, word ptr [bx+4]",
            "movsx ax, al",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_a_constant_compared_first_is_swapped() {
    let text = "define i16 @f(i16 %a) {
entry:
  %c = icmp sgt i16 5, %a
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 2
}
";
    let got = listing(text, "f", &cdecl(1));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "cmp word ptr [bp+6], 5",
            "jl L0_2",
            "L0_3:",
            "mov ax, 2",
            "pop bp",
            "retf",
            "L0_2:",
            "mov ax, 1",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_what_is_not_selected_yet_is_refused() {
    for (body, why) in [
        ("%c = icmp eq i16 %a, 0\n  %d = add i1 %c, %c\n  %v = sext i1 %d to i16\n  ret i16 %v", "arithmetic on an i1"),
        ("%b = trunc i16 %a to i8\n  %v = sdiv i8 %b, 3\n  %w = sext i8 %v to i16\n  ret i16 %w", "a byte division"),
    ] {
        let text = format!("define i16 @f(ptr %p, i16 %a) {{\n  {body}\n}}\n");
        assert_eq!(selected(&text, "f", &cdecl(2)).err(), Some(Unselected(why.to_owned())), "{body}");
    }
}

/// A shift counts from cl: the count as a word printed `shl ax, cx`.
#[test]
fn test_division_and_variable_shifts() {
    let text = "define i16 @f(i16 %a, i16 %b) {
  %q = sdiv i16 %a, %b
  %r = urem i16 %a, %b
  %s = add i16 %q, %r
  %t = shl i16 %s, %b
  ret i16 %t
}
";
    let got = listing(text, "f", &cdecl(2));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "push si",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "mov cx, word ptr [bp+8]",
            "mov ax, bx",
            "cwd",
            "idiv cx",
            "mov si, ax",
            "mov ax, bx",
            "xor dx, dx",
            "div cx",
            "mov ax, si",
            "add ax, dx",
            "shl ax, cl",
            "pop si",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_variable_indices_are_scaled_and_added() {
    let text = "define i16 @f(ptr %p, i16 %i) {
  %x = alloca [4 x i16]
  %y = getelementptr inbounds [4 x i16], ptr %x, i16 0, i16 %i
  store i16 5, ptr %y
  %q = getelementptr inbounds { i16, [3 x i16] }, ptr %p, i16 0, i32 1, i16 %i
  %v = load i16, ptr %q
  ret i16 %v
}
";
    let got = listing(text, "f", &cdecl(2));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 8",
            "push si",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "mov ax, word ptr [bp+8]",
            "lea cx, [eax+eax]",
            "lea si, [bp-8]",
            "add si, cx",
            "mov word ptr [si], 5",
            "shl ax, 1",
            "add bx, 2",
            "add bx, ax",
            "mov ax, word ptr [bx]",
            "pop si",
            "leave",
            "retf",
        ]
    );
}

#[test]
fn test_a_switch_is_a_chain_of_compares() {
    let text = "define i16 @f(i16 %a) {
entry:
  switch i16 %a, label %other [ i16 1, label %one
                                i16 5, label %five
                                i16 7, label %other
                                i16 9, label %one ]
one:
  %x = phi i16 [ 10, %entry ], [ 10, %entry ]
  ret i16 %x
five:
  ret i16 50
other:
  ret i16 0
}
";
    let got = listing(text, "f", &cdecl(1));
    // 7 goes to the default anyway; `one` is entered from two blocks of the chain.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "cmp ax, 1",
            "je L0_1",
            "L0_5:",
            "cmp ax, 5",
            "je L0_3",
            "L0_6:",
            "cmp ax, 9",
            "je L0_1",
            "L0_4:",
            "mov ax, 0",
            "pop bp",
            "retf",
            "L0_1:",
            "mov ax, 10",
            "pop bp",
            "retf",
            "L0_3:",
            "mov ax, 50",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_a_dword_result_leaves_in_dx_ax() {
    let convention = Convention { parameters: vec![Home::Frame(6)], returns: vec![Register::EAX, Register::EDX] };
    let got = listing("define i32 @f(i32 %a) {\n  %b = add i32 %a, 1\n  ret i32 %b\n}\n", "f", &convention);
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov eax, dword ptr [bp+6]",
            "inc eax",
            "shld edx, eax, 16",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_comparisons_as_values() {
    let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %lt = icmp slt i16 %a, %b
  %basic = sext i1 %lt to i16
  %eq = icmp eq i16 %a, 3
  %c = zext i1 %eq to i16
  %both = and i1 %lt, %eq
  br i1 %both, label %yes, label %no
yes:
  %s = add i16 %basic, %c
  ret i16 %s
no:
  ret i16 0
}
";
    let got = listing(text, "f", &cdecl(2));
    // An i1 is a byte of 0 or 1: sext is movzx and neg, BASIC's -1.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov cx, word ptr [bp+6]",
            "mov ax, word ptr [bp+8]",
            "cmp cx, ax",
            "setl bl",
            "movzx ax, bl",
            "neg ax",
            "cmp cx, 3",
            "sete dl",
            "movzx cx, dl",
            "and bl, dl",
            "or bl, bl",
            "jne L0_6",
            "L0_8:",
            "mov ax, 0",
            "pop bp",
            "retf",
            "L0_6:",
            "add ax, cx",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_calls_push_as_their_convention_orders() {
    let text = "declare cc1000 i16 @basic(i16, i8) addrspace(1)
declare i32 @c(i16, i16)
define i32 @f(i16 %a) {
  %b = trunc i16 %a to i8
  %x = call cc1000 addrspace(1) i16 @basic(i16 %a, i8 %b)
  %y = call i32 @c(i16 %x, i16 5)
  ret i32 %y
}
";
    let convention = Convention { parameters: vec![Home::Frame(6)], returns: vec![Register::EAX, Register::EDX] };
    let got = listing(text, "f", &convention);
    // BASIC's far callee pops `a`, then the byte widened; C's near one is popped by its caller.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "mov bl, al",
            "push ax",
            "movzx ax, bl",
            "push ax",
            "call far ptr basic",
            "mov bx, 5",
            "push bx",
            "push ax",
            "call c",
            "add sp, 4",
            "movzx ebx, ax",
            "movzx eax, dx",
            "shl eax, 16",
            "or eax, ebx",
            "shld edx, eax, 16",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_near_globals_are_symbols() {
    let text = "@count = internal global i16 5
@table = internal global [3 x i16] [i16 1, i16 2, i16 3]
define i16 @f(i16 %i) {
  %c = load i16, ptr @count
  %d = add i16 %c, 1
  store i16 %d, ptr @count
  %e = load i16, ptr getelementptr inbounds ([3 x i16], ptr @table, i16 0, i16 2)
  %p = getelementptr inbounds [3 x i16], ptr @table, i16 0, i16 %i
  %v = load i16, ptr %p
  %s = add i16 %e, %v
  ret i16 %s
}
";
    let got = listing(text, "f", &cdecl(1));
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "push si",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "add word ptr count, 1",
            "mov ax, word ptr table+4",
            "shl bx, 1",
            "mov si, offset table",
            "add si, bx",
            "add ax, word ptr [si]",
            "pop si",
            "pop bp",
            "retf",
        ]
    );
}

/// An initializer's bytes, and a relocation for each address in it: near,
/// far, a far pointer's offset word, and its segment.
#[test]
fn test_initializers_are_bytes_and_relocations() {
    use crate::backend::masm::{Datum, Label, Pointer};
    let text = "@far = internal addrspace(1) global [2 x i8] c\"HI\"
@rec = internal global { i8, i16, ptr, ptr addrspace(1), i16, ptr addrspace(2) } { i8 7, i16 -2, ptr getelementptr (i8, ptr @rec, i16 3), ptr addrspace(1) @far, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @far, i16 1) to i16), ptr addrspace(2) addrspacecast (ptr addrspace(1) @far to ptr addrspace(2)) }
";
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses");
    let names = crate::backend::globals::names(&module).expect("names");
    let rec = module.named("rec").expect("@rec");
    let pointer = |name: &str, offset, far| Datum::Pointer(Pointer { name: name.to_owned(), offset, far });
    assert_eq!(
        crate::backend::globals::datums(&module, rec, &names).expect("data"),
        [
            Datum::Label(Label { name: "rec".to_owned() }),
            Datum::Bytes(vec![7, 0, 0xFE, 0xFF]),
            pointer("rec", 3, false),
            pointer("far", 0, true),
            pointer("far", 1, false),
            Datum::SegmentWord("far".to_owned()),
        ]
    );
}
