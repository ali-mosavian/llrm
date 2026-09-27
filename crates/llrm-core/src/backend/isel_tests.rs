use crate::abi::qb::HirAbi;
use crate::backend::assemble::{self, Abi};
use crate::backend::constpool::Pool;
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
    isel::selected(&parsed(text), name, &contracts, &mut Pool::new(0), crate::backend::cpu::profile("486").expect("a target"), &crate::backend::target::BUILT_IN)
}

/// The module's text, once its object is written: a listing that does not
/// encode is no listing.
fn assembled(text: &str) -> String {
    assembled_on("486", text)
}

/// The module's text as `cpu` prices it.
fn assembled_on(cpu: &str, text: &str) -> String {
    let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Name(cpu), &crate::backend::target::BUILT_IN).expect("assembles");
    crate::backend::omfwrite::written_as(&module, "t.asm", crate::backend::omfwrite::CodeLayout::OneSegment).expect("encodes");
    masm::text(&module).expect("prints")
}

/// The procedure's instructions, through every machine phase.
fn listing(text: &str, name: &str) -> Vec<String> {
    listing_on("486", text, name)
}

/// The procedure's instructions as `cpu` prices them.
fn listing_on(cpu: &str, text: &str, name: &str) -> Vec<String> {
    let text = assembled_on(cpu, text);
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

/// A variable index is scaled, and an address only accesses read is their
/// base plus it: `[bp+di-8]`, `[bx+si+2]`.
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
            "push di",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "mov si, word ptr [bp+8]",
            "lea di, [esi+esi]",
            "mov word ptr ss:[bp+di-8], 5",
            "add si, si",
            "mov ax, word ptr [bx+si+2]",
            "pop di",
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
            "cmp cx, word ptr [bp+8]",
            "setl bl",
            "movzx ax, bl",
            "neg ax",
            "cmp cx, 3",
            "sete dl",
            "movzx cx, dl",
            "and bl, dl",
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
            "pushw 5",
            "push ax",
            "call c",
            "add sp, 4",
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
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "add word ptr count, 1",
            "mov ax, word ptr table+4",
            "add bx, bx",
            "add ax, word ptr table[bx]",
            "pop bp",
            "retf",
        ]
    );
}

/// An initializer's bytes, and a relocation for each address in it: near,
/// far, a far pointer's offset word, and its segment. A far pointer to near
/// data is DGROUP's: its group offset and DGROUP's selector.
#[test]
fn test_initializers_are_bytes_and_relocations() {
    use crate::backend::masm::{Datum, Label, Pointer};
    let text = "@far = internal addrspace(1) global [2 x i8] c\"HI\"
@rec = internal global { i8, i16, ptr, ptr addrspace(1), i16, ptr addrspace(2), ptr addrspace(1) } { i8 7, i16 -2, ptr getelementptr (i8, ptr @rec, i16 3), ptr addrspace(1) @far, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @far, i16 1) to i16), ptr addrspace(2) addrspacecast (ptr addrspace(1) @far to ptr addrspace(2)), ptr addrspace(1) addrspacecast (ptr getelementptr (i8, ptr @rec, i16 1) to ptr addrspace(1)) }
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
            pointer("rec", 1, false),
            Datum::SegmentWord("DGROUP".to_owned()),
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
            "push word ptr [bp+6]",
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
/// first; beyond that `rep stosd` through es:di, the tail by `stosw` and
/// `stosb`, as the old route's `_fill`.
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
            "stosw",
            "pop es",
            "mov ax, word ptr [bp-70]",
            "pop di",
            "leave",
            "retf",
        ]
    );
}

/// fixed.nib's `left * right / right` in 16.16: a 64-bit product and
/// quotient of 32-bit factors, which isel refused as "a i64 value". Each
/// i64 is a pair of dwords; factors and a divisor sign-extended from i32
/// take one imul and a long division of magnitudes, as the old path did.
#[test]
fn test_a_fixed_product_and_quotient_take_dword_pairs() {
    let text = "define i32 @scaled(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = mul i64 %2, %3
  %5 = ashr i64 %4, 16
  %6 = trunc i64 %5 to i32
  %7 = sext i32 %6 to i64
  %8 = sext i32 %1 to i64
  %9 = shl i64 %7, 16
  %10 = sdiv i64 %9, %8
  %11 = trunc i64 %10 to i32
  ret i32 %11
}
";
    assert_eq!(
        listing(text, "scaled"),
        [
            "push bp",
            "mov bp, sp",
            "push si",
            "push di",
            "L0_0:",
            "mov eax, dword ptr [bp+6]",
            "mov ebx, dword ptr [bp+10]",
            "imul ebx",
            "mov ecx, eax",
            "shrd ecx, edx, 16",
            "mov eax, ecx",
            "cdq",
            "mov ax, dx",
            "shld eax, ecx, 16",
            "shl ecx, 16",
            "mov esi, eax",
            "sar esi, 31",
            "xor ecx, esi",
            "xor eax, esi",
            "sub ecx, esi",
            "sbb eax, esi",
            "mov edi, ebx",
            "sar edi, 31",
            "xor ebx, edi",
            "sub ebx, edi",
            "xor edx, edx",
            "div ebx",
            "mov eax, ecx",
            "div ebx",
            "xor esi, edi",
            "xor eax, esi",
            "sub eax, esi",
            "shld edx, eax, 16",
            "pop di",
            "pop si",
            "pop bp",
            "retf",
        ]
    );
}

/// A call names its callee as the procedure is defined: runtime.nib's
/// `buffers.allocate`, no assembler symbol, was defined as `G$0` but called
/// as `_buffers.allocate`, and the runtime did not link.
#[test]
fn test_a_call_names_an_internal_callee_as_it_is_defined() {
    let text = "define internal i16 @buffers.allocate() addrspace(1) {
  ret i16 1
}
define i16 @f() addrspace(1) {
  %v = call addrspace(1) i16 @buffers.allocate()
  ret i16 %v
}
";
    let text = assembled(text);
    assert!(text.contains("G$0 proc far"), "{text}");
    assert!(text.lines().any(|line| line.trim() == "call far ptr G$0"), "{text}");
}

/// Constant folding leaves `br i1 true` and a float constant as an operand,
/// both valid MIR, which isel refused: "a branch on a constant", "a float
/// constant operand". A branch on a constant jumps; a float constant loads
/// from the pool in its narrowest exact format, as LLVM's does.
#[test]
fn test_a_constant_condition_and_a_float_constant_select() {
    let text = "define i16 @f() addrspace(1) {
b1:
  br i1 true, label %b2, label %b3

b2:
  %x = fadd double 1.5, 2.0
  %y = fptosi double %x to i16
  ret i16 %y

b3:
  ret i16 0
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp", "mov bp, sp", "sub sp, 6", "L0_0:", "fnstcw word ptr [bp-4]", "mov ax, word ptr [bp-4]", "or ax, 3072", "mov word ptr [bp-6], ax", "L0_1:",
            "fld dword ptr $K1", "fadd dword ptr $K2", "fldcw word ptr [bp-6]", "fistp word ptr [bp-2]", "fldcw word ptr [bp-4]",
            "mov ax, word ptr [bp-2]", "leave", "retf",
        ]
    );
}

/// A phi whose input comes along an edge a constant branch never takes:
/// isel panicked, "phis_from made every other input".
#[test]
fn test_a_phi_ignores_an_edge_never_taken() {
    let text = "define i16 @f() addrspace(1) {
b1:
  br i1 true, label %b3, label %b2

b2:
  br label %b3

b3:
  %0 = phi i16 [ 7, %b1 ], [ 0, %b2 ]
  ret i16 %0
}
";
    assert_eq!(listing(text, "f"), ["L0_0:", "L0_1:", "mov ax, 7", "retf"]);
}

