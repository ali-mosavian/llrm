use crate::abi::qb::HirAbi;
use crate::backend::assemble::{self, Abi};
use crate::backend::cpu::ProfileOrName;
use crate::backend::isel::{self, Unselected};
use crate::backend::masm;

const LAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16\"\n";

fn qb() -> HirAbi {
    HirAbi { runtime: crate::hir::model::RuntimeProfile::Qb45, objects: Default::default() }
}

fn parsed(text: &str) -> llrm_mir::Module {
    llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses")
}

fn selected(text: &str, name: &str) -> Result<isel::Selected, Unselected> {
    let contracts = |callee: &str, pops: bool, pushed: i64| qb().contract(callee, pops, pushed);
    isel::selected(&parsed(text), name, &contracts)
}

/// The module's text, once its object is written: a listing that does not
/// encode is no listing.
fn assembled(text: &str) -> String {
    let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Name("486")).expect("assembles");
    crate::backend::omfwrite::written_as(&module, "t.asm", crate::backend::omfwrite::CodeLayout::OneSegment).expect("encodes");
    masm::text(&module).expect("prints")
}

/// The procedure's instructions, through every machine phase.
fn listing(text: &str, name: &str) -> Vec<String> {
    let text = assembled(text);
    let from = text.find(&format!("{name} proc")).expect("the procedure");
    text[from..].lines().skip(1).take_while(|line| !line.ends_with("endp")).map(|line| line.trim().to_owned()).collect()
}

/// A return reads only its result and the epilogue's registers: read as
/// reading every register, it kept bx live and its load unfolded.
#[test]
fn test_arithmetic_on_stack_parameters() {
    let got = listing("define i16 @f(i16 %a, i16 %b) addrspace(1) {\n  %s = sub i16 %a, %b\n  %t = add i16 %s, 3\n  ret i16 %t\n}\n", "f");
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
    let text = "define i16 @sum(i16 %n) addrspace(1) {
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
    let got = listing(text, "sum");
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
    let text = "define i16 @f(i16 %a) addrspace(1) {
  %x = alloca [2 x i16]
  %y = getelementptr inbounds [2 x i16], ptr %x, i16 0, i16 1
  store i16 %a, ptr %y
  store volatile i16 7, ptr %x
  %v = load i16, ptr %y
  ret i16 %v
}
";
    let got = listing(text, "f");
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
    let text = "define i16 @f(ptr %p) addrspace(1) {
  %q = getelementptr inbounds i8, ptr %p, i16 4
  %v = load i16, ptr %q
  %w = trunc i16 %v to i8
  %x = sext i8 %w to i16
  ret i16 %x
}
";
    let got = listing(text, "f");
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
    let text = "define i16 @f(i16 %a) addrspace(1) {
entry:
  %c = icmp sgt i16 5, %a
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 2
}
";
    let got = listing(text, "f");
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
        let text = format!("define i16 @f(ptr %p, i16 %a) addrspace(1) {{\n  {body}\n}}\n");
        assert_eq!(selected(&text, "f").err(), Some(Unselected(why.to_owned())), "{body}");
    }
}