/// A far null is offset 0, selector 0: runtime.nib's errors.say stored one
/// and isel refused it as "an address of no global".
#[test]
fn test_a_far_null_is_two_zero_words() {
    let text = "declare void @take(ptr addrspace(1))
define void @f() addrspace(1) {
  %cell = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %cell
  call void @take(ptr addrspace(1) null)
  ret void
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp", "mov bp, sp", "sub sp, 4", "L0_0:", "mov word ptr [bp-4], 0", "mov word ptr [bp-2], 0", "pushd 0", "call take", "add sp, 4", "leave", "retf",
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
            "push si",
            "L0_0:",
            "les si, dword ptr [bp+6]",
            "mov bx, word ptr [bp+10]",
            "add bx, bx",
            "mov ax, word ptr es:[bx+si]",
            "mov word ptr es:[si+4], ax",
            "mov word ptr [bp-4], si",
            "mov bx, es",
            "mov word ptr [bp-2], bx",
            "push ax",
            "pushw DGROUP",
            "push offset buf",
            "call take",
            "add sp, 6",
            "mov ax, word ptr [bp-4]",
            "add ax, 4",
            "mov dx, word ptr [bp-2]",
            "pop si",
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
            "sub sp, 36",
            "L1_0:",
            "mov ax, word ptr [bp+6]",
            "mov word ptr [bp-2], ax",
            "fild word ptr [bp-2]",
            "fst qword ptr [bp-10]",
            "push dword ptr [bp-6]",
            "push dword ptr [bp-10]",
            "fstp qword ptr [bp-18]",
            "push dword ptr [bp-14]",
            "push dword ptr [bp-18]",
            "call far ptr scale",
            "add sp, 16",
            "fst dword ptr [bp-22]",
            "fld dword ptr [bp-22]",
            "fst dword ptr out",
            "fstp dword ptr [bp-26]",
            "push dword ptr [bp-26]",
            "fstp qword ptr [bp-36]",
            "call far ptr show",
            "fld qword ptr [bp-36]",
            "fild word ptr [bp-2]",
            "fld st(1)",
            "fcompp",
            "fnstsw ax",
            "sahf",
            "ja L1_8",
            "L1_10:",
            "fstp st(0)",
            "mov ax, 0",
            "leave",
            "retf",
            "L1_8:",
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

/// A fixed-point quotient by a power of two was a long division, two
/// `div`s and the signs around them: matmul_fixed's `/ 4` ran 23280
/// instructions to the old path's 17402. It is a biased arithmetic shift.
#[test]
fn test_an_i64_divided_by_a_power_of_two_is_shifted() {
    let text = "define i32 @quotient(i32 %0) addrspace(1) {
b1:
  %1 = sext i32 %0 to i64
  %2 = shl i64 %1, 16
  %3 = sdiv i64 %2, 1024
  %4 = trunc i64 %3 to i32
  ret i32 %4
}
define i32 @remainder(i32 %0) addrspace(1) {
b1:
  %1 = sext i32 %0 to i64
  %2 = shl i64 %1, 16
  %3 = srem i64 %2, 1024
  %4 = trunc i64 %3 to i32
  ret i32 %4
}
";
    assert_eq!(
        listing(text, "quotient"),
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov ebx, dword ptr [bp+6]",
            "mov eax, ebx",
            "cdq",
            "shld edx, ebx, 16",
            "shl ebx, 16",
            "mov eax, edx",
            "sar eax, 31",
            "shr eax, 22",
            "xor ecx, ecx",
            "add ebx, eax",
            "adc edx, ecx",
            "shrd ebx, edx, 10",
            "shld edx, ebx, 16",
            "mov ax, bx",
            "pop bp",
            "retf",
        ]
    );
    assert_eq!(
        listing(text, "remainder"),
        [
            "push bp",
            "mov bp, sp",
            "L1_0:",
            "mov ebx, dword ptr [bp+6]",
            "mov eax, ebx",
            "cdq",
            "shld edx, ebx, 16",
            "shl ebx, 16",
            "sar edx, 31",
            "shr edx, 22",
            "add edx, ebx",
            "and edx, 4294966272",
            "sub ebx, edx",
            "shld edx, ebx, 16",
            "mov ax, bx",
            "pop bp",
            "retf",
        ]
    );
}

/// A float phi's constant input is loaded as any float constant is: taken
/// for an address, it refused `an address of no global` and left deedlines'
/// CREATEOBJECT and qbdemo's FRACLINE unselected.
#[test]
fn test_a_float_phi_takes_a_constant_input() {
    let text = "define double @f(i16 %n) addrspace(1) {
entry:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %more, label %done
more:
  br label %done
done:
  %x = phi double [ 0.0, %entry ], [ 1.0, %more ]
  ret double %x
}
";
    assert_eq!(
        listing(text, "f"),
        ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+6]", "fldz", "or ax, ax", "jle L0_3", "L0_2:", "fstp st(0)", "fld1", "L0_3:", "pop bp", "retf"]
    );
}

/// A far pointer's i16 is its offset word, as LLVM truncates a ptrtoint: it
/// refused, and left oimad's main and qbdemo's BENCHMARK unselected.
#[test]
fn test_a_far_pointer_as_a_word_is_its_offset() {
    let text = "define i16 @f(ptr addrspace(1) %p) addrspace(1) {
  %o = ptrtoint ptr addrspace(1) %p to i16
  ret i16 %o
}
";
    let got = listing(text, "f");
    assert!(got.contains(&"mov ax, word ptr [bp+6]".to_owned()), "{got:?}");
}

/// Float equality holds only when ordered: `fcom` leaves unordered as
/// equal with PF set, so oeq is `sete` and `setnp`, une `setne` or `setp`.
#[test]
fn test_float_equality_is_equal_and_ordered() {
    let text = "define i1 @eq(double %a, double %b) addrspace(1) {
  %c = fcmp oeq double %a, %b
  ret i1 %c
}
define i1 @ne(double %a, double %b) addrspace(1) {
  %c = fcmp une double %a, %b
  ret i1 %c
}
";
    let compared = ["push bp", "mov bp, sp", "fld qword ptr [bp+6]", "fcomp qword ptr [bp+14]", "fnstsw ax", "sahf"];
    let answered = |name: &str| listing(text, name).into_iter().filter(|line| !compared.contains(&line.as_str()) && !line.ends_with(':')).collect::<Vec<_>>();
    assert_eq!(answered("eq"), ["sete al", "setnp bl", "and al, bl", "pop bp", "retf"]);
    assert_eq!(answered("ne"), ["setne al", "setp bl", "or al, bl", "pop bp", "retf"]);
}

/// An unordered predicate is the carry or zero unordered also sets: ult is
/// `setb`, ugt the same with the operands the other way. fpemu's `fcmp ult`
/// was refused.
#[test]
fn test_float_unordered_predicates_are_below() {
    let text = "define i1 @ult(double %a, double %b) addrspace(1) {
  %c = fcmp ult double %a, %b
  ret i1 %c
}
define i1 @uge(double %a, double %b) addrspace(1) {
  %c = fcmp uge double %a, %b
  ret i1 %c
}
";
    let prologue = ["push bp", "mov bp, sp", "fnstsw ax", "sahf", "pop bp", "retf"];
    let answered = |name: &str| listing(text, name).into_iter().filter(|line| !prologue.contains(&line.as_str()) && !line.ends_with(':')).collect::<Vec<_>>();
    assert_eq!(answered("ult"), ["fld qword ptr [bp+6]", "fcomp qword ptr [bp+14]", "setb al"]);
    assert_eq!(answered("uge"), ["fld qword ptr [bp+14]", "fcomp qword ptr [bp+6]", "setbe al"]);
}

/// A port below 256 is an immediate, any other is in dx; the byte is in al.
#[test]
fn test_ports_are_in_and_out() {
    let text = "declare i8 @llrm.ia16.in.i8(i16)
declare void @llrm.ia16.out.i8(i16, i8)
define void @f(i16 %port) addrspace(1) {
  %v = call i8 @llrm.ia16.in.i8(i16 96)
  call void @llrm.ia16.out.i8(i16 968, i8 %v)
  call void @llrm.ia16.out.i8(i16 %port, i8 %v)
  ret void
}
";
    assert_eq!(
        listing(text, "f"),
        ["push bp", "mov bp, sp", "L0_0:", "mov bx, word ptr [bp+6]", "in al, 96", "mov dx, 968", "out dx, al", "mov dx, bx", "out dx, al", "pop bp", "retf"]
    );
}

/// B$SCMP's result is the flags: its compare with zero is the call itself.
#[test]
fn test_a_flags_result_is_read_where_its_call_leaves_it() {
    let text = "declare cc1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)
define i16 @f(ptr %a, ptr %b) addrspace(1) {
entry:
  %s = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c = icmp sgt i16 %s, 0
  br i1 %c, label %greater, label %other
greater:
  ret i16 1
other:
  ret i16 0
}
";
    let got = listing(text, "f");
    let call = got.iter().position(|line| line == "call far ptr B$SCMP").expect("the call");
    assert_eq!(got[call + 1], "jg L0_3", "{got:?}");
}

/// A float function no x87 instruction is stays one operation on st(0),
/// which the frontend's finalizer spells.
#[test]
fn test_log2_exp2_and_atan_are_single_x87_operations() {
    let text = "declare double @llvm.log2.f64(double)
declare double @llvm.exp2.f64(double)
declare double @llvm.atan.f64(double)
define double @f(double %x) addrspace(1) {
  %a = call double @llvm.log2.f64(double %x)
  %b = call double @llvm.exp2.f64(double %a)
  %c = call double @llvm.atan.f64(double %b)
  ret double %c
}
";
    let body = selected(text, "f").expect("selects").body;
    let names: Vec<String> = body.blocks.iter().flat_map(|block| &block.insns).filter_map(|one| one.what.as_ref()).filter(|what| what.op == crate::model::ir::Operation::FloatUnary).filter_map(|what| what.name.clone()).collect();
    assert_eq!(names, ["flog2", "fexp2", "fatan"]);
}

/// A segment's far pointer is offset 0: plasma's element address was
/// `xor bx,bx; add bx,cx`, a zero made and added each iteration.
#[test]
fn test_a_segments_element_is_addressed_by_its_index_alone() {
    let text = "define i16 @f(ptr %d, i16 %i) addrspace(1) {
  %s = load i16, ptr %d
  %p = inttoptr i16 %s to ptr addrspace(2)
  %f = addrspacecast ptr addrspace(2) %p to ptr addrspace(1)
  %e = getelementptr i16, ptr addrspace(1) %f, i16 %i
  %v = load i16, ptr addrspace(1) %e
  ret i16 %v
}
";
    let got = listing(text, "f");
    assert!(!got.iter().any(|line| line.starts_with("xor")), "{got:?}");
    assert_eq!(got.iter().filter(|line| line.starts_with("add")).count(), 1, "{got:?}");
}

/// A static array's element is [index+symbol]: plasmablobs built each
/// address as `mov di,sym; add di,cx` before reading it.
#[test]
fn test_a_global_arrays_element_is_addressed_by_its_symbol_and_index() {
    let text = "@a = global [8 x i16] zeroinitializer

define void @f(i16 %i) addrspace(1) {
  %e = getelementptr [8 x i16], ptr @a, i16 0, i16 %i
  %v = load i16, ptr %e
  %w = add i16 %v, 1
  store i16 %w, ptr %e
  ret void
}
";
    let got = listing(text, "f");
    assert_eq!(got[3..6], ["mov bx, word ptr [bp+6]", "add bx, bx", "add word ptr a[bx], 1"], "{got:?}");
}

/// An integer x87 converts is read from memory: through a frame temporary,
/// qbdemo's `x / i%` stored and reloaded every divisor.
#[test]
fn test_an_integer_load_only_a_conversion_reads_is_the_x87_operand() {
    let text = "define double @f(double %x, ptr %p) addrspace(1) {
  %v = load i16, ptr %p
  %w = sitofp i16 %v to double
  %q = fdiv double %x, %w
  ret double %q
}
";
    assert_eq!(
        listing(text, "f"),
        ["push bp", "mov bp, sp", "L0_0:", "mov bx, word ptr [bp+14]", "fild word ptr [bx]", "fdivr qword ptr [bp+6]", "pop bp", "retf"]
    );
}

/// A load read again is in a register, and x87 reads that.
#[test]
fn test_an_integer_load_read_again_is_converted_from_its_register() {
    let text = "define double @f(double %x, ptr %p) addrspace(1) {
  %v = load i16, ptr %p
  %w = sitofp i16 %v to double
  %q = fdiv double %x, %w
  %n = add i16 %v, 1
  store i16 %n, ptr %p
  ret double %q
}
";
    let got = listing(text, "f");
    assert!(got.contains(&"mov ax, word ptr [bx]".to_owned()) && got.contains(&"fidiv word ptr [bp-2]".to_owned()), "{got:?}");
}

/// A store between the load and its conversion may change the cell: x87
/// reads what the load read.
#[test]
fn test_an_integer_load_before_a_store_is_not_read_after_it() {
    let text = "define double @f(double %x, ptr %p, ptr %r) addrspace(1) {
  %v = load i16, ptr %p
  store i16 0, ptr %r
  %w = sitofp i16 %v to double
  %q = fdiv double %x, %w
  ret double %q
}
";
    let got = listing(text, "f");
    let (load, store) = (got.iter().position(|one| one == "mov ax, word ptr [bx]"), got.iter().position(|one| one == "mov word ptr [si], 0"));
    assert!(load < store && got.contains(&"fidiv word ptr [bp-2]".to_owned()), "{got:?}");
}

/// A volatile load is its own access, not an x87 operand.
#[test]
fn test_a_volatile_load_is_not_an_x87_operand() {
    let text = "define double @f(double %x, ptr %p) addrspace(1) {
  %v = load volatile i32, ptr %p
  %w = sitofp i32 %v to double
  %q = fmul double %x, %w
  ret double %q
}
";
    let got = listing(text, "f");
    assert!(got.contains(&"mov eax, dword ptr [bx]".to_owned()) && got.contains(&"fimul dword ptr [bp-4]".to_owned()), "{got:?}");
}

/// A conversion only a store reads is stored by x87 into the store's cell:
/// fptosi, lrint and fptrunc each went through a frame temporary.
#[test]
fn test_a_conversion_only_a_store_reads_is_stored_by_x87() {
    let text = "@b = internal global i16 0
@o = internal global float 0.0
declare i16 @llvm.lrint.i16.f64(double)
define void @f(double %x) addrspace(1) {
  %i = fptosi double %x to i16
  store i16 %i, ptr @b
  %j = call i16 @llvm.lrint.i16.f64(double %x)
  store i16 %j, ptr @b
  %t = fptrunc double %x to float
  store float %t, ptr @o
  ret void
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "sub sp, 4",
            "L0_0:",
            "fnstcw word ptr [bp-2]",
            "mov ax, word ptr [bp-2]",
            "or ax, 3072",
            "mov word ptr [bp-4], ax",
            "fld qword ptr [bp+6]",
            "fld st(0)",
            "fldcw word ptr [bp-4]",
            "fistp word ptr b",
            "fldcw word ptr [bp-2]",
            "fld st(0)",
            "fistp word ptr b",
            "fstp dword ptr o",
            "leave",
            "retf",
        ]
    );
}

/// A conversion read again is in a register, stored from there.
#[test]
fn test_a_conversion_read_again_is_stored_through_its_register() {
    let text = "@b = internal global i16 0
define i16 @f(double %x) addrspace(1) {
  %i = fptosi double %x to i16
  store i16 %i, ptr @b
  %r = add i16 %i, 1
  ret i16 %r
}
";
    let got = listing(text, "f");
    assert!(got.contains(&"fistp word ptr [bp-2]".to_owned()) && got.contains(&"mov word ptr b, ax".to_owned()), "{got:?}");
}

/// The instructions between a procedure's label and its epilogue.
fn inner(text: &str) -> Vec<String> {
    inner_on("486", text)
}

/// The instructions between a procedure's label and its epilogue, as `cpu` prices them.
fn inner_on(cpu: &str, text: &str) -> Vec<String> {
    let got = listing_on(cpu, text, "f");
    let from = got.iter().position(|one| one.ends_with(':')).expect("a label") + 1;
    got[from..].iter().take_while(|one| !one.starts_with("pop ") && *one != "leave" && *one != "retf").cloned().collect()
}

/// A load, its update and its store back to the cell are one instruction,
/// as the old route's rmw selects: nbody kept each field in a temporary.
#[test]
fn test_an_update_stored_back_to_its_cell_is_one_instruction() {
    let update = |operation: &str, ty: &str| {
        inner(&format!(
            "define void @f(ptr %p, {ty} %x) addrspace(1) {{\n  %v = load {ty}, ptr %p\n  %s = {operation} {ty} %v, %x\n  store {ty} %s, ptr %p\n  ret void\n}}\n"
        ))
    };
    assert_eq!(update("add", "i16"), ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bp+8]", "add word ptr [bx], ax"]);
    assert_eq!(update("sub", "i16"), ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bp+8]", "sub word ptr [bx], ax"]);
    assert_eq!(update("or", "i8"), ["mov bx, word ptr [bp+6]", "mov al, byte ptr [bp+8]", "or byte ptr [bx], al"]);
    let global = "@a = internal global i16 0
define void @f() addrspace(1) {
  %v = load i16, ptr @a
  %s = add i16 %v, 3
  store i16 %s, ptr @a
  ret void
}
";
    assert_eq!(inner(global), ["add word ptr a, 3"]);
}