/// A shift counts from cl: the count as a word printed `shl ax, cx`.
#[test]
fn test_division_and_variable_shifts() {
    let text = "define i16 @f(i16 %a, i16 %b) addrspace(1) {
  %q = sdiv i16 %a, %b
  %r = urem i16 %a, %b
  %s = add i16 %q, %r
  %t = shl i16 %s, %b
  ret i16 %t
}
";
    let got = listing(text, "f");
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
    let text = "define i16 @f(ptr %p, i16 %i) addrspace(1) {
  %x = alloca [4 x i16]
  %y = getelementptr inbounds [4 x i16], ptr %x, i16 0, i16 %i
  store i16 5, ptr %y
  %q = getelementptr inbounds { i16, [3 x i16] }, ptr %p, i16 0, i32 1, i16 %i
  %v = load i16, ptr %q
  ret i16 %v
}
";
    let got = listing(text, "f");
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
    let text = "define i16 @f(i16 %a) addrspace(1) {
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
    let got = listing(text, "f");
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
    let got = listing("define i32 @f(i32 %a) addrspace(1) {\n  %b = add i32 %a, 1\n  ret i32 %b\n}\n", "f");
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
    let text = "define i16 @f(i16 %a, i16 %b) addrspace(1) {
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
    let got = listing(text, "f");
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
define i32 @f(i16 %a) addrspace(1) {
  %b = trunc i16 %a to i8
  %x = call cc1000 addrspace(1) i16 @basic(i16 %a, i8 %b)
  %y = call i32 @c(i16 %x, i16 5)
  ret i32 %y
}
";
    let got = listing(text, "f");
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
define i16 @f(i16 %i) addrspace(1) {
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
    let got = listing(text, "f");
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
    let names = crate::backend::globals::names(&module, &|name| qb().linked(name)).expect("names");
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

/// Where arguments arrive is the MIR's: BASIC's pushed first-to-last and
/// popped by the callee, C's near one first-nearest, BP above a near return.
/// An unused argument is not loaded: it was, and machine DCE keeps loads.
#[test]
fn test_parameters_arrive_as_the_convention_pushed_them() {
    let text = "define cc1000 i16 @basic(i16 %a, i32 %b, i16 %c) addrspace(1) {
  %d = sub i16 %a, %c
  ret i16 %d
}
define i16 @near(i16 %a, i16 %b) {
  %d = sub i16 %a, %b
  ret i16 %d
}
";
    assert_eq!(
        listing(text, "basic"),
        ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+12]", "sub ax, word ptr [bp+6]", "pop bp", "retf 8"]
    );
    assert_eq!(
        listing(text, "near"),
        ["push bp", "mov bp, sp", "L1_0:", "mov ax, word ptr [bp+4]", "sub ax, word ptr [bp+6]", "pop bp", "ret"]
    );
}

/// A module whole: a runtime routine extern by its linked name, an external
/// function public, an internal one private, and its data.
#[test]
fn test_a_module_is_its_procedures_externs_publics_and_data() {
    let text = "@count = internal global i16 5
declare cc1000 void @\"llrm.qb.B$PEI2\"(i16) addrspace(1)
define internal cc1000 void @helper(i16 %a) addrspace(1) {
  call cc1000 addrspace(1) void @\"llrm.qb.B$PEI2\"(i16 %a)
  ret void
}
define cc1000 void @MAIN() addrspace(1) {
  %c = load i16, ptr @count
  call cc1000 addrspace(1) void @helper(i16 %c)
  ret void
}
";
    let got = assembled(text);
    assert_eq!(
        got.lines().map(str::trim).collect::<Vec<_>>(),
        [
            ".model medium",
            ".386",
            "",
            "public MAIN",
            ".data",
            "count label byte",
            "db 005h,000h",
            "extern B$PEI2:far",
            ".code T_TEXT",
            "helper proc far",
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "push ax",
            "call far ptr B$PEI2",
            "pop bp",
            "retf 2",
            "helper endp",
            "MAIN proc far",
            "L1_0:",
            "mov ax, word ptr count",
            "push ax",
            "call far ptr helper",
            "retf",
            "MAIN endp",
            "end",
        ]
    );
}

/// An intrinsic is no symbol: declaring one refused the module as "no
/// assembler symbol".
#[test]
fn test_a_declared_intrinsic_names_no_symbol() {
    let text = "declare i16 @llvm.smax.i16(i16, i16)
define i16 @f(i16 %a) addrspace(1) {
  ret i16 %a
}
";
    assert_eq!(listing(text, "f"), ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+6]", "pop bp", "retf"]);
}