/// `x - [p]` is no `sub [p], x`: the cell must be the left operand.
#[test]
fn test_a_subtraction_from_a_value_does_not_update_the_cell() {
    let text = "define void @f(ptr %p, i16 %x) addrspace(1) {
  %v = load i16, ptr %p
  %s = sub i16 %x, %v
  store i16 %s, ptr %p
  ret void
}
";
    assert_eq!(inner(text), ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bp+8]", "sub ax, word ptr [bx]", "mov word ptr [bx], ax"]);
}

/// No update in memory where the store goes elsewhere, the loaded value is
/// read again, or the accesses are volatile.
#[test]
fn test_an_update_is_not_one_instruction_unless_private_to_its_cell() {
    let elsewhere = "define void @f(ptr %p, ptr %q, i16 %x) addrspace(1) {
  %v = load i16, ptr %p
  %s = add i16 %v, %x
  store i16 %s, ptr %q
  ret void
}
";
    assert!(inner(elsewhere).iter().all(|one| !one.starts_with("add word ptr [")), "{:?}", inner(elsewhere));
    let again = "define i16 @f(ptr %p, i16 %x) addrspace(1) {
  %v = load i16, ptr %p
  %s = add i16 %v, %x
  store i16 %s, ptr %p
  ret i16 %v
}
";
    assert!(inner(again).iter().all(|one| !one.starts_with("add word ptr [")), "{:?}", inner(again));
    let volatile = "define void @f(ptr %p, i16 %x) addrspace(1) {
  %v = load volatile i16, ptr %p
  %s = add i16 %v, %x
  store volatile i16 %s, ptr %p
  ret void
}
";
    assert_eq!(inner(volatile), ["mov bx, word ptr [bp+6]", "mov cx, word ptr [bx]", "add cx, word ptr [bp+8]", "mov word ptr [bx], cx"]);
}

/// A load only a comparison reads is its operand, as comparefold selects;
/// a zero-extended byte tested for zero is compared as the byte.
#[test]
fn test_a_load_only_a_comparison_reads_is_its_operand() {
    let compared = |load: &str, test: &str| {
        inner(&format!(
            "define i16 @f(ptr %p) addrspace(1) {{\n  {load}\n  %c = icmp {test}\n  br i1 %c, label %y, label %n\ny:\n  ret i16 1\nn:\n  ret i16 0\n}}\n"
        ))[..3]
            .to_vec()
    };
    assert_eq!(compared("%v = load i16, ptr %p", "sgt i16 %v, 5"), ["mov bx, word ptr [bp+6]", "cmp word ptr [bx], 5", "jg L0_3"]);
    assert_eq!(
        compared("%v = load i8, ptr %p\n  %w = zext i8 %v to i16", "eq i16 %w, 0"),
        ["mov bx, word ptr [bp+6]", "cmp byte ptr [bx], 0", "je L0_4"]
    );
}

/// A byte compared as a signed word is never negative; as a byte it could
/// be: its load stays a zero extension.
#[test]
fn test_a_zero_extended_byte_compared_signed_keeps_its_extension() {
    let text = "define i16 @f(ptr %p) addrspace(1) {
  %v = load i8, ptr %p
  %w = zext i8 %v to i16
  %c = icmp sgt i16 %w, 0
  br i1 %c, label %y, label %n
y:
  ret i16 1
n:
  ret i16 0
}
";
    assert!(inner(text).iter().all(|one| !one.starts_with("cmp byte ptr")), "{:?}", inner(text));
}

/// A load read again is not a comparison's operand.
#[test]
fn test_a_load_read_again_is_compared_from_its_register() {
    let text = "define i16 @f(ptr %p) addrspace(1) {
  %v = load i16, ptr %p
  %c = icmp sgt i16 %v, 5
  br i1 %c, label %y, label %n
y:
  ret i16 %v
n:
  ret i16 0
}
";
    assert_eq!(inner(text)[..3], ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bx]", "cmp ax, 5"]);
}

/// A parameter pushed once is pushed from its frame cell, a constant as an
/// immediate; one pushed twice is loaded once.
#[test]
fn test_an_argument_is_pushed_from_its_cell_only_when_read_once() {
    let once = "declare void @take(i16, i16)
define void @f(i16 %a) addrspace(1) {
  call void @take(i16 %a, i16 7)
  ret void
}
";
    assert_eq!(inner(once), ["pushw 7", "push word ptr [bp+6]", "call take", "add sp, 4"]);
    let twice = "declare void @take(i16, i16)
define void @f(i16 %a) addrspace(1) {
  call void @take(i16 %a, i16 %a)
  ret void
}
";
    assert_eq!(inner(twice), ["mov ax, word ptr [bp+6]", "push ax", "push ax", "call take", "add sp, 4"]);
}

/// A far pointer loaded through a register is one `les`, as farload
/// selects, unless its selector is also read as a number.
#[test]
fn test_a_far_pointer_loaded_through_a_register_is_les() {
    let text = "define i16 @f(ptr %p) addrspace(1) {
  %q = load ptr addrspace(1), ptr %p
  %v = load i16, ptr addrspace(1) %q
  ret i16 %v
}
";
    assert_eq!(inner(text), ["mov bx, word ptr [bp+6]", "les bx, dword ptr [bx]", "mov ax, word ptr es:[bx]"]);
    let numeric = "define i32 @f(ptr %p) addrspace(1) {
  %q = load ptr addrspace(1), ptr %p
  %v = load i16, ptr addrspace(1) %q
  %i = ptrtoint ptr addrspace(1) %q to i32
  %w = zext i16 %v to i32
  %r = add i32 %i, %w
  ret i32 %r
}
";
    assert!(inner(numeric).iter().all(|one| !one.starts_with("les")), "{:?}", inner(numeric));
}

/// An address only accesses read is `[base+index]`, as the old route's
/// addressforms folds it: shellsort formed `base + (i << 1)` in a third
/// register, and C shellsort `lea bx,[bp-132]` in every array block.
#[test]
fn test_an_address_only_accesses_read_is_base_plus_index() {
    let based = "define void @f(ptr %p, i16 %i) addrspace(1) {
  %q = getelementptr inbounds i16, ptr %p, i16 %i
  %v = load i16, ptr %q
  %w = add i16 %v, 1
  %r = getelementptr inbounds i8, ptr %q, i16 2
  store i16 %w, ptr %r
  ret void
}
";
    assert_eq!(
        inner(based),
        ["mov bx, word ptr [bp+6]", "mov si, word ptr [bp+8]", "add si, si", "mov ax, word ptr [bx+si]", "inc ax", "mov word ptr [bx+si+2], ax"]
    );
    let local = "define i16 @f(i16 %i) addrspace(1) {
  %a = alloca [8 x i16]
  %q = getelementptr inbounds [8 x i16], ptr %a, i16 0, i16 %i
  store i16 3, ptr %q
  %v = load i16, ptr %q
  ret i16 %v
}
";
    assert_eq!(inner(local), ["mov si, word ptr [bp+6]", "add si, si", "mov word ptr ss:[bp+si-16], 3", "mov ax, word ptr ss:[bp+si-16]"]);
}

/// An array read only past its start by index is still reserved whole:
/// the indexed cell names no frame slot, and the frame put nothing below
/// `[bp-6]` for the element `[bp-8]` it could not see.
#[test]
fn test_an_array_read_only_by_index_is_reserved_whole() {
    let text = "define i16 @f(i16 %i) addrspace(1) {
  %a = alloca [4 x i16]
  %q = getelementptr inbounds [4 x i16], ptr %a, i16 0, i16 %i
  %r = getelementptr inbounds i8, ptr %q, i16 2
  %v = load i16, ptr %r
  ret i16 %v
}
";
    assert!(listing(text, "f").contains(&"sub sp, 8".to_owned()), "{:?}", listing(text, "f"));
}

/// An address also read as a value -- stored, or passed -- is computed.
#[test]
fn test_an_address_read_as_a_value_is_computed() {
    let text = "declare void @take(ptr)
define void @f(ptr %p, i16 %i) addrspace(1) {
  %q = getelementptr inbounds i16, ptr %p, i16 %i
  store i16 0, ptr %q
  call void @take(ptr %q)
  ret void
}
";
    assert_eq!(
        inner(text),
        ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bp+8]", "add ax, ax", "add bx, ax", "mov word ptr [bx], 0", "push bx", "call take", "add sp, 2"]
    );
}

/// A block laid out before the one defining the address it reads is
/// selected after it: selected in layout order, T059 read the folded
/// address's register, which nothing defined.
#[test]
fn test_an_address_folds_into_a_reader_laid_out_before_it() {
    let text = "define i16 @f(ptr %p, i16 %i) addrspace(1) {
entry:
  br label %def
use:
  %v = load i16, ptr %q
  ret i16 %v
def:
  %q = getelementptr inbounds i16, ptr %p, i16 %i
  br label %use
}
";
    assert!(listing(text, "f").contains(&"mov ax, word ptr [bx+si]".to_owned()), "{:?}", listing(text, "f"));
}

/// A global element's address a phi reads is computed where it is made:
/// folded into accesses, the phi read a register nothing defined.
#[test]
fn test_a_global_elements_address_a_phi_reads_is_computed() {
    let text = "@a = internal global [8 x i16] zeroinitializer
define i16 @f(i16 %i, i1 %c) addrspace(1) {
entry:
  %q = getelementptr inbounds [8 x i16], ptr @a, i16 0, i16 %i
  br i1 %c, label %one, label %two
one:
  br label %join
two:
  br label %join
join:
  %r = phi ptr [ %q, %one ], [ @a, %two ]
  %v = load i16, ptr %r
  ret i16 %v
}
";
    assert_eq!(listing(text, "f")[3..7], ["mov bx, word ptr [bp+6]", "mov al, byte ptr [bp+8]", "add bx, bx", "add bx, offset a"]);
}

/// A signed dword divided by a constant is a multiply by its reciprocal
/// where the target prices that cheaper, as the old route's
/// division::reciprocal selects: P5's imul is cheap, the 386's is not.
#[test]
fn test_a_dword_divided_by_a_constant_is_multiplied_where_cheaper() {
    let text = "define i32 @f(i32 %x) addrspace(1) {
  %q = sdiv i32 %x, 10
  %r = srem i32 %x, 10
  %s = add i32 %q, %r
  ret i32 %s
}
";
    let divides = |cpu: &str| inner_on(cpu, text).iter().filter(|one| one.starts_with("idiv")).count();
    assert_eq!((divides("P5"), divides("386")), (0, 1));
}

/// A word, and an unsigned dword, divided by a constant stay divisions:
/// the reciprocal is the old route's for signed dwords only.
#[test]
fn test_a_word_or_unsigned_division_by_a_constant_divides() {
    let word = "define i16 @f(i16 %x) addrspace(1) {\n  %q = sdiv i16 %x, 10\n  ret i16 %q\n}\n";
    let unsigned = "define i32 @f(i32 %x) addrspace(1) {\n  %q = udiv i32 %x, 10\n  ret i32 %q\n}\n";
    assert!(inner_on("P5", word).contains(&"idiv bx".to_owned()), "{:?}", inner_on("P5", word));
    assert!(inner_on("P5", unsigned).contains(&"div ebx".to_owned()), "{:?}", inner_on("P5", unsigned));
}

/// A multiply by a constant is shifts and adds where the target prices
/// them below imul, as the old route's _scaled selects.
#[test]
fn test_a_multiply_by_a_constant_is_shifts_and_adds() {
    let by = |factor: i16| inner(&format!("define i16 @f(i16 %x) addrspace(1) {{\n  %q = mul i16 %x, {factor}\n  ret i16 %q\n}}\n"));
    assert_eq!(by(10), ["mov bx, word ptr [bp+6]", "lea ax, [ebx+ebx*4]", "add ax, ax"]);
    assert_eq!(by(7), ["mov ax, word ptr [bp+6]", "mov bx, ax", "shl bx, 3", "sub bx, ax", "mov ax, bx"]);
    let variable = "define i16 @f(i16 %x, i16 %y) addrspace(1) {\n  %q = mul i16 %x, %y\n  ret i16 %q\n}\n";
    assert_eq!(inner(variable), ["mov ax, word ptr [bp+6]", "mov bx, word ptr [bp+8]", "imul ax, bx"]);
}

/// A dword load read only as its words is those words loaded, as the old
/// route's narrow selects: the high word cost a copy and a shift.
#[test]
fn test_a_dword_read_only_as_words_loads_the_words() {
    let text = "define i16 @f(ptr %p) addrspace(1) {
  %v = load i32, ptr %p
  %h = lshr i32 %v, 16
  %w = trunc i32 %h to i16
  %l = trunc i32 %v to i16
  %s = add i16 %w, %l
  ret i16 %s
}
";
    assert_eq!(inner(text), ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bx+2]", "add ax, word ptr [bx]"]);
}

/// A dword also read whole, or loaded volatile, is loaded whole.
#[test]
fn test_a_dword_read_whole_or_volatile_is_loaded_whole() {
    let whole = "define i32 @f(ptr %p) addrspace(1) {
  %v = load i32, ptr %p
  %l = trunc i32 %v to i16
  %x = zext i16 %l to i32
  %s = add i32 %v, %x
  ret i32 %s
}
";
    assert!(inner(whole).iter().any(|one| one.ends_with("dword ptr [bx]")), "{:?}", inner(whole));
    let volatile = "define i16 @f(ptr %p) addrspace(1) {
  %v = load volatile i32, ptr %p
  %l = trunc i32 %v to i16
  ret i16 %l
}
";
    assert!(inner(volatile).iter().any(|one| one.ends_with("dword ptr [bx]")), "{:?}", inner(volatile));
}

/// An AND only a comparison with zero reads is `test`, as the old route's
/// _flag_test selects: its result needs no register.
#[test]
fn test_an_and_only_compared_with_zero_is_test() {
    let tested = |mask: &str, ret: &str| {
        inner(&format!(
            "define i16 @f(i16 %x, i16 %m) addrspace(1) {{\n  %a = and i16 %x, {mask}\n  %c = icmp eq i16 %a, 0\n  br i1 %c, label %y, label %n\ny:\n  ret i16 1\nn:\n  ret i16 {ret}\n}}\n"
        ))
    };
    assert_eq!(tested("%m", "0")[..4], ["mov ax, word ptr [bp+6]", "mov bx, word ptr [bp+8]", "test ax, bx", "je L0_3"]);
    // Read again, the AND is computed; a constant mask is the old route's AND too.
    assert!(tested("%m", "%a").iter().any(|one| one.starts_with("and ")), "{:?}", tested("%m", "%a"));
    assert!(tested("12", "0").iter().any(|one| one.starts_with("and ")), "{:?}", tested("12", "0"));
}

/// A block every path from which ends in `unreachable` is cold, laid out
/// after the hot code, as the old route's noreturn::cold marks a failed
/// bounds check's: it sat between the loop and its exit.
#[test]
fn test_a_block_that_cannot_return_is_laid_out_last() {
    let text = "declare void @panic() addrspace(1)
define i16 @f(i16 %i, i16 %n) addrspace(1) {
entry:
  br label %loop
loop:
  %k = phi i16 [ 0, %entry ], [ %k1, %fine ]
  %ok = icmp ult i16 %k, %n
  br i1 %ok, label %fine, label %fail
fail:
  call addrspace(1) void @panic()
  unreachable
fine:
  %k1 = add i16 %k, 1
  %d = icmp eq i16 %k1, %i
  br i1 %d, label %done, label %loop
done:
  ret i16 %k1
}
";
    assert_eq!(listing(text, "f").last().map(String::as_str), Some("call far ptr panic"));
}

/// A quotient and remainder of the same operands are one division, as the
/// old route's divmod: C's `q = x / y; r = x % y` divided twice.
#[test]
fn test_a_quotient_and_remainder_are_one_division() {
    let text = "define i16 @f(i16 %x, i16 %y) addrspace(1) {
  %q = sdiv i16 %x, %y
  %r = srem i16 %x, %y
  %s = add i16 %q, %r
  ret i16 %s
}
";
    assert_eq!(inner(text).iter().filter(|one| one.starts_with("idiv")).count(), 1, "{:?}", inner(text));
    let reciprocal = "define i32 @f(i32 %x) addrspace(1) {
  %q = sdiv i32 %x, 10
  %r = srem i32 %x, 10
  %s = add i32 %q, %r
  ret i32 %s
}
";
    assert_eq!(inner_on("P5", reciprocal).iter().filter(|one| one.starts_with("imul")).count(), 1, "{:?}", inner_on("P5", reciprocal));
    let different = "define i16 @f(i16 %x, i16 %y) addrspace(1) {
  %q = sdiv i16 %x, %y
  %r = srem i16 %y, %x
  %s = add i16 %q, %r
  ret i16 %s
}
";
    assert_eq!(inner(different).iter().filter(|one| one.starts_with("idiv")).count(), 2, "{:?}", inner(different));
}