/// A memset expands as LLVM's getMemset does: up to 16 stores, widest
/// first; beyond that `rep stosd` through es:di, the tail stored.
#[test]
fn test_a_memset_is_stores_or_a_string_fill() {
    let text = |size: u32| {
        format!(
            "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define i16 @f() addrspace(1) {{
  %a = alloca [{size} x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 1, i16 {size}, i1 false)
  %v = load i16, ptr %a
  ret i16 %v
}}
"
        )
    };
    assert_eq!(
        listing(&text(7), "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 8",
            "L0_0:",
            "mov dword ptr [bp-8], 16843009",
            "mov word ptr [bp-4], 257",
            "mov byte ptr [bp-2], 1",
            "mov ax, word ptr [bp-8]",
            "leave",
            "retf",
        ]
    );
    assert_eq!(
        listing(&text(70), "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 70",
            "push di",
            "L0_0:",
            "lea di, [bp-70]",
            "push es",
            "push ss",
            "pop es",
            "mov eax, 16843009",
            "mov cx, 17",
            "rep stosd",
            "pop es",
            "mov word ptr [bp-2], 257",
            "mov ax, word ptr [bp-70]",
            "pop di",
            "leave",
            "retf",
        ]
    );
}

/// A far pointer is its offset and selector, as a type legalizer expands a
/// value no register holds: accessed through es, pushed selector first,
/// returned in dx:ax, and a near pointer made far in DGROUP.
#[test]
fn test_far_pointers_are_an_offset_and_a_selector() {
    let text = "@buf = internal global [4 x i16] zeroinitializer
declare void @take(ptr addrspace(1), i16)
define ptr addrspace(1) @f(ptr addrspace(1) %p, i16 %i) addrspace(1) {
  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i
  %v = load i16, ptr addrspace(1) %q
  %r = getelementptr i8, ptr addrspace(1) %p, i16 4
  store i16 %v, ptr addrspace(1) %r
  %b = addrspacecast ptr @buf to ptr addrspace(1)
  call void @take(ptr addrspace(1) %b, i16 %v)
  ret ptr addrspace(1) %r
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 4",
            "L0_0:",
            "mov ax, word ptr [bp+6]",
            "mov word ptr [bp-2], ax",
            "mov ax, word ptr [bp+8]",
            "mov word ptr [bp-4], ax",
            "mov bx, word ptr [bp+10]",
            "shl bx, 1",
            "add bx, word ptr [bp-2]",
            "mov es, word ptr [bp-4]",
            "mov ax, word ptr es:[bx]",
            "mov bx, word ptr [bp-2]",
            "mov word ptr es:[bx+4], ax",
            "mov bx, offset buf",
            "mov cx, DGROUP",
            "push ax",
            "push cx",
            "push bx",
            "call take",
            "add sp, 6",
            "mov ax, word ptr [bp-2]",
            "add ax, 4",
            "mov dx, word ptr [bp-4]",
            "leave",
            "retf",
        ]
    );
}