/// A float load only a comparison reads second is `fcom`'s operand, as the
/// old route compared a cell: it was loaded, exchanged and popped with the
/// other.
#[test]
fn test_a_float_load_compared_second_is_the_comparisons_operand() {
    let compared = |test: &str| {
        inner(&format!(
            "define i16 @f(double %x, ptr %p) addrspace(1) {{\n  %w = load double, ptr %p\n  %c = fcmp {test}\n  br i1 %c, label %y, label %n\ny:\n  ret i16 1\nn:\n  ret i16 0\n}}\n"
        ))
    };
    assert_eq!(compared("ogt double %x, %w")[..3], ["fld qword ptr [bp+6]", "mov bx, word ptr [bp+14]", "fcomp qword ptr [bx]"]);
    // Compared first, the cell would have to be st(0): it is loaded.
    assert!(compared("olt double %x, %w").contains(&"fld qword ptr [bx]".to_owned()), "{:?}", compared("olt double %x, %w"));
}

/// A comparison a branch reads is made beside the branch: a store between
/// them may change the cell, so the load stays where it was.
#[test]
fn test_a_float_load_before_a_store_is_not_compared_after_it() {
    let text = "define i16 @f(double %x, ptr %p) addrspace(1) {
  %w = load double, ptr %p
  %c = fcmp ogt double %x, %w
  store double 0.0, ptr %p
  br i1 %c, label %y, label %n
y:
  ret i16 1
n:
  ret i16 0
}
";
    let got = inner(text);
    let (load, store) = (got.iter().position(|one| one == "fld qword ptr [bx]"), got.iter().position(|one| one.starts_with("mov dword ptr [bx]")));
    assert!(load.is_some() && load < store, "{got:?}");
}

/// A cell whose offset `exact_offsets` proves is `Mem::exact`, so
/// `exactaddress` folds its `add si,si` into the address: unmarked, the
/// isel route kept every such chain the old route folded.
#[test]
fn test_an_exact_elements_offset_is_folded_into_a_scaled_address() {
    let sum = |inbounds: &str| {
        format!(
            "@a = global [8 x i16] zeroinitializer

define i16 @f() addrspace(1) {{
entry:
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %e = getelementptr {inbounds} [8 x i16], ptr @a, i16 0, i16 %i
  %v = load i16, ptr %e
  %t = add i16 %s, %v
  %j = add i16 %i, 1
  %more = icmp ult i16 %j, 8
  br i1 %more, label %body, label %done
done:
  ret i16 %t
}}
"
        )
    };
    let element = |text: &str| inner(text).into_iter().find(|line| line.contains("a[")).expect("the element's read");
    assert_eq!(element(&sum("inbounds")), "add ax, word ptr a[esi+esi]");
    // Without `inbounds` nothing places the start: the offset may wrap.
    assert_eq!(element(&sum("")), "add ax, word ptr a[si]");
}

/// A word product only cells read is the 67h form's scaled index on the
/// 386, as the old route's addressforms makes it: `[ebx+esi*2]`, the index
/// and the base widened where they are loaded.
#[test]
fn test_a_non_negative_typed_index_is_scaled_in_the_67h_form() {
    let text = |guard: &str| {
        format!(
            "define i16 @f(ptr %p, ptr %q) addrspace(1) {{
entry:
  %i = load i16, ptr %q
  %b = load ptr, ptr %p
  %c = icmp {guard} i16 %i, 0
  br i1 %c, label %ok, label %no
ok:
  %e = getelementptr inbounds i16, ptr %b, i16 %i
  %v = load i16, ptr %e, !tbaa !1
  ret i16 %v
no:
  ret i16 0
}}

!0 = !{{!\"int\"}}
!1 = !{{!0, !0, i64 0}}
"
        )
    };
    let got = listing_on("386", &text("sge"), "f");
    assert_eq!(got[6..8], ["movzx eax, word ptr [si]", "movzx ebx, word ptr [bx]"], "{got:?}");
    assert!(got.contains(&"mov ax, word ptr [ebx+eax*2]".to_owned()), "{got:?}");
    // A negative index names another byte 32 bits wide; the 486 prices
    // the form above a spill.
    for (cpu, guard) in [("386", "ne"), ("486", "sge")] {
        let got = listing_on(cpu, &text(guard), "f");
        assert!(!got.iter().any(|line| line.contains("movzx") || line.contains("*2")), "{cpu} {guard}: {got:?}");
    }
}

/// A counted loop's trips reach the machine phases: without them the
/// isel route's bodies had none, and `executed` guessed nine in ten.
#[test]
fn test_a_counted_loops_trips_are_the_bodys() {
    let text = "define i16 @f() addrspace(1) {
entry:
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %t = add i16 %s, %i
  %j = add i16 %i, 1
  %more = icmp ult i16 %j, 5
  br i1 %more, label %body, label %done
done:
  ret i16 %t
}
";
    let body = selected(text, "f").expect("selects").body;
    let header = body.blocks[1].at;
    assert_eq!(body.loop_trip_counts, [(header, 5)]);
}

/// A wide half nothing reads is not made: matmul8's `x * 256 / 1024`
/// kept `sar edx,10`, the quotient's high dword its truncation dropped,
/// through every machine phase.
#[test]
fn test_an_i64_halfs_unread_computation_is_not_made() {
    let text = "define i32 @f(i32 %x) addrspace(1) {
  %w = sext i32 %x to i64
  %s = shl i64 %w, 8
  %q = sdiv i64 %s, 1024
  %t = trunc i64 %q to i32
  ret i32 %t
}
";
    let body = selected(text, "f").expect("selects").body;
    let names: Vec<&str> = body.blocks.iter().flat_map(|block| &block.insns).filter_map(|one| one.what.as_ref()?.name.as_deref()).collect();
    // The quotient's low dword: the bias's add and its carry, and shrd.
    let low = names.iter().position(|name| *name == "shrd").expect("the low dword's shift");
    assert_eq!(names[low - 2..low], ["add", "adc"], "{names:?}");
    // One `sar`, the dividend's sign; none for the quotient's high dword.
    assert_eq!(names.iter().filter(|name| **name == "sar").count(), 1, "{names:?}");
}

/// isel asks whether a call between a load and its only reader writes
/// memory, as the callee's attributes state; only llrm-mir's function-attrs
/// stated them of a defined body, so a load across a call to one that
/// writes nothing was not `fild`'s cell. The whole-module stamp states them.
#[test]
fn a_load_is_folded_across_a_call_to_a_stamped_body_that_writes_nothing() {
    let text = "@g = global i16 0
@h = global i16 0

define internal i16 @reads() addrspace(1) {
  %v = load i16, ptr @h
  ret i16 %v
}

define double @f() addrspace(1) {
  %v = load i16, ptr @g
  %r = call addrspace(1) i16 @reads()
  %d = sitofp i16 %v to double
  ret double %d
}
";
    let mut module = parsed(text);
    let mut analyses = llrm_mir::passes::ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_mir::target::Neutral));
    llrm_transforms::interprocedural::stamped(&mut module, &mut analyses).unwrap();
    let stamped = llrm_mir::print::module(&module);
    let stamped = stamped.lines().filter(|line| !line.starts_with("target datalayout")).collect::<Vec<_>>().join("\n");
    assert!(listing(&stamped, "f").iter().any(|line| line == "fild word ptr g"), "{stamped}");
    assert!(!listing(text, "f").iter().any(|line| line == "fild word ptr g"));
}

/// A far pointer a loop steps is a phi of its offset and one of its
/// selector, as the old route split it into two words: strength made the
/// runtime's `buffers.copy` step one, and isel refused every program.
#[test]
fn test_a_far_pointer_a_loop_steps_is_two_phis() {
    let text = "define void @copy(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) {
b1:
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %done, label %loop
loop:
  %far = phi ptr addrspace(1) [ %far.next, %loop ], [ %1, %b1 ]
  %near = phi ptr [ %near.next, %loop ], [ %0, %b1 ]
  %i = phi i16 [ %j, %loop ], [ %2, %b1 ]
  %v = load i8, ptr addrspace(1) %far
  store i8 %v, ptr %near
  %j = sub i16 %i, 1
  %more = icmp ne i16 %j, 0
  %near.next = getelementptr i8, ptr %near, i16 1
  %far.next = getelementptr i8, ptr addrspace(1) %far, i16 1
  br i1 %more, label %loop, label %done
done:
  ret void
}
";
    let got = listing(text, "copy");
    let body = got.iter().position(|line| line == "L0_2:").expect("the loop");
    assert_eq!(got[body + 1..body + 7], ["mov cl, byte ptr es:[si]", "mov byte ptr [bx], cl", "inc bx", "inc si", "dec ax", "jne L0_2"], "{got:?}");
}

/// An i64 converted to a float is `fild qword` of its pair stored, as the
/// old route's C `_wide` did; isel refused "SIToFP from an i64".
#[test]
fn test_an_i64_to_a_float_is_filds_qword() {
    let text = "define float @f(i32 %x) addrspace(1) {
  %w = zext i32 %x to i64
  %f = sitofp i64 %w to float
  ret float %f
}
";
    let got = inner(text);
    let fild = got.iter().position(|line| line.starts_with("fild qword ptr [bp-8]")).expect("fild qword");
    assert_eq!(got[..fild], ["mov eax, dword ptr [bp+6]", "mov ebx, 0", "mov dword ptr [bp-8], eax", "mov dword ptr [bp-4], ebx"], "{got:?}");
}

/// A far global is `seg name:offset name`, and a memset of a variable
/// length `rep stosd` then `rep stosb` through it, as the old route made
/// C's `fill_far`; isel refused "a far global", then the variable length.
#[test]
fn test_a_far_globals_variable_memset_is_a_string_fill_through_its_segment() {
    let text = "@g = internal addrspace(1) global [64 x i8] zeroinitializer
declare void @llvm.memset.p1.i16(ptr addrspace(1), i8, i16, i1)

define void @f(i16 %n) addrspace(1) {
  call void @llvm.memset.p1.i16(ptr addrspace(1) @g, i8 0, i16 %n, i1 false)
  ret void
}
";
    let got = listing(text, "f");
    let from = got.iter().position(|line| line == "L0_0:").expect("the body") + 1;
    assert_eq!(
        got[from..from + 11],
        [
            "mov bx, word ptr [bp+6]",
            "mov cx, bx",
            "shr cx, 2",
            "and bx, 3",
            "mov di, offset g",
            "pushw seg g",
            "pop es",
            "mov eax, 0",
            "rep stosd",
            "mov cx, bx",
            "rep stosb",
        ]
    );
}

/// A far global's element is read through its segment and offset, as the
/// old route made C's `peek`.
#[test]
fn test_a_far_globals_element_is_read_through_its_segment() {
    let text = "@s = internal addrspace(1) global [16 x i8] zeroinitializer

define i16 @f(i16 %i) addrspace(1) {
  %o = shl i16 %i, 1
  %e = getelementptr inbounds i8, ptr addrspace(1) @s, i16 %o
  %v = load i16, ptr addrspace(1) %e
  ret i16 %v
}
";
    let got = listing(text, "f");
    assert_eq!(got[4..10], ["mov bx, word ptr [bp+6]", "add bx, bx", "pushw seg s", "pop es", "mov si, offset s", "mov ax, word ptr es:[bx+si]"], "{got:?}");
}

/// An i64 is a pair of dwords wherever it goes: a parameter's two cells,
/// a phi per dword, edx:eax out of a call and a return, and a comparison's
/// halves or-ed. isel refused "a i64 value"; C's fib64 kept both
/// Fibonacci terms in registers as the old route did.
#[test]
fn test_an_i64_is_a_dword_pair_through_phis_calls_and_returns() {
    let text = "define i64 @f(i64 %x, i16 %n) addrspace(1) {
entry:
  br label %loop
loop:
  %a = phi i64 [ 0, %entry ], [ %b, %loop ]
  %b = phi i64 [ %x, %entry ], [ %c, %loop ]
  %i = phi i16 [ %n, %entry ], [ %j, %loop ]
  %c = add i64 %a, %b
  %j = sub i16 %i, 1
  %more = icmp ne i16 %j, 0
  br i1 %more, label %loop, label %done
done:
  ret i64 %b
}

define i16 @g() addrspace(1) {
  %v = call addrspace(1) i64 @f(i64 1, i16 9)
  %d = icmp ne i64 %v, 34
  %r = zext i1 %d to i16
  ret i16 %r
}
";
    let f = listing(text, "f");
    let add = f.iter().position(|line| line.starts_with("add e")).expect("the low dwords' sum");
    assert!(f[add + 1].starts_with("adc e") && !f.iter().any(|line| line.contains("[bp-")), "{f:?}");
    let g = listing(text, "g");
    for line in ["pushd 0", "pushd 1", "call far ptr f", "xor eax, 34", "or eax, edx"] {
        assert!(g.iter().any(|one| one == line), "{line}: {g:?}");
    }
}

/// An i64 remainder's high dword is its low's sign: taken as the
/// dividend's, a zero remainder of a negative dividend was -2^32, and C's
/// euclid64 crunch went wrong on its third round.
#[test]
fn test_an_i64_remainders_high_dword_is_its_low_dwords_sign() {
    let text = "define i64 @f(i64 %x) addrspace(1) {
  %r = srem i64 %x, 7
  ret i64 %r
}
";
    let got = listing(text, "f");
    let tail = &got[got.len() - 6..];
    assert!(tail.windows(2).any(|two| two[0].starts_with("mov edx, e") && two[1] == "sar edx, 31") || tail.contains(&"cdq".to_owned()), "{got:?}");
}

/// An i64 divided by a variable is the old route's inline helper,
/// edx:eax by ecx:ebx laid down in place of a call; isel refused "an
/// i64 urem", and C's gcd64 with it. A quotient and remainder of the same
/// operands share one.
#[test]
fn test_an_i64_divided_by_a_variable_is_the_inline_helper() {
    let text = "define i64 @f(i64 %x, i64 %y) addrspace(1) {
  %q = udiv i64 %x, %y
  %r = urem i64 %x, %y
  %s = add i64 %q, %r
  ret i64 %s
}
";
    let got = listing(text, "f");
    let helper = got.iter().filter(|line| line.starts_with("db 066h,009h,0c9h,075h,02ah")).count();
    assert_eq!(helper, 1, "{got:?}");
    assert!(got.iter().any(|line| line == "add eax, ebx") && got.iter().any(|line| line == "adc edx, ecx"), "{got:?}");
}

/// Adjacent argument words forwarded from memory are dword pushes: pushed a
/// word at a time, os.write's forwarded far buffer cost one more memory
/// operand per call than the old route's.
#[test]
fn test_a_far_pointer_argument_from_memory_is_one_push() {
    let text = "declare void @g(ptr addrspace(1), i16) addrspace(1)
define void @f(ptr addrspace(1) %p, i16 %n) addrspace(1) {
  call addrspace(1) void @g(ptr addrspace(1) %p, i16 %n)
  ret void
}
";
    let got = listing(text, "f");
    let pushes: Vec<&String> = got.iter().filter(|line| line.starts_with("push") && line.contains("[bp+")).collect();
    assert_eq!(pushes.len(), 2, "{got:?}");
}

/// An i64 divided by a sign-extended i32 whose quotient fits a dword is one
/// idiv: divided as magnitudes it took two divs and sign fixups, nbody's
/// 262144 / d costing 504 more instructions than the old route's.
#[test]
fn test_a_quotient_that_fits_a_dword_is_one_idiv() {
    let text = "define i64 @f(i32 %d) addrspace(1) {
  %w = sext i32 %d to i64
  %q = sdiv i64 262144, %w
  ret i64 %q
}
";
    let got = listing(text, "f");
    let divides: Vec<&String> = got.iter().filter(|line| line.starts_with("div") || line.starts_with("idiv")).collect();
    assert_eq!(divides.len(), 1, "{got:?}");
    assert!(divides[0].starts_with("idiv"), "{got:?}");
}

/// A parameter needed after a call is loaded again from its argument slot,
/// which nothing in the IR can address: spilled across the call instead,
/// format.field stored and reloaded its three argument words every call.
#[test]
fn test_a_parameter_is_reloaded_from_its_slot_across_a_call() {
    let text = "declare void @g() addrspace(1)
declare void @h(i16) addrspace(1)
define void @f(i16 %a) addrspace(1) {
  %b = add i16 %a, 1
  call addrspace(1) void @g()
  call addrspace(1) void @h(i16 %a)
  call addrspace(1) void @h(i16 %b)
  ret void
}
";
    let got = listing(text, "f");
    // Only %b, computed before the call, needs a slot.
    assert_eq!(got.iter().filter(|line| line.starts_with("mov word ptr [bp-")).count(), 1, "{got:?}");
}