/// Floats are x87 values: loads fold into arithmetic, a result leaves in
/// st(0), an argument is pushed from a stack temporary, conversions go
/// through memory, and an ordered compare is fcom, sahf and ja.
#[test]
fn test_floats_are_x87_values() {
    let text = "@k = internal global double 1.5
@out = internal global float 0.0
declare cc1000 void @show(float) addrspace(1)
declare i16 @llvm.lrint.i16.f64(double)
define double @scale(double %x, double %y) addrspace(1) {
  %m = fmul double %x, %y
  %c = load double, ptr @k
  %s = fadd double %m, %c
  ret double %s
}
define i16 @f(i16 %n) addrspace(1) {
  %a = sitofp i16 %n to float
  %b = fpext float %a to double
  %r = call addrspace(1) double @scale(double %b, double %b)
  %t = fptrunc double %r to float
  store float %t, ptr @out
  call cc1000 addrspace(1) void @show(float %t)
  %g = fcmp ogt double %r, %b
  br i1 %g, label %big, label %small
big:
  %i = call i16 @llvm.lrint.i16.f64(double %r)
  ret i16 %i
small:
  ret i16 0
}
";
    assert_eq!(
        listing(text, "scale"),
        ["push bp", "mov bp, sp", "L0_0:", "fld qword ptr [bp+6]", "fmul qword ptr [bp+14]", "fadd qword ptr k", "pop bp", "retf"]
    );
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 48",
            "L1_0:",
            "mov ax, word ptr [bp+6]",
            "mov word ptr [bp-2], ax",
            "fild word ptr [bp-2]",
            "fstp tbyte ptr [bp-38]",
            "fld tbyte ptr [bp-38]",
            "fst qword ptr [bp-10]",
            "push dword ptr [bp-6]",
            "push dword ptr [bp-10]",
            "fstp qword ptr [bp-18]",
            "push dword ptr [bp-14]",
            "push dword ptr [bp-18]",
            "call far ptr scale",
            "fstp tbyte ptr [bp-48]",
            "add sp, 16",
            "fld tbyte ptr [bp-48]",
            "fstp dword ptr [bp-22]",
            "fld dword ptr [bp-22]",
            "fst dword ptr out",
            "fstp dword ptr [bp-26]",
            "push dword ptr [bp-26]",
            "call far ptr show",
            "fld tbyte ptr [bp-48]",
            "fld tbyte ptr [bp-38]",
            "fxch st(1)",
            "fcompp",
            "fnstsw ax",
            "sahf",
            "ja L1_8",
            "L1_10:",
            "mov ax, 0",
            "leave",
            "retf",
            "L1_8:",
            "fld tbyte ptr [bp-48]",
            "fistp word ptr [bp-28]",
            "mov ax, word ptr [bp-28]",
            "leave",
            "retf",
        ]
    );
}

/// A float function x87 computes in one instruction is that instruction.
/// frndint, fsin and fcos listed but did not encode.
#[test]
fn test_float_functions_are_x87_instructions() {
    let text = "declare double @llvm.sqrt.f64(double)
declare double @llvm.rint.f64(double)
declare double @llvm.fabs.f64(double)
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
define double @f(double %x) addrspace(1) {
  %a = call double @llvm.sqrt.f64(double %x)
  %b = call double @llvm.rint.f64(double %a)
  %c = call double @llvm.fabs.f64(double %b)
  %d = call double @llvm.sin.f64(double %c)
  %e = call double @llvm.cos.f64(double %d)
  ret double %e
}
";
    assert_eq!(
        listing(text, "f"),
        ["push bp", "mov bp, sp", "L0_0:", "fld qword ptr [bp+6]", "fsqrt", "frndint", "fabs", "fsin", "fcos", "pop bp", "retf"]
    );
}


/// fptosi is fisttp; fptoui, which x87 cannot store, is a dword's low word.
#[test]
fn test_a_float_to_an_integer_is_stored_toward_zero() {
    let text = "define i16 @f(double %x) addrspace(1) {
  %s = fptosi double %x to i16
  %u = fptoui double %x to i16
  %r = add i16 %s, %u
  ret i16 %r
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 10",
            "L0_0:",
            "fnstcw word ptr [bp-8]",
            "mov ax, word ptr [bp-8]",
            "or ax, 3072",
            "mov word ptr [bp-10], ax",
            "fld qword ptr [bp+6]",
            "fld st(0)",
            "fldcw word ptr [bp-10]",
            "fistp word ptr [bp-2]",
            "fldcw word ptr [bp-8]",
            "mov ax, word ptr [bp-2]",
            "fldcw word ptr [bp-10]",
            "fistp dword ptr [bp-6]",
            "fldcw word ptr [bp-8]",
            "add ax, word ptr [bp-6]",
            "leave",
            "retf",
        ]
    );
}