/// A far pointer loaded from memory only to be passed on is one dword push:
/// loaded as two words, format.put_text read one more memory operand per
/// call than the old route's dword.
#[test]
fn test_a_far_pointer_loaded_to_be_passed_is_one_push() {
    let text = "declare void @g(ptr addrspace(1), i16) addrspace(1)
define void @f(ptr addrspace(1) %t) addrspace(1) {
  %at = getelementptr i8, ptr addrspace(1) %t, i16 4
  %p = load ptr addrspace(1), ptr addrspace(1) %at
  %n = load i16, ptr addrspace(1) %t
  call addrspace(1) void @g(ptr addrspace(1) %p, i16 %n)
  ret void
}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.starts_with("push dword ptr es:[")), "{got:?}");
}

/// A dword read as two words and pushed whole in several places is one
/// dword load: format.put read its far buffer's two words and pushed them
/// apart on both paths, one memory operand and one push more than the old
/// route's dword per call.
#[test]
fn test_a_far_pointer_pushed_whole_on_two_paths_is_one_dword() {
    let text = "@sink = internal global ptr null
declare void @w(ptr addrspace(1), i16) addrspace(1)
declare ptr @a(ptr, ptr addrspace(1), i16) addrspace(1)
define void @f(ptr addrspace(1) %p, i16 %n) addrspace(1) {
  %s = load ptr, ptr @sink
  %z = icmp eq ptr %s, null
  br i1 %z, label %one, label %two
one:
  call addrspace(1) void @w(ptr addrspace(1) %p, i16 %n)
  ret void
two:
  %r = call addrspace(1) ptr @a(ptr %s, ptr addrspace(1) %p, i16 %n)
  store ptr %r, ptr @sink
  ret void
}
";
    let got = listing(text, "f");
    // Three argument words in two reads, pushed as two on each path.
    assert_eq!(got.iter().filter(|line| line.contains("[bp+")).count(), 2, "{got:?}");
    assert_eq!(got.iter().filter(|line| line.starts_with("push") && *line != "push bp").count(), 5, "{got:?}");
}

/// Nib's fixed point as `llvm.smul.fix` and `llvm.sdiv.fix`: the product
/// one widening `imul` shifted down, as the old route's FixedMul; the
/// quotient by a whole divisor one `idiv` of the dividend shifted up, as
/// its FixedDiv. isel refused both calls.
#[test]
fn test_fixed_point_intrinsics_take_imul_and_idiv() {
    let text = "declare i32 @llvm.smul.fix.i32(i32, i32, i32)
declare i32 @llvm.sdiv.fix.i32(i32, i32, i32)

define i32 @scaled(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = call i32 @llvm.smul.fix.i32(i32 %0, i32 %1, i32 16)
  %3 = call i32 @llvm.sdiv.fix.i32(i32 %2, i32 196608, i32 16)
  ret i32 %3
}
";
    assert_eq!(
        listing(text, "scaled"),
        [
            "push bp", "mov bp, sp", "L0_0:", "mov eax, dword ptr [bp+6]", "mov ebx, dword ptr [bp+10]", "imul ebx", "mov ebx, eax", "shrd ebx, edx, 16",
            "mov ecx, 196608", "mov eax, ebx", "cdq", "shld edx, ebx, 16", "shl eax, 16", "idiv ecx", "shld edx, eax, 16", "pop bp", "retf",
        ]
    );
}

/// A parameter passed in the frame's own bytes is refused: its slot is
/// addressable, so selected it would break `sealed_arguments`, and a call
/// passing one would push the pointer where the callee expects the bytes.
#[test]
fn test_parameters_passed_in_the_frame_are_refused() {
    for attribute in ["byval(i32)", "inalloca(i32)", "byref(i32)"] {
        let text = format!("define i16 @f(ptr {attribute} %p) addrspace(1) {{\n  %v = load i16, ptr %p\n  ret i16 %v\n}}\n");
        assert!(selected(&text, "f").is_err(), "{attribute}");
        let text = format!("declare void @g(ptr) addrspace(1)\ndefine void @f(ptr %p) addrspace(1) {{\n  call addrspace(1) void @g(ptr {attribute} %p)\n  ret void\n}}\n");
        assert!(selected(&text, "f").is_err(), "a call's {attribute}");
    }
}

/// A call writes what its MIR says it may: a double loaded from a cell the
/// callee is handed is not read again after the call, which may change it.
/// isel's calls listed nothing, and floatassign read the cell again.
#[test]
fn test_a_float_in_a_cell_a_call_may_write_is_not_read_again_after_it() {
    let text = "@out = internal global double 0.0
declare void @g(ptr) addrspace(1)
define void @f(double %a) addrspace(1) {
  %x = alloca double
  store double %a, ptr %x
  %v = load double, ptr %x
  call addrspace(1) void @g(ptr %x)
  %w = fadd double %v, %v
  store double %w, ptr @out
  ret void
}
";
    let got = listing(text, "f");
    let call = got.iter().position(|line| line.starts_with("call")).expect("the call");
    assert!(!got[call..].iter().any(|line| line.starts_with("fld") && line.contains("[bp-8]")), "{got:?}");
    // A cell no call can reach is still read again rather than saved.
    let private = text.replace("declare void @g(ptr)", "declare void @g()").replace("@g(ptr %x)", "@g()");
    let got = listing(&private, "f");
    let call = got.iter().position(|line| line.starts_with("call")).expect("the call");
    assert!(got[call..].iter().any(|line| line.starts_with("fld") && line.contains("[bp-8]")), "{got:?}");
}

/// A store of poison stores nothing, as LLVM's DAGCombiner drops it: the
/// memory may then hold anything. isel refused it as "an address of no
/// global".
#[test]
fn test_a_store_of_poison_stores_nothing() {
    let text = "@g = internal global i16 0\ndefine void @f() addrspace(1) {\n  store i16 poison, ptr @g\n  ret void\n}\n";
    assert!(!listing(text, "f").iter().any(|line| line.starts_with("mov")));
}

/// A dword that arrived as two words leaves as those words: a call's dx:ax
/// result returned as is was joined into one register and split back.
#[test]
fn test_a_dword_returned_as_it_arrived_is_not_joined() {
    let text = "declare i32 @g() addrspace(1)
define i32 @f() addrspace(1) {
  %r = call addrspace(1) i32 @g()
  ret i32 %r
}
";
    assert_eq!(listing(text, "f"), ["L0_0:", "call far ptr g", "retf"]);
}

/// A word truncated from a dword that arrived as two words is its low word:
/// struct_view's main joined its call's dx:ax result to return ax.
#[test]
fn test_a_word_truncated_from_a_joined_dword_is_its_low_word() {
    let text = "declare i32 @g() addrspace(1)
define i16 @f() addrspace(1) {
  %r = call addrspace(1) i32 @g()
  %t = trunc i32 %r to i16
  ret i16 %t
}
";
    assert_eq!(listing(text, "f"), ["L0_0:", "call far ptr g", "retf"]);
}

/// Zero minus a value is its negation: crc's `0 - (crc & 1)` cost a
/// zeroing `xor` and a `sub` in each of its eight unrolled steps.
#[test]
fn test_zero_minus_a_value_is_a_neg() {
    let text = "define i32 @f(i32 %a) addrspace(1) {
  %m = and i32 %a, 1
  %n = sub i32 0, %m
  ret i32 %n
}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.starts_with("neg")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("sub")), "{got:?}");
}

/// An address known at link time is stored as immediates, as the old route
/// stores it: lru's four far pointers each went through two registers.
#[test]
fn test_a_constant_address_is_stored_as_immediates() {
    let text = "@g = internal global i16 0
@near = internal global ptr null
@far = internal global ptr addrspace(1) null
define void @f() addrspace(1) {
  store ptr @g, ptr @near
  %a = addrspacecast ptr @g to ptr addrspace(1)
  store ptr addrspace(1) %a, ptr @far
  store ptr addrspace(1) null, ptr getelementptr (i8, ptr @far, i16 4)
  ret void
}
";
    let got = listing(text, "f");
    let moves: Vec<&String> = got.iter().filter(|line| line.starts_with("mov")).collect();
    assert!(moves.iter().all(|line| line.starts_with("mov word ptr") || line.starts_with("mov dword ptr")), "{got:?}");
}
