use crate::abi::qb::HirAbi;
use crate::backend::assemble::{self, Abi};
use crate::backend::constpool::Pool;
use crate::backend::cpu::ProfileOrName;
use crate::backend::isel::{self, Unselected};
use crate::backend::masm;

const LAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16\"\n";

fn qb() -> HirAbi {
    HirAbi { runtime: crate::hir::model::RuntimeProfile::Qb45, objects: Default::default(), preserved: Default::default(), stack_check: None }
}

fn parsed(text: &str) -> llrm_mir::Module {
    llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses")
}

fn selected(text: &str, name: &str) -> Result<isel::Selected, Unselected> {
    isel::selected(&parsed(text), name, &qb(), &mut Pool::new(0), crate::backend::cpu::profile("486").expect("a target"), &crate::backend::target::BASIC, isel::code16(), &llrm_x86_code16::Code16, false, 0)
}

/// The module's text, once its object is written: a listing that does not
/// encode is no listing.
fn assembled(text: &str) -> String {
    assembled_on("486", text)
}

/// Borland C's medium model: a call keeps all but ax, bx, cx, dx, es and the flags.
fn borland() -> HirAbi {
    use crate::abi::runtime::{EVERY, Reg};
    let clobbered = [Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Es, Reg::Flags];
    HirAbi { runtime: crate::hir::model::RuntimeProfile::Freestanding, objects: Default::default(), preserved: EVERY.iter().copied().filter(|one| !clobbered.contains(one)).collect(), stack_check: None }
}

/// The module's text as `cpu` prices it, for BASIC's runtime.
fn assembled_on(cpu: &str, text: &str) -> String {
    assembled_by(&qb(), &crate::backend::target::BASIC, cpu, text)
}

/// The module's text under `abi` and `segments`, as `cpu` prices it.
fn assembled_by(abi: &HirAbi, segments: &crate::backend::target::Segments, cpu: &str, text: &str) -> String {
    let module = assemble::assembled(&parsed(text), abi, "T_TEXT", ProfileOrName::Name(cpu), segments).expect("assembles");
    crate::backend::objbuild::written_as(&module, "t.asm", crate::backend::objbuild::CodeLayout::OneSegment).expect("encodes");
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
    // The comparison stays flags beside its branch; the phis' zeros are made in the entry,
    // which runs a copy of the test rather than jump to it.
    assert_eq!(
        got,
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "mov bx, word ptr [bp+6]",
            "xor ax, ax",
            "xor cx, cx",
            "cmp cx, bx",
            "jl L0_5",
            "jmp L0_8",
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
/// base plus it: `[bp+di-8]`, `[bx+si+2]`. The frame cell's SS is BP's own,
/// so no `ss:` override: it cost a byte and a clock at each access.
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
            "mov word ptr [bp+di-8], 5",
            "add si, si",
            "mov ax, word ptr [bx+si+2]",
            "pop di",
            "pop si",
            "leave",
            "retf",
        ]
    );
}

/// A constant added to an index is the address's displacement, not a
/// register: `a[i + 8]` of bytes was `mov di, si; add di, 8; [bx+di]`, an
/// add and a copy a trip that main's `[bx+si+8]` did not pay.
#[test]
fn test_a_constant_added_to_an_index_is_a_displacement() {
    let text = "define i16 @f(ptr %p, i16 %i) addrspace(1) {
  %j = add nsw i16 %i, 8
  %q = getelementptr inbounds i8, ptr %p, i16 %j
  %v = load i8, ptr %q
  %w = zext i8 %v to i16
  ret i16 %w
}
";
    let got = listing(text, "f").join("\n");
    assert!(got.contains("[bx+si+8]") && !got.contains("add si, 8") && !got.contains("add di, 8"), "{got}");
}

/// A far pointer of offset zero indexed by a sum of two registers is a base
/// and an index, `es:[bx+si]`: the sum was added into a third register
/// first, a `mov` and an `add` a trip in qbdemo's FRACTALEFFECT.
#[test]
fn test_a_sum_of_two_registers_is_the_base_and_index_of_a_far_address() {
    let text = "define i16 @f(i16 %sel, i16 %i, i16 %j) addrspace(1) {
  %seg = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %seg to ptr addrspace(1)
  %k = add i16 %i, %j
  %p = getelementptr inbounds i8, ptr addrspace(1) %far, i16 %k
  %v = load i8, ptr addrspace(1) %p
  %w = zext i8 %v to i16
  ret i16 %w
}
";
    let got = listing(text, "f").join("\n");
    assert!(got.contains("es:[bx+si]") || got.contains("es:[bx+di]") || got.contains("es:[si+bx]") || got.contains("es:[di+bx]"), "{got}");
    assert!(!got.lines().any(|line| line.starts_with("add ") && !line.contains("sp")), "{got}");
}

/// A constant `getelementptr` under two variable indexes is the access's
/// displacement, `[bx+si+6072]`. It was added into a register first, `add si,
/// 6072` a trip with its result live beside the others: x_walkcols_usescalem1
/// spilled the array's base for it (#243).
#[test]
fn test_a_constant_under_two_variable_indexes_is_the_displacement_of_the_access() {
    let text = "define i16 @f(ptr %p, i16 %i, i16 %j) addrspace(1) {
  %c = getelementptr i8, ptr %p, i16 6072
  %a = getelementptr i8, ptr %c, i16 %i
  %b = getelementptr i8, ptr %a, i16 %j
  %v = load i8, ptr %b
  %w = zext i8 %v to i16
  ret i16 %w
}
";
    let got = listing(text, "f").join("\n");
    assert!(got.contains("6072]"), "{got}");
    assert!(!got.lines().any(|line| line.starts_with("add ") && line.contains("6072")), "{got}");
}

/// Pointers are ordered where `icmp ult ptr` is lowered, not through an
/// integer: a huge pointer's selector then offset (`sub`, `sbb`, as the old
/// raise), a near pointer's offset, both unsigned (qcport-rich's request).
#[test]
fn test_ordered_compares_of_pointers_are_unsigned_and_lowered_here() {
    let below = |space: &str| {
        listing(
            &format!("define i16 @f(ptr{space} %a, ptr{space} %b) addrspace(1) {{\n  %c = icmp ult ptr{space} %a, %b\n  %r = zext i1 %c to i16\n  ret i16 %r\n}}\n"),
            "f",
        )
        .join("\n")
    };
    let huge = below(" addrspace(3)");
    assert!(huge.contains("sbb") && !huge.contains("jl") && !huge.contains("setl") && !huge.contains("ptrtoint"), "{huge}");
    let near = below("");
    assert!(near.contains("cmp") && (near.contains("jb") || near.contains("setb") || near.contains("sbb")), "{near}");
    assert!(!near.contains("jl") && !near.contains("setl"), "{near}");
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
            "xor ax, ax",
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
            "xor ax, ax",
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
            "push word ptr count",
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

/// A memcpy of a constant length is loads and stores, widest first, each
/// through a register: no string move, and a byte not named is not read.
#[test]
fn test_a_small_memcpy_is_moves_widest_first() {
    let text = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f() addrspace(1) {
  %a = alloca [7 x i8]
  %b = alloca [7 x i8]
  store i16 3, ptr %a
  call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 7, i1 false)
  %v = load i16, ptr %b
  ret i16 %v
}
";
    let moves = listing(text, "f").into_iter().filter(|one| one.starts_with("mov") && one.contains('[')).collect::<Vec<_>>();
    assert!(moves.iter().any(|one| one.contains("dword ptr")), "{moves:?}");
    assert!(moves.iter().any(|one| one.contains("byte ptr")), "{moves:?}");
    assert!(!listing(text, "f").iter().any(|one| one.starts_with("rep")), "{moves:?}");
}

/// A pattern fill of a word or dword is `rep stosw` or `rep stosd` of its
/// cells; two cells are stores. Its count in cells is never doubled.
#[test]
fn test_a_pattern_fill_is_stores_or_rep_stos_of_its_cell() {
    let text = |bits: u32, count: u32| {
        format!(
            "declare void @llvm.experimental.memset.pattern.p0.i{bits}.i16(ptr, i{bits}, i16, i1)
define i16 @f() addrspace(1) {{
  %a = alloca [{count} x i{bits}]
  call void @llvm.experimental.memset.pattern.p0.i{bits}.i16(ptr %a, i{bits} 4660, i16 {count}, i1 false)
  %v = load i16, ptr %a
  ret i16 %v
}}
"
        )
    };
    let word = listing(&text(16, 40), "f");
    assert!(word.contains(&"mov cx, 40".to_owned()) && word.contains(&"rep stosw".to_owned()), "{word:?}");
    let dword = listing(&text(32, 40), "f");
    assert!(dword.contains(&"mov cx, 40".to_owned()) && dword.contains(&"rep stosd".to_owned()), "{dword:?}");
    let few = listing(&text(16, 2), "f");
    assert!(!few.iter().any(|one| one.starts_with("rep")) && few.iter().filter(|one| one.starts_with("mov word ptr [bp")).count() == 2, "{few:?}");
}

/// A memmove whose direction a pass proved is a forward or a backward copy:
/// downward it is `std`, the tail's bytes from the last byte, then the dwords
/// from the last dword, and `cld`. One no pass proved is refused.
#[test]
fn test_a_memmove_is_a_forward_or_a_backward_string_move() {
    let text = |way: &str, length: &str| {
        format!(
            "declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f(ptr %a, ptr %b, i16 %n) addrspace(1) {{
  call void @llvm.memmove.p0.p0.i16(ptr %a, ptr %b, i16 {length}, i1 false){way}
  ret i16 0
}}
!0 = !{{}}
"
        )
    };
    let forward = listing(&text(", !llrm.forward !0", "%n"), "f");
    assert!(forward.contains(&"rep movsd".to_owned()) && !forward.contains(&"std".to_owned()), "{forward:?}");
    let backward = listing(&text(", !llrm.backward !0", "%n"), "f");
    let at = |what: &str| backward.iter().position(|one| one == what).unwrap_or_else(|| panic!("{what}: {backward:?}"));
    assert!(at("std") < at("rep movsb") && at("rep movsb") < at("rep movsd") && at("rep movsd") < at("cld"), "{backward:?}");
    // A few bytes are loads and stores, the highest first.
    let few = listing(&text(", !llrm.backward !0", "18"), "f");
    let first = |what: &str| few.iter().position(|one| one.contains(what)).unwrap_or_else(|| panic!("{what}: {few:?}"));
    assert!(first("[si+16]") < first("[si+12]") && first("[si+4]") < first("[si]") && !few.contains(&"std".to_owned()), "{few:?}");
    // Past them the tail's two bytes, one dword step down, then the dwords.
    let many = listing(&text(", !llrm.backward !0", "70"), "f");
    assert_eq!(many.iter().filter(|one| one.as_str() == "movsb").count(), 2, "{many:?}");
    assert!(many.contains(&"rep movsd".to_owned()) && many.iter().any(|one| one == "sub si, 3") && many.iter().any(|one| one == "sub di, 3"), "{many:?}");
    let refused = std::panic::catch_unwind(|| listing(&text("", "%n"), "f"));
    assert!(refused.is_err(), "a memmove of no proved direction was selected");
}

/// A backward move of constant addresses and a length of whole dwords stepped
/// `mov si, K` then `sub si, 3` (and the same for di): the start is one
/// `mov si, K-3`.
#[test]
fn test_a_backward_memmove_of_constant_addresses_starts_at_its_last_dword() {
    let text = "@a = internal global [3840 x i8] zeroinitializer
@b = internal global [3840 x i8] zeroinitializer
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f() addrspace(1) {
  call void @llvm.memmove.p0.p0.i16(ptr @a, ptr @b, i16 3840, i1 false), !llrm.backward !0
  ret i16 0
}
!0 = !{}
";
    let lines = listing(text, "f");
    assert!(lines.contains(&"std".to_owned()), "premise: a backward string move: {lines:?}");
    let starts = |register: &str| lines.iter().filter(|one| one.starts_with(&format!("mov {register}, offset")) && one.ends_with("+3836")).count();
    assert_eq!((starts("si"), starts("di")), (1, 1), "{lines:?}");
    assert!(!lines.iter().any(|one| one.starts_with("sub si") || one.starts_with("sub di")), "{lines:?}");
}

/// Every near string op saved ES, set it to DS and restored it
/// (`push es / push ds / pop es / ... / pop es`), 16 instructions a trip of
/// scroll's loop. DGROUP is a constant any op reads: set once ahead of the
/// loop, and nothing saved, for the C convention keeps no ES.
#[test]
fn test_near_string_ops_set_es_once_and_save_none() {
    let text = "@a = internal global [3840 x i8] zeroinitializer
@b = internal global [3840 x i8] zeroinitializer
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f(i16 %n) addrspace(1) {
  br label %loop
loop:
  %i = phi i16 [ 0, %0 ], [ %next, %loop ]
  call void @llvm.memcpy.p0.p0.i16(ptr @a, ptr @b, i16 3840, i1 false)
  call void @llvm.memcpy.p0.p0.i16(ptr @b, ptr @a, i16 3840, i1 false)
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %n
  br i1 %done, label %out, label %loop
out:
  ret i16 0
}
";
    let lines = listing(text, "f");
    let (head, body) = lines.split_at(lines.iter().position(|one| one.starts_with("L0_") && one != "L0_0:").expect("the loop label"));
    assert_eq!(body.iter().filter(|one| one.starts_with("rep movsd")).count(), 2, "premise: two string moves in the loop: {lines:?}");
    assert!(!lines.iter().any(|one| one == "push es" || one == "push ds"), "ES saved: {lines:?}");
    assert!(!body.iter().any(|one| one.contains(" es") || one.contains("DGROUP")), "ES set inside the loop: {lines:?}");
    assert!(head.iter().any(|one| one == "pop es" || one.starts_with("mov es,")), "ES set ahead of the loop: {lines:?}");
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
            "mov es, ss",
            "mov eax, 16843009",
            "mov cx, 17",
            "rep stosd",
            "stosw",
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
        ["push bp", "mov bp, sp", "sub sp, 6", "L0_0:", "fnstcw word ptr [bp-4]", "mov ax, word ptr [bp-4]", "or ax, 3072", "mov word ptr [bp-6], ax", "L0_1:", "fld dword ptr $K1", "fadd dword ptr $K2", "fldcw word ptr [bp-6]", "fistp word ptr [bp-2]", "mov ax, word ptr [bp-2]", "fldcw word ptr [bp-4]", "leave", "retf"]
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
            "push si",
            "L0_0:",
            "les bx, dword ptr [bp+6]",
            "mov si, word ptr [bp+10]",
            "add si, si",
            "mov ax, word ptr es:[bx+si]",
            "mov word ptr es:[bx+4], ax",
            "push ax",
            "pushw DGROUP",
            "push offset buf",
            "call take",
            "add sp, 6",
            "mov ax, word ptr [bp+6]",
            "add ax, 4",
            "mov dx, word ptr [bp+8]",
            "pop si",
            "pop bp",
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
            "sub sp, 32",
            "L1_0:",
            "mov ax, word ptr [bp+6]",
            "mov word ptr [bp-2], ax",
            "fild word ptr [bp-2]",
            "fst qword ptr [bp-8]",
            "push dword ptr [bp-4]",
            "push dword ptr [bp-8]",
            "fst qword ptr [bp-16]",
            "push dword ptr [bp-12]",
            "push dword ptr [bp-16]",
            "fstp qword ptr [bp-24]",
            "call far ptr scale",
            "add sp, 16",
            "fst dword ptr [bp-4]",
            "fld dword ptr [bp-4]",
            "fst dword ptr out",
            "fstp dword ptr [bp-4]",
            "push dword ptr [bp-4]",
            "fstp qword ptr [bp-32]",
            "call far ptr show",
            "fld qword ptr [bp-32]",
            "fcomp qword ptr [bp-24]",
            "fnstsw ax",
            "sahf",
            "ja L1_8",
            "L1_10:",
            "xor ax, ax",
            "leave",
            "retf",
            "L1_8:",
            "fld qword ptr [bp-32]",
            "fistp word ptr [bp-2]",
            "mov ax, word ptr [bp-2]",
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
        ["push bp", "mov bp, sp", "sub sp, 8", "L0_0:", "fnstcw word ptr [bp-6]", "mov ax, word ptr [bp-6]", "or ax, 3072", "mov word ptr [bp-8], ax", "fld qword ptr [bp+6]", "fld st(0)", "fldcw word ptr [bp-8]", "fistp word ptr [bp-2]", "mov ax, word ptr [bp-2]", "fistp dword ptr [bp-4]", "mov bx, word ptr [bp-4]", "add ax, bx", "fldcw word ptr [bp-6]", "leave", "retf"]
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
    // Either polarity: the branch reads the call's flags, no compare between.
    assert!(got[call + 1].starts_with("jg ") || got[call + 1].starts_with("jle "), "{got:?}");
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
        ["push bp", "mov bp, sp", "sub sp, 4", "L0_0:", "fnstcw word ptr [bp-2]", "mov ax, word ptr [bp-2]", "or ax, 3072", "mov word ptr [bp-4], ax", "fld qword ptr [bp+6]", "fld st(0)", "fldcw word ptr [bp-4]", "fistp word ptr b", "fld st(0)", "fldcw word ptr [bp-2]", "fistp word ptr b", "fstp dword ptr o", "leave", "retf"]
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
    assert_eq!(inner(local), ["mov si, word ptr [bp+6]", "add si, si", "mov word ptr [bp+si-16], 3", "mov ax, word ptr [bp+si-16]"]);
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
        ["mov bx, word ptr [bp+6]", "mov ax, word ptr [bp+8]", "lea bx, [ebx+eax*2]", "mov word ptr [bx], 0", "push bx", "call take", "add sp, 2"]
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
    // The byte test reads its cell where it runs: the load crosses the address arithmetic.
    assert_eq!(listing(text, "f")[3..7], ["mov bx, word ptr [bp+6]", "add bx, bx", "add bx, offset a", "cmp byte ptr [bp+8], 0"]);
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
    assert_eq!(inner(variable), ["mov ax, word ptr [bp+6]", "imul ax, word ptr [bp+8]"]);
}

/// Tuned for size, a multiply by a constant is the imul unless the shifts and adds are fewer
/// bytes: both selectors priced in clocks, so x * 446 was a 20-byte chain where `imul r, r, imm`
/// is 7, and -Os grew examples/mandel.nib by 7 bytes.
#[test]
fn test_a_multiply_by_a_constant_tuned_for_size_is_the_smaller_form() {
    let text = "define i32 @f(i32 %x) addrspace(1) {\n  %q = mul i32 %x, 446\n  ret i32 %q\n}\n";
    let on = |size: bool| {
        let cpu = crate::backend::cpu::tuned("486", size).expect("a target");
        let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Profile(cpu), &crate::backend::target::BASIC).expect("assembles");
        masm::text(&module).expect("prints")
    };
    assert!(on(true).contains("imul eax, eax, 446") && !on(true).contains("shl eax"), "{}", on(true));
    assert!(on(false).contains("shl eax") && !on(false).contains("imul"), "{}", on(false));
}

/// Tuned for size, a dword constant argument is the two word pushes where they are fewer
/// bytes: `pushd 7340144` is 6, `push 112; push 112` 4. QCport -Os had some 800 of them (the
/// fixed-point and float arguments), and `dword_push` joined any two word pushes.
#[test]
fn test_a_dword_constant_argument_tuned_for_size_is_the_fewer_bytes_of_one_push_or_two() {
    let text = "declare void @use(i32, float, i32, i32)
define void @f() addrspace(1) {
  call void @use(i32 7340144, float 1.0, i32 7, i32 -3)
  ret void
}
";
    let on = |size: bool| {
        let cpu = crate::backend::cpu::tuned("486", size).expect("a target");
        let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Profile(cpu), &crate::backend::target::BASIC).expect("assembles");
        masm::text(&module).expect("prints")
    };
    let pushes = |listing: String| listing.lines().map(str::trim).filter(|line| line.starts_with("push")).map(str::to_owned).collect::<Vec<_>>();
    // Last argument first: -3 and 7 are byte-immediate dword pushes; 1.0 is two words (5 bytes);
    // 7340144 is two (4), the first's zero low word joining the next word as `pushd 112` (3 for 4).
    assert_eq!(pushes(on(true)), ["pushd -3", "pushd 7", "pushw 16256", "pushd 112", "pushw 112"], "{}", on(true));
    assert_eq!(pushes(on(false)), ["pushd -3", "pushd 7", "pushd 1065353216", "pushd 7340144"], "{}", on(false));
}

/// A dword compared for equality with a constant xors only the halves the constant sets: QCport
/// -Os had `xor dx,0` (3 bytes, no effect) in 71 `got != 73`-style compares.
#[test]
fn test_a_dword_equality_xors_only_the_constant_halves_that_are_not_zero() {
    let text = |constant: i64| {
        format!("declare i32 @rd()
define i16 @f() addrspace(1) {{
  %v = call i32 @rd()
  %c = icmp ne i32 %v, {constant}
  %r = zext i1 %c to i16
  ret i16 %r
}}
")
    };
    let xors = |constant: i64| listing(&text(constant), "f").into_iter().filter(|line| line.starts_with("xor")).collect::<Vec<_>>();
    assert_eq!(xors(73), ["xor ax, 73"]);
    assert_eq!(xors(0x20000), ["xor dx, 2"]);
    assert_eq!(xors(0x20005), ["xor ax, 5", "xor dx, 2"]);
}

/// An internal function given `fastcc` pops its own arguments: `ret 6`, and its caller's
/// cleanup is the callee's, not an `add sp,6` too (QCport -Os: some 720 of those).
#[test]
fn test_a_fastcc_function_pops_its_arguments_and_its_caller_does_not() {
    let text = "define internal fastcc i16 @work(i16 %a, i16 %b, i16 %c) {
  %x = add i16 %a, %b
  %y = add i16 %x, %c
  ret i16 %y
}
define i16 @f(i16 %x) {
  %p = call fastcc i16 @work(i16 %x, i16 2, i16 3)
  ret i16 %p
}
";
    let work = listing(text, "work");
    assert_eq!(work.last().map(String::as_str), Some("ret 6"), "{work:?}");
    let caller = listing(text, "f");
    assert!(caller.iter().any(|line| line.starts_with("call")) && !caller.iter().any(|line| line.starts_with("add sp") || line == "pop cx"), "{caller:?}");
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
    let mut steps = got[body + 3..body + 5].to_vec();
    steps.sort();
    assert_eq!((&got[body + 1..body + 3], steps, &got[body + 5..body + 7]), (&["mov cl, byte ptr es:[si]".to_owned(), "mov byte ptr [bx], cl".to_owned()][..], vec!["inc bx".to_owned(), "inc si".to_owned()], &["dec ax".to_owned(), "jne L0_2".to_owned()][..]), "{got:?}");
}

/// An unsigned integer converts as the signed one twice its width it
/// zero-extends to, which fild reads exactly; isel refused "UIToFP of a float".
#[test]
fn test_an_unsigned_integer_to_a_float_is_fild_of_twice_its_width() {
    let converted = |ty: &str| inner(&format!("define double @f({ty} %x) addrspace(1) {{\n  %f = uitofp {ty} %x to double\n  ret double %f\n}}\n"));
    assert_eq!(converted("i8"), ["movzx ax, byte ptr [bp+6]", "mov word ptr [bp-2], ax", "fild word ptr [bp-2]"]);
    assert_eq!(converted("i16"), ["movzx eax, word ptr [bp+6]", "mov dword ptr [bp-4], eax", "fild dword ptr [bp-4]"]);
    assert_eq!(converted("i32"), ["mov eax, dword ptr [bp+6]", "mov dword ptr [bp-8], eax", "mov dword ptr [bp-4], 0", "fild qword ptr [bp-8]"]);
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
    assert_eq!(got[..fild], ["mov eax, dword ptr [bp+6]", "xor ebx, ebx", "mov dword ptr [bp-8], eax", "mov dword ptr [bp-4], ebx"], "{got:?}");
}

/// A zeroed 22-byte array descriptor was five `mov dword ptr [bp-n], 0`, 8
/// bytes each: qbdemo's WHITEFADE spent 45 bytes clearing it.
#[test]
fn test_a_memsets_dwords_store_one_register() {
    let text = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define i16 @f() addrspace(1) {
  %a = alloca [12 x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 12, i1 false)
  %v = load i16, ptr %a
  ret i16 %v
}
";
    let got = listing(text, "f");
    let stores: Vec<&String> = got.iter().filter(|line| line.starts_with("mov dword ptr")).collect();
    assert_eq!(stores.len(), 3, "{got:?}");
    assert!(stores.iter().all(|line| line.ends_with(", eax")), "{got:?}");
}

/// Tuned for size, a memset is the smaller of its stores and one
/// `rep stosb`: a 22-byte descriptor's six stores are 31 bytes, the fill
/// 16. Always stores, qbdemo cleared nine descriptors so and grew 1.5% at
/// -Os; as `rep stosd` and a tail the fill was 23.
#[test]
fn test_a_memset_tuned_for_size_is_the_smaller_form() {
    let text = |size: u32| {
        format!(
            "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define i16 @f() addrspace(1) {{
  %a = alloca [{size} x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 {size}, i1 false)
  %v = load i16, ptr %a
  ret i16 %v
}}
"
        )
    };
    let sized = |size: u32| {
        let cpu = crate::backend::cpu::tuned("486", true).expect("a target");
        let module = assemble::assembled(&parsed(&text(size)), &qb(), "T_TEXT", ProfileOrName::Profile(cpu), &crate::backend::target::BASIC).expect("assembles");
        masm::text(&module).expect("prints")
    };
    let filled = sized(22);
    assert!(filled.contains("rep stosb") && !filled.contains("stosd") && !filled.contains("stosw"), "{filled}");
    assert!(filled.contains("mov cx, 22"), "{filled}");
    assert!(!sized(8).contains("stos"), "{}", sized(8));
    assert!(!assembled_on("486", &text(22)).contains("stos"));
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
            "xor eax, eax",
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

/// A dword that arrives in dx:ax is joined as the old route joins it,
/// `shl eax, 16` and `shrd eax, edx, 16`: two zero extensions, a shift and
/// an `or` cost parity/control two more instructions per call.
#[test]
fn test_a_dword_from_dx_ax_is_joined_by_shrd() {
    let text = "declare i32 @g() addrspace(1)
define i32 @f() addrspace(1) {
  %a = call addrspace(1) i32 @g()
  %b = mul i32 %a, 3
  ret i32 %b
}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.starts_with("shrd")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("movzx") || line.starts_with("or ")), "{got:?}");
}

/// A float constant argument is pushed as its bits, as a float constant is
/// stored: qmove loaded each from the pool and stored it to a temporary to
/// push it from there.
#[test]
fn test_a_float_constant_argument_is_pushed_as_its_bits() {
    let text = "declare void @g(float, double) addrspace(1)
define void @f() addrspace(1) {
  call addrspace(1) void @g(float 2.5e-01, double 1.0e+01)
  ret void
}
";
    let got = listing_on("386", text, "f");
    assert!(!got.iter().any(|line| line.starts_with("fld") || line.starts_with("fstp")), "{got:?}");
    assert_eq!(got.iter().filter(|line| line.starts_with("push")).count(), 3, "{got:?}");
}

/// A pointer at offset 0 from a register is that register: string-array-
/// element's descriptor offset was made by `add ax, 0`.
#[test]
fn test_a_pointer_at_offset_zero_adds_nothing() {
    let text = "declare void @g(ptr) addrspace(1)
define void @f(ptr %p) addrspace(1) {
  %q = getelementptr i8, ptr %p, i16 0
  call addrspace(1) void @g(ptr %q)
  ret void
}
";
    let got = listing_on("386", text, "f");
    assert!(!got.iter().any(|line| line.starts_with("add") && line.ends_with(", 0")), "{got:?}");
}

/// A routine the ABI passes and answers in registers: pdhuge's B$HARY takes
/// its descriptor in BX after its pushed subscripts and count, and answers
/// the element in ES:BX, which the store goes through directly. It was
/// refused as "a { i16, i16 } value".
#[test]
fn test_an_answer_in_registers_is_its_fields() {
    let text = "@d = internal global [8 x i8] zeroinitializer
declare cc1000 { i16, i16 } @llrm.qb.B$HARY(...) addrspace(1)
define void @f(i16 %i) addrspace(1) {
  %a = call cc1000 addrspace(1) { i16, i16 } (...) @llrm.qb.B$HARY(i16 %i, i16 1, ptr @d)
  %o = extractvalue { i16, i16 } %a, 0
  %s = extractvalue { i16, i16 } %a, 1
  %t = inttoptr i16 %s to ptr addrspace(2)
  %u = addrspacecast ptr addrspace(2) %t to ptr addrspace(1)
  %p = getelementptr i8, ptr addrspace(1) %u, i16 %o
  store i16 123, ptr addrspace(1) %p
  ret void
}
";
    let got = listing(text, "f");
    let call = got.iter().position(|line| line.starts_with("call far ptr B$HARY")).unwrap_or_else(|| panic!("{got:?}"));
    assert_eq!(got[call - 1], "mov bx, offset d", "{got:?}");
    assert_eq!(got.iter().filter(|line| line.starts_with("push") && *line != "push bp").count(), 2, "{got:?}");
    assert_eq!(got[call + 1], "mov word ptr es:[bx], 123", "{got:?}");
}

/// An index the MIR scales by its own `mul nsw` is matched as the scaled
/// index it is, as LLVM's address matcher folds a multiply into the scale:
/// lru's `bstamp[block]` shifted a copy of `block` by 2 into a 16-bit index.
#[test]
fn test_an_index_multiplied_in_the_mir_is_the_scaled_index() {
    let text = "define void @f(ptr addrspace(1) %s, i16 %i) addrspace(1) {
entry:
  %b = load ptr addrspace(1), ptr addrspace(1) %s, !tbaa !1
  %c = icmp slt i16 %i, 0
  br i1 %c, label %no, label %ok
ok:
  %m = mul nsw i16 %i, 4
  %e = getelementptr inbounds i8, ptr addrspace(1) %b, i16 %m
  store i32 7, ptr addrspace(1) %e, !tbaa !1
  ret void
no:
  ret void
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.contains("*4]")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("shl") || line.starts_with("imul")), "{got:?}");
}

/// A multiply that may wrap is still the scale where the index's range
/// says it cannot: segld's `a(i)` scaled `i` in 1..20 by a plain `mul`, and
/// `lea si, [ecx+ecx]` ran before every store.
#[test]
fn test_an_index_multiplied_within_its_range_is_the_scaled_index() {
    let text = "define void @f(ptr addrspace(1) %s, i16 %i) addrspace(1) {
entry:
  %b = load ptr addrspace(1), ptr addrspace(1) %s, !tbaa !1
  %c = icmp slt i16 %i, 0
  br i1 %c, label %no, label %low
low:
  %d = icmp sgt i16 %i, 999
  br i1 %d, label %no, label %ok
ok:
  %m = mul i16 %i, 4
  %e = getelementptr inbounds i8, ptr addrspace(1) %b, i16 %m
  store i32 7, ptr addrspace(1) %e, !tbaa !1
  ret void
no:
  ret void
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.contains("*4]")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("shl") || line.starts_with("imul") || line.starts_with("lea")), "{got:?}");
    let wraps = text.replace("sgt i16 %i, 999", "sgt i16 %i, 9999");
    let got = listing_on("386", &wraps, "f");
    assert!(!got.iter().any(|line| line.contains("*4]")), "{got:?}");
}

/// A branch on the `and` or `or` of two compares branches on each, as
/// LLVM's FindMergedConditions splits it: rcflip's IF a = 0 AND b = 1 made
/// both truth values, and-ed them, and tested the result.
#[test]
fn test_a_branch_on_two_compares_branches_on_each() {
    for join in ["and", "or"] {
        let text = format!(
            "declare void @g() addrspace(1)
define void @f(i16 %a, i16 %b) addrspace(1) {{
entry:
  %x = icmp eq i16 %a, 0
  %y = icmp slt i16 %b, 5
  %c = {join} i1 %x, %y
  br i1 %c, label %yes, label %no
yes:
  call addrspace(1) void @g()
  ret void
no:
  ret void
}}
"
        );
        let got = listing_on("386", &text, "f");
        assert!(!got.iter().any(|line| line.starts_with("set") || line.starts_with("and") || line.starts_with("or ")), "{join}: {got:?}");
        let branches: Vec<&String> = got.iter().filter(|line| line.starts_with('j') && !line.starts_with("jmp")).collect();
        assert_eq!(branches.len(), 2, "{join}: {got:?}");
    }
}

/// A 32-bit counter proven small indexes every frame array at its own
/// scale, `[ebp+esi*4-disp]` with one register, and EBP's upper half zeroed once before the
/// loop: truncated, each array took a shift and a register of its own.
#[test]
fn test_a_wide_counter_indexes_frame_arrays_at_each_scale() {
    let text = "define i32 @f() addrspace(1) {
entry:
  %l = alloca [16 x i32]
  %w = alloca [16 x i16]
  %b = alloca [16 x i8]
  br label %body
body:
  %i = phi i32 [ 0, %entry ], [ %j, %body ]
  %s = phi i32 [ 0, %entry ], [ %t, %body ]
  %pl = getelementptr inbounds i32, ptr %l, i32 %i
  %pw = getelementptr inbounds i16, ptr %w, i32 %i
  %pb = getelementptr inbounds i8, ptr %b, i32 %i
  %vl = load i32, ptr %pl, !tbaa !1
  %vw = load i16, ptr %pw, !tbaa !3
  %vb = load i8, ptr %pb, !tbaa !5
  %xw = sext i16 %vw to i32
  %xb = sext i8 %vb to i32
  %u = add i32 %vl, %xw
  %t = add i32 %u, %xb
  %j = add nuw nsw i32 %i, 1
  %more = icmp ult i32 %j, 16
  br i1 %more, label %body, label %done
done:
  ret i32 %t
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
!2 = !{!\"int\"}
!3 = !{!2, !2, i64 0}
!4 = !{!\"char\"}
!5 = !{!4, !4, i64 0}
";
    let got = listing(text, "f");
    let top = got.iter().position(|line| line.ends_with(':') && line != "L0_0:").expect("a loop label");
    let end = top + got[top..].iter().position(|line| line.starts_with('j')).expect("the back edge");
    let looped = &got[top..end];
    let cells: Vec<&String> = looped.iter().filter(|line| line.contains('[')).collect();
    assert_eq!(cells.len(), 3, "{got:?}");
    let index = |cell: &str| cell.split_once("[ebp+").map(|(_, rest)| rest[..3].to_owned());
    let shared = index(cells[0]).expect("a frame cell 32 bits wide");
    for (cell, scale) in cells.iter().zip(["*4", "*2", ""]) {
        assert!(index(cell).as_ref() == Some(&shared) && cell.contains(&format!("{shared}{scale}")), "{cell}: {got:?}");
    }
    assert!(got[..top].contains(&"movzx ebp, bp".to_owned()), "{got:?}");
    assert!(!looped.iter().any(|line| line.starts_with("shl ") || line.starts_with("movzx ebp")), "{got:?}");
}

/// A dword counter counted up to zero is a negative index, still one word
/// sign-extended: `[ebp+esi*4-disp]` names the same byte. Required
/// non-negative, the counted-down loop lost the form.
#[test]
fn test_a_negative_dword_counter_indexes_a_frame_array() {
    let text = "define i32 @f() addrspace(1) {
entry:
  %l = alloca [16 x i32]
  %end = getelementptr inbounds i8, ptr %l, i16 64
  br label %body
body:
  %i = phi i32 [ -16, %entry ], [ %j, %body ]
  %s = phi i32 [ 0, %entry ], [ %t, %body ]
  %p = getelementptr inbounds i32, ptr %end, i32 %i
  %v = load i32, ptr %p, !tbaa !1
  %t = add i32 %s, %v
  %j = add nsw i32 %i, 1
  %more = icmp ne i32 %j, 0
  br i1 %more, label %body, label %done
done:
  ret i32 %t
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.contains("[ebp+") && line.contains("*4")), "{got:?}");
}

/// EBP's upper half is zeroed once, before the outermost loop that leaves
/// it alone: zeroed at each innermost loop, nbody's outer loop ran a
/// `movzx ebp,bp` every trip.
#[test]
fn test_ebp_is_zeroed_once_outside_every_loop_that_keeps_it() {
    let text = "define i32 @f(i16 %n) addrspace(1) {
entry:
  %l = alloca [16 x i32]
  br label %outer
outer:
  %k = phi i32 [ 0, %entry ], [ %k.next, %done ]
  %so = phi i32 [ 0, %entry ], [ %t, %done ]
  %p = getelementptr inbounds i32, ptr %l, i32 %k
  %w = load i32, ptr %p, !tbaa !1
  br label %body
body:
  %i = phi i16 [ 0, %outer ], [ %j, %body ]
  %s = phi i32 [ %so, %outer ], [ %t, %body ]
  %v = load i32, ptr %p, !tbaa !1
  %u = add i32 %s, %v
  %t = add i32 %u, %w
  store i32 %t, ptr %p, !tbaa !1
  %j = add i16 %i, 1
  %more = icmp ult i16 %j, %n
  br i1 %more, label %body, label %done
done:
  %k.next = add nsw i32 %k, 1
  %again = icmp ult i32 %k.next, 16
  br i1 %again, label %outer, label %exit
exit:
  ret i32 %t
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.contains("[ebp+")), "{got:?}");
    assert_eq!(got.iter().filter(|line| *line == "movzx ebp, bp").count(), 1, "{got:?}");
}

/// A product folded into a dword index's scale is not also computed:
/// shellsort's loop kept `add esi,esi`, doubling a stale register nothing
/// read.
#[test]
fn test_a_product_folded_into_the_scale_is_not_computed() {
    let text = "define void @f(i16 %n) addrspace(1) {
entry:
  %a = alloca [128 x i8]
  br label %head
head:
  %i = phi i32 [ 0, %entry ], [ %j, %body ]
  %more = icmp ult i32 %i, 64
  %w = trunc i32 %i to i16
  br i1 %more, label %body, label %done
body:
  %v = shl i16 %w, 7
  %m = mul i32 %i, 2
  %p = getelementptr inbounds i8, ptr %a, i32 %m
  store i16 %v, ptr %p, !tbaa !1
  %j = add i32 %i, 1
  br label %head
done:
  ret void
}

!0 = !{!\"int\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.contains("*2")), "{got:?}");
    let doubled = |line: &&String| line.split_once(' ').is_some_and(|(op, rest)| op == "add" && rest.split(", ").collect::<Vec<_>>().windows(2).any(|pair| pair[0] == pair[1]));
    assert!(!got.iter().any(|line| doubled(&line) || line.starts_with("shl e") || line.starts_with("lea ")), "{got:?}");
}

/// qbdemo's PLASMA, array-merged: the video segment, a constant spilled
/// where every selector is taken, was rematerialized at its store inside
/// the pixel loop, `push 0A000h / pop ds` every trip.
#[test]
fn test_a_constant_selector_is_loaded_outside_the_loop_that_uses_it() {
    let text = std::fs::read_to_string(concat!(env!("LLRM_ROOT"), "/tests/check/mir/plasma.ll")).unwrap();
    let got = listing(&text, "PLASMA");
    let video = |line: &String| line.contains("-24576") || line.contains("40960") || line.contains("0A000h");
    let loops: Vec<(usize, usize)> = got
        .iter()
        .enumerate()
        .filter_map(|(bottom, line)| {
            let target = line.split_whitespace().nth(1).filter(|_| line.starts_with('j'))?;
            Some((got[..bottom].iter().position(|one| *one == format!("{target}:"))?, bottom))
        })
        .collect();
    for (top, bottom) in &loops {
        let innermost = !loops.iter().any(|(other, below)| (other, below) != (top, bottom) && top <= other && below <= bottom);
        assert!(!innermost || !got[*top..*bottom].iter().any(video), "{:#?}", &got[*top..=*bottom]);
    }
}

/// keybind.c's `action_for` passes its far pointer parameter on in a loop.
/// dword_pairs sliced one block with the other half's load index and
/// panicked: "slice index starts at 2 but ends at 1".
#[test]
fn test_a_far_parameter_pushed_in_a_loop_is_selected() {
    let text = "@_names = internal global [2 x i16] [i16 0, i16 0]\n\
declare i16 @_text_eq(ptr addrspace(1), ptr) addrspace(1)\n\
define i16 @_action_for(ptr addrspace(1) %0) addrspace(1) {\n\
b1:\n  br label %b2\n\
b2:\n  %1 = phi i16 [ 0, %b1 ], [ %8, %b5 ]\n  %2 = icmp slt i16 %1, 8\n  br i1 %2, label %b4, label %b7\n\
b4:\n  %3 = mul nsw i16 %1, 2\n  %4 = getelementptr inbounds i8, ptr @_names, i16 %3\n  %5 = load ptr, ptr %4\n  %6 = call addrspace(1) i16 @_text_eq(ptr addrspace(1) %0, ptr %5)\n  %7 = icmp ne i16 %6, 0\n  br i1 %7, label %b7, label %b5\n\
b5:\n  %8 = add nsw i16 %1, 1\n  br label %b2\n\
b7:\n  %9 = phi i16 [ -1, %b2 ], [ %1, %b4 ]\n  ret i16 %9\n}\n";
    assert!(selected(text, "_action_for").is_ok());
}

/// x87 stores no byte integer: loadscr.c's palette `(char)(6 + f * 62)` and
/// part.c were refused as "a float to a byte". A word is stored, and its
/// low byte read.
#[test]
fn test_a_float_to_a_byte_is_a_stored_word_low_byte() {
    let text = "define void @f(float %x, ptr %p) addrspace(1) {
  %s = fptosi float %x to i8
  store i8 %s, ptr %p
  %q = getelementptr i8, ptr %p, i16 1
  %u = fptoui float %x to i8
  store i8 %u, ptr %q
  ret void
}
";
    let got = listing(text, "f");
    let stores: Vec<&String> = got.iter().filter(|one| one.starts_with("fistp")).collect();
    assert!(stores.len() == 2 && stores.iter().all(|one| one.starts_with("fistp word ptr")), "{got:#?}");
}

/// mathlib.c's `ent_step_to`: a compare consuming one operand and keeping
/// the other, with seven more floats live. floatassign missed the consumed
/// operand's slot and floatalloc refused "floating instruction requires too
/// many stack operands".
#[test]
fn test_a_compare_counts_the_operand_it_pops() {
    let text = "declare float @_q_rsqrt(float) addrspace(1)
define i16 @_ent_step_to(ptr addrspace(1) %0, ptr addrspace(1) %1, float %2) addrspace(1) {
b1:
  %3 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 0
  %4 = load float, ptr addrspace(1) %3
  %5 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 0
  %6 = load float, ptr addrspace(1) %5
  %7 = fsub float %4, %6
  %8 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 4
  %9 = load float, ptr addrspace(1) %8
  %10 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 4
  %11 = load float, ptr addrspace(1) %10
  %12 = fsub float %9, %11
  %13 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 8
  %14 = load float, ptr addrspace(1) %13
  %15 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 8
  %16 = load float, ptr addrspace(1) %15
  %17 = fsub float %14, %16
  %18 = fmul float %7, %7
  %19 = fmul float %12, %12
  %20 = fadd float %18, %19
  %21 = fmul float %17, %17
  %22 = fadd float %20, %21
  %23 = fmul float %2, %2
  %24 = fcmp ole float %22, %23
  br i1 %24, label %b3, label %b2

b2:
  %25 = call addrspace(1) float @_q_rsqrt(float %22)
  %26 = fmul float %2, %25
  %27 = fmul float %7, %26
  %28 = fadd float %6, %27
  store float %28, ptr addrspace(1) %5
  %29 = fmul float %12, %26
  %30 = fadd float %11, %29
  store float %30, ptr addrspace(1) %10
  %31 = fmul float %17, %26
  %32 = fadd float %16, %31
  store float %32, ptr addrspace(1) %15
  br label %b4

b3:
  store float %4, ptr addrspace(1) %5
  %33 = load float, ptr addrspace(1) %8
  store float %33, ptr addrspace(1) %10
  %34 = load float, ptr addrspace(1) %13
  store float %34, ptr addrspace(1) %15
  br label %b4

b4:
  %35 = phi i16 [ 0, %b2 ], [ 1, %b3 ]
  ret i16 %35
}
";
    let got = listing(text, "_ent_step_to");
    assert!(got.iter().any(|one| one == "call far ptr _q_rsqrt"), "{got:#?}");
}

/// r_walk.c's `r_recursive_world_node`: three values live across a float
/// compare, one spilled and reloaded into ax between `fnstsw ax` and
/// `sahf`. The branch went on garbage flags and the far wall was not drawn.
#[test]
fn test_nothing_writes_ax_between_fnstsw_and_sahf() {
    let text = "declare float @g() addrspace(1)
declare i16 @k() addrspace(1)
declare void @h(i16, i16, i16) addrspace(1)
define void @f() addrspace(1) {
b0:
  %y = call addrspace(1) i16 @k()
  %z = call addrspace(1) i16 @k()
  %w = call addrspace(1) i16 @k()
  %x = call addrspace(1) float @g()
  %c = fcmp oge float %x, 0.000000e+00
  br i1 %c, label %b1, label %b2
b1:
  call addrspace(1) void @h(i16 %y, i16 %z, i16 %w)
  call addrspace(1) void @h(i16 %z, i16 %w, i16 %y)
  ret void
b2:
  call addrspace(1) void @h(i16 %w, i16 %y, i16 %z)
  call addrspace(1) void @h(i16 %y, i16 %z, i16 %w)
  ret void
}
";
    let text = assembled_by(&borland(), &crate::backend::target::BUILT_IN, "386", text);
    let from = text.find("f proc").expect("the procedure");
    let got: Vec<String> = text[from..].lines().skip(1).take_while(|line| !line.ends_with("endp")).map(|line| line.trim().to_owned()).collect();
    let writes_ax = |one: &str| one.split_once(' ').is_some_and(|(_, rest)| rest.starts_with("ax,") || rest.starts_with("eax,") || rest.starts_with("ah,"));
    let mut status = false;
    for one in &got {
        if one == "fnstsw ax" {
            status = true;
        } else if one == "sahf" {
            status = false;
        } else {
            assert!(!(status && writes_ax(one)), "{one} between fnstsw and sahf:\n{got:#?}");
        }
    }
}

/// A stack temporary lives within the instruction that made it: qcport's
/// ai_pain took a fresh 8-byte cell for every double it passed, 60 bytes of
/// frame where one cell served them all.
#[test]
fn test_stack_temporaries_share_one_cell() {
    let text = "declare void @g(double) addrspace(1)
define void @f(double %a, double %b) addrspace(1) {
  %x = fadd double %a, %b
  call addrspace(1) void @g(double %x)
  %y = fmul double %a, %b
  call addrspace(1) void @g(double %y)
  ret void
}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|one| one == "sub sp, 8"), "{got:#?}");
}

/// A far pointer passed as a long, as qcport's qgl calls take
/// `(long) (void far *) &local`: its words were joined into a dword register
/// (movzx, movzx, shl, or: 13 bytes) only to push it. Its words are pushed.
#[test]
fn test_a_joined_dword_is_pushed_as_its_words() {
    let text = "@v = internal global i16 0
declare void @g(i32) addrspace(1)
define void @f() addrspace(1) {
  %p = addrspacecast ptr @v to ptr addrspace(1)
  %n = ptrtoint ptr addrspace(1) %p to i32
  call addrspace(1) void @g(i32 %n)
  ret void
}
";
    let got = listing(text, "f");
    assert!(!got.iter().any(|one| one.starts_with("movzx") || one.starts_with("shl") || one.starts_with("push e")), "{got:#?}");
}

/// A far pointer tested against null, as qcport tests `if ( ent )`: its
/// words were joined into a dword register (movzx, movzx, shl, or) to be
/// compared. Its words are or-ed.
#[test]
fn test_a_far_pointer_is_tested_by_its_words() {
    let text = "declare void @g() addrspace(1)
define void @f(ptr addrspace(1) %p, ptr addrspace(1) %q) addrspace(1) {
  %pi = ptrtoint ptr addrspace(1) %p to i32
  %n = icmp eq i32 %pi, 0
  br i1 %n, label %done, label %more
more:
  %qi = ptrtoint ptr addrspace(1) %q to i32
  %e = icmp ne i32 %pi, %qi
  br i1 %e, label %call, label %done
call:
  call addrspace(1) void @g()
  br label %done
done:
  ret void
}
";
    let got = listing(text, "f");
    assert!(!got.iter().any(|one| one.starts_with("movzx") || one.starts_with("shl") || one.contains(" e")), "{got:#?}");
}

/// A parameter live across calls and stores through pointers is reloaded
/// from its own argument cell, which no pointer reaches: qcport's
/// savegame_load copied every parameter into a frame slot first, 608 copies
/// over QCport, each read far from bp.
#[test]
fn test_a_parameter_spills_to_its_own_argument_cell() {
    let text = "declare void @g(i16) addrspace(1)
define void @f(i16 %a, i16 %b, i16 %c, i16 %d, ptr %p) addrspace(1) {
  call addrspace(1) void @g(i16 1)
  store i16 7, ptr %p
  call addrspace(1) void @g(i16 %a)
  call addrspace(1) void @g(i16 %b)
  call addrspace(1) void @g(i16 %c)
  call addrspace(1) void @g(i16 %d)
  call addrspace(1) void @g(i16 %a)
  call addrspace(1) void @g(i16 %b)
  call addrspace(1) void @g(i16 %c)
  call addrspace(1) void @g(i16 %d)
  ret void
}
";
    let got = assembled_by(&borland(), &crate::backend::target::BUILT_IN, "386", text);
    let from = got.find("f proc").expect("f");
    let body: Vec<&str> = got[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
    assert!(!body.iter().any(|one| one.contains("[bp-")), "{body:#?}");
}

/// A spill reload whose only reader is a push is the push's memory
/// operand: qcport's savegame_load reloaded each spilled argument into ax to
/// push it, a byte more each than BCC's `push [bp-n]`.
#[test]
fn test_a_reload_pushed_is_pushed_from_memory() {
    let mut text = String::from("declare i16 @h(i16) addrspace(1)\ndeclare void @k(i16) addrspace(1)\ndefine void @f() addrspace(1) {\n");
    for n in 0..8 {
        text += &format!("  %x{n} = call addrspace(1) i16 @h(i16 {n})\n");
    }
    for n in 0..8 {
        text += &format!("  call addrspace(1) void @k(i16 %x{n})\n");
    }
    text += "  ret void\n}\n";
    let got = assembled_by(&borland(), &crate::backend::target::BUILT_IN, "386", &text);
    let from = got.find("f proc").expect("f");
    let body: Vec<&str> = got[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
    let reloaded_then_pushed = body.windows(2).any(|two| {
        two[0].starts_with("mov ") && two[0].contains("ptr [bp-") && two[1] == format!("push {}", two[0][4..].split(',').next().unwrap_or(""))
    });
    assert!(!reloaded_then_pushed, "{body:#?}");
}

/// `-g`: each instruction's `!dbg` line reaches the object's LINNUM, at the
/// offset of the first code selected from it.
#[test]
fn test_dbg_lines_become_linnum() {
    let text = "define i16 @f(i16 %a, i16 %b) addrspace(1) {\n  %s = sub i16 %a, %b, !dbg !0\n  %t = add i16 %s, 3, !dbg !1\n  ret i16 %t, !dbg !1\n}\n!0 = !{i32 7}\n!1 = !{i32 8}\n";
    let module = parsed(text);
    let (_, _, function) = module.functions().next().expect("f");
    let instructions = function.layout().iter().flat_map(|&block| function.block(block).instructions());
    let attached = instructions.filter(|&&one| function.instruction(one).metadata.iter().any(|(kind, _)| kind == "dbg")).count();
    assert_eq!(attached, 3, "the fixture carries its lines");
    let mut assembled = assemble::assembled(&module, &qb(), "T_TEXT", ProfileOrName::Name("486"), &crate::backend::target::BASIC).expect("assembles");
    let records = |assembled: &crate::backend::masm::Module| {
        let object = crate::backend::objbuild::written_as(assembled, "t.asm", crate::backend::objbuild::CodeLayout::OneSegment).expect("encodes");
        llrm_omf::omf::parse(&object).expect("parses")
    };
    // Lines alone are a BASIC statement table's, not -g.
    assert!(!records(&assembled).iter().any(|one| one.r#type == llrm_omf::omf::LINNUM));
    let flavor = llrm_omf::cvwrite::Flavor { qb45: false };
    assembled.debug = Some(crate::backend::codeview::Debug { flavor, types: Vec::new(), nodes: Default::default(), procedures: Default::default(), globals: Vec::new() });
    let records = records(&assembled);
    let lines: Vec<(u16, u16)> = records.iter().filter(|one| one.r#type == llrm_omf::omf::LINNUM).flat_map(|one| llrm_omf::omf::lines(one).1).collect();
    // push bp; mov bp, sp (3 bytes) is line 7's; mov ax, [bp+6]; sub ax, [bp+8] (6 bytes) too.
    assert_eq!(lines, [(7, 0), (8, 9)]);
    let marker = records.iter().any(|one| one.r#type == llrm_omf::omf::COMENT && one.body.get(1) == Some(&0xA1));
    assert!(marker, "the CodeView marker");
}

/// A near global indexed by a dword the loop proves small is its scaled
/// cell: addrm's `B&` stored through `mov di,cx; shl edi,2` a trip, where
/// a frame or a far base took the scale.
#[test]
fn test_a_global_indexed_by_a_small_dword_is_the_scaled_cell() {
    let text = "@b = internal global [64 x i32] zeroinitializer
define void @f(i32 %i) addrspace(1) {
entry:
  %c = icmp slt i32 %i, 0
  br i1 %c, label %no, label %low
low:
  %d = icmp sgt i32 %i, 60
  br i1 %d, label %no, label %ok
ok:
  %m = shl i32 %i, 2
  %e = getelementptr i8, ptr @b, i32 %m
  store i32 7, ptr %e, !tbaa !1
  ret void
no:
  ret void
}

!0 = !{!\"long\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.contains("b[") && line.contains("*4]")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("shl")), "{got:?}");
}

/// A global indexed by one register and then another is `[bx+si+global]`:
/// conc12's twelve arrays, each `array + 2n` for the count-to-zero
/// counter, were twelve pointers spilled to the frame and reloaded each
/// trip, where the two registers make each address in its access.
#[test]
fn test_a_global_indexed_by_two_registers_is_one_address() {
    let text = "@g = internal global [64 x i16] zeroinitializer
define i16 @f(i16 %n, i16 %i) addrspace(1) {
entry:
  %a = getelementptr i8, ptr @g, i16 %n
  %b = getelementptr i8, ptr %a, i16 %i
  %c = getelementptr i8, ptr %b, i16 18
  %v = load i16, ptr %c, !tbaa !1
  ret i16 %v
}

!0 = !{!\"short\"}
!1 = !{!0, !0, i64 0}
";
    let got = listing_on("386", text, "f");
    assert!(got.iter().any(|line| line.contains("g+18[") && line.contains('+')), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("add ") || line.starts_with("lea ")), "{got:?}");
}

/// The optimizer left `getelementptr i8, ptr null, ...` and isel refused it as
/// "an address of no global: Null": examples/entries.nib and tally.nib did not
/// compile. A constant address is a direct address, `[disp16]`; a variable
/// index from it is the register.
#[test]
fn test_an_address_of_no_global_is_a_direct_address() {
    let constant = "define i16 @f() addrspace(1) {\n  %p = getelementptr i8, ptr null, i16 -4\n  %v = load i16, ptr %p\n  ret i16 %v\n}\n";
    let number = "define i16 @f() addrspace(1) {\n  %v = load i16, ptr inttoptr (i16 1132 to ptr)\n  ret i16 %v\n}\n";
    let variable = "define i16 @f(i16 %i) addrspace(1) {\n  %p = getelementptr i8, ptr null, i16 %i\n  %v = load i16, ptr %p\n  ret i16 %v\n}\n";
    assert!(constant.contains("getelementptr i8, ptr null") && number.contains("inttoptr (i16 1132") && variable.contains("ptr null, i16 %i"), "the shape that was refused");
    assert_eq!(listing(constant, "f"), ["L0_0:", "mov ax, word ptr [65532]", "retf"]);
    assert_eq!(listing(number, "f"), ["L0_0:", "mov ax, word ptr [1132]", "retf"]);
    assert_eq!(listing(variable, "f"), ["push bp", "mov bp, sp", "L0_0:", "mov bx, word ptr [bp+6]", "mov ax, word ptr [bx]", "pop bp", "retf"]);
}

/// `icmp` of far pointers was refused as "a ptr addrspace(1) value":
/// examples/roster.nib did not compile. Equal is both words; ordered, the
/// offsets.
#[test]
fn test_far_pointers_compare_by_words() {
    let compare = |predicate: &str, second: &str| {
        let text = format!("define i16 @f(ptr addrspace(1) %p, ptr addrspace(1) %q) addrspace(1) {{\n  %c = icmp {predicate} ptr addrspace(1) %p, {second}\n  %r = zext i1 %c to i16\n  ret i16 %r\n}}\n");
        assert!(text.contains(&format!("icmp {predicate} ptr addrspace(1)")), "the shape that was refused");
        listing(&text, "f")
    };
    assert_eq!(
        compare("eq", "%q"),
        ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+6]", "mov bx, word ptr [bp+8]", "xor ax, word ptr [bp+10]", "xor bx, word ptr [bp+12]", "or ax, bx", "sete al", "movzx ax, al", "pop bp", "retf"]
    );
    assert_eq!(compare("ne", "null"), ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+6]", "or ax, word ptr [bp+8]", "setne al", "movzx ax, al", "pop bp", "retf"]);
    assert!(compare("ult", "%q").iter().any(|line| line == "setb al"));
}

/// The rich route refused HIR inline assembly ("HIR asm"): examples/speaker.nib
/// did not compile. A block is a call of `llrm.ia16.asm.*`: its bytes in
/// place, each argument in the register it names, each field of the answer
/// out of one, and what it changes clobbered.
#[test]
fn test_inline_assembly_is_its_bytes_with_its_declared_registers() {
    let text = "declare {i16, i16} @llrm.ia16.asm.cd1a.ax.cx_dx.flags.n(i16)
declare void @llrm.ia16.asm.fa.-.-.-.n()
define i16 @f() addrspace(1) {
  call void @llrm.ia16.asm.fa.-.-.-.n()
  %r = call {i16, i16} @llrm.ia16.asm.cd1a.ax.cx_dx.flags.n(i16 0)
  %a = extractvalue {i16, i16} %r, 0
  %b = extractvalue {i16, i16} %r, 1
  %s = add i16 %a, %b
  ret i16 %s
}
";
    assert!(text.contains("call {i16, i16} @llrm.ia16.asm."), "the shape that was refused");
    let got = listing(text, "f");
    assert_eq!(got, ["push bp", "mov bp, sp", "L0_0:", "db 0fah", "xor ax, ax", "db 0cdh,01ah", "mov ax, cx", "add ax, dx", "pop bp", "retf"]);
}

/// The same address as a phi's input, made in the predecessor before
/// selection reaches it: refused "outside a block".
#[test]
fn test_an_address_of_no_global_can_be_a_phis_input() {
    let text = "define ptr @f(i1 %c, ptr %p) addrspace(1) {
entry:
  br i1 %c, label %a, label %b
a:
  %g = getelementptr i8, ptr null, i16 -4
  br label %b
b:
  %r = phi ptr [ %g, %a ], [ %p, %entry ]
  ret ptr %r
}
";
    assert!(text.contains("getelementptr i8, ptr null") && text.contains("phi ptr [ %g"), "the shape that was refused");
    let got = listing(text, "f");
    assert_eq!(got, ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+8]", "cmp byte ptr [bp+6], 0", "je L0_3", "L0_1:", "mov ax, -4", "L0_3:", "pop bp", "retf"]);
}

/// A huge pointer indexes by 32 bits, so a displacement that crosses 64K steps
/// the selector: the offset widened and summed, its high word shifted to the
/// machine's stride (`shl 12`) and added to the selector. Folded into the
/// access as a far one is, `es:[bx+80000]`, it wrapped (#101).
#[test]
fn test_a_huge_pointer_displacement_carries_into_its_selector() {
    let text = "define i16 @f(ptr addrspace(3) %p) addrspace(1) {
  %q = getelementptr i32, ptr addrspace(3) %p, i32 20000
  %v = load i16, ptr addrspace(3) %q
  ret i16 %v
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "movzx ebx, word ptr [bp+6]",
            "mov ax, word ptr [bp+8]",
            "add ebx, 80000",
            "mov ecx, ebx",
            "sar ecx, 16",
            "shl ecx, 12",
            "add ax, cx",
            "mov es, ax",
            "mov ax, word ptr es:[bx]",
            "pop bp",
            "retf",
        ]
    );
    let far = "define i16 @f(ptr addrspace(1) %p) addrspace(1) {
  %q = getelementptr i32, ptr addrspace(1) %p, i16 5000
  %v = load i16, ptr addrspace(1) %q
  ret i16 %v
}
";
    assert!(!listing(far, "f").iter().any(|line| line.contains("sar")), "a far pointer's displacement wraps in its offset");
}

#[test]
fn test_a_variable_index_in_a_huge_pointer_is_scaled_and_carried() {
    let text = "define i16 @f(ptr addrspace(3) %p, i32 %i) addrspace(1) {
  %q = getelementptr i16, ptr addrspace(3) %p, i32 %i
  %v = load i16, ptr addrspace(3) %q
  ret i16 %v
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "movzx ebx, word ptr [bp+6]",
            "mov ax, word ptr [bp+8]",
            "mov ecx, dword ptr [bp+10]",
            "lea ebx, [ebx+ecx*2]",
            "mov ecx, ebx",
            "sar ecx, 16",
            "shl ecx, 12",
            "add ax, cx",
            "mov es, ax",
            "mov ax, word ptr es:[bx]",
            "pop bp",
            "retf",
        ]
    );
}

/// Huge pointers are canonical, so an order is the selector's then the
/// offset's: one subtraction across both words, where a far pointer's order
/// is its offset alone.
#[test]
fn test_huge_pointers_compare_by_selector_then_offset() {
    let huge = "define i16 @f(ptr addrspace(3) %a, ptr addrspace(3) %b) addrspace(1) {
  %c = icmp ult ptr addrspace(3) %a, %b
  %z = zext i1 %c to i16
  ret i16 %z
}
";
    assert_eq!(
        listing(huge, "f"),
        ["push bp", "mov bp, sp", "L0_0:", "mov ax, word ptr [bp+6]", "mov bx, word ptr [bp+8]", "mov cx, word ptr [bp+12]", "sub ax, word ptr [bp+10]", "sbb bx, cx", "setb al", "movzx ax, al", "pop bp", "retf"]
    );
    let far = huge.replace("addrspace(3)", "addrspace(1)");
    assert!(!listing(&far, "f").iter().any(|line| line.starts_with("sbb")));
}

#[test]
fn test_a_huge_pointer_difference_is_whole_strides_and_the_offset_remainder() {
    let text = "declare i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3), ptr addrspace(3))
define i32 @f(ptr addrspace(3) %a, ptr addrspace(3) %b) addrspace(1) {
  %d = call i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3) %a, ptr addrspace(3) %b)
  ret i32 %d
}
";
    assert_eq!(
        listing(text, "f"),
        [
            "push bp",
            "mov bp, sp",
            "L0_0:",
            "movzx eax, word ptr [bp+6]",
            "movzx ebx, word ptr [bp+8]",
            "movzx edx, word ptr [bp+10]",
            "movzx ecx, word ptr [bp+12]",
            "sub eax, edx",
            "sub ebx, ecx",
            "sar ebx, 12",
            "shl ebx, 16",
            "add eax, ebx",
            "shld edx, eax, 16",
            "pop bp",
            "retf",
        ]
    );
}

#[test]
fn test_far_and_huge_pointers_cast_into_one_another_as_the_same_two_words() {
    let text = "define ptr addrspace(3) @f(ptr addrspace(1) %p) addrspace(1) {
  %h = addrspacecast ptr addrspace(1) %p to ptr addrspace(3)
  %q = getelementptr i8, ptr addrspace(3) %h, i32 70000
  ret ptr addrspace(3) %q
}
define ptr addrspace(1) @g(ptr addrspace(3) %p) addrspace(1) {
  %f = addrspacecast ptr addrspace(3) %p to ptr addrspace(1)
  ret ptr addrspace(1) %f
}
";
    let got = listing(text, "f");
    assert!(got.contains(&"add eax, 70000".to_owned()) && got.contains(&"sar ebx, 16".to_owned()), "{got:?}");
    assert_eq!(listing(text, "g"), ["push bp", "mov bp, sp", "L1_0:", "mov ax, word ptr [bp+6]", "mov dx, word ptr [bp+8]", "pop bp", "retf"]);
}

/// A loop over a huge array: its pointer carries each trip, and its end
/// compare is across both words. A constant step's carry is a mask of the
/// borrow cut to the selector's stride, not the dword shifted down and up.
#[test]
fn test_a_loop_walks_a_huge_pointer_to_its_end() {
    let text = "define i16 @f(ptr addrspace(3) %p, ptr addrspace(3) %e) addrspace(1) {
entry:
  br label %loop
loop:
  %q = phi ptr addrspace(3) [ %p, %entry ], [ %n, %loop ]
  %s = phi i16 [ 0, %entry ], [ %t, %loop ]
  %v = load i16, ptr addrspace(3) %q
  %t = add i16 %s, %v
  %n = getelementptr i16, ptr addrspace(3) %q, i32 1
  %c = icmp ult ptr addrspace(3) %n, %e
  br i1 %c, label %loop, label %done
done:
  ret i16 %t
}
";
    let got = listing(text, "f").join("\n");
    let mask = regex::Regex::new(r"sbb (\w+), (\w+)\nand (\w+), 4096").unwrap();
    let masked = mask.captures(&got).is_some_and(|one| one[1] == one[2] && one[2] == one[3]);
    assert!(masked && !got.contains("sar ") && got.contains("jb "), "{got}");
}

/// A far pointer stepped in a loop and read after it is stepped once: the
/// exit read the step remade from the old pointer, so both lived at the
/// latch and every trip copied one into the other (copy1d's windows, seven
/// instructions a trip for five).
#[test]
fn test_a_far_step_read_after_its_loop_is_made_once() {
    let text = "define i16 @f(ptr addrspace(1) %p, i16 %n) addrspace(1) {
entry:
  br label %loop
loop:
  %q = phi ptr addrspace(1) [ %p, %entry ], [ %q.next, %loop ]
  %c = phi i16 [ %n, %entry ], [ %c.next, %loop ]
  store i16 7, ptr addrspace(1) %q
  %q.next = getelementptr i8, ptr addrspace(1) %q, i16 2
  %c.next = sub i16 %c, 1
  %t = icmp ne i16 %c.next, 0
  br i1 %t, label %loop, label %done
done:
  %h = addrspacecast ptr addrspace(1) %q.next to ptr addrspace(3)
  %v = load i16, ptr addrspace(3) %h
  ret i16 %v
}
";
    let got = listing(text, "f");
    let steps = got.iter().filter(|line| line.starts_with("add ") && line.ends_with(", 2")).count();
    assert!(steps == 1 && !got.iter().any(|line| line.starts_with("jmp ")), "{got:?}");
}

/// A window over a huge pointer moves its offset's whole paragraphs into
/// the selector: the offset left is below 16, so the far pointer reaches
/// 64K less 15 bytes with no carry (`window`).
#[test]
fn test_a_window_normalizes_its_huge_pointer() {
    let text = "declare ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3))

define i16 @f(ptr addrspace(3) %p) addrspace(1) {
entry:
  %w = call ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3) %p)
  %q = getelementptr i8, ptr addrspace(1) %w, i16 300
  %v = load i16, ptr addrspace(1) %q
  ret i16 %v
}
";
    let got = listing(text, "f").join("\n");
    let normal = regex::Regex::new(r"shr (\w+), 4\n(?:.*\n)*?add \w+, (\w+)\n(?:.*\n)*?and \w+, 15").unwrap();
    assert!(normal.captures(&got).is_some_and(|one| one[1] == one[2]) && got.contains("+300]"), "{got}");
}

/// A huge pointer made of a global's address and a constant past 64K is a
/// stepped selector and the offset's remainder: the whole constant kept in
/// the offset word wrapped to another element (#101).
#[test]
fn test_a_constant_huge_pointer_past_64k_steps_the_selector() {
    let text = "@big = addrspace(1) global [4 x i32] zeroinitializer
define i32 @f() addrspace(1) {
  %v = load i32, ptr addrspace(3) getelementptr (i32, ptr addrspace(3) addrspacecast (ptr addrspace(1) @big to ptr addrspace(3)), i32 19999)
  ret i32 %v
}
";
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.starts_with("add ") && line.ends_with(&format!(", {}", (1 << 12)))), "{got:?}");
    assert!(got.iter().any(|line| line.contains("es:[bx+14460]") || line.contains("es:[bx+0x387c]") || line.contains("+14460]")), "{got:?}");
}

/// `(double)long_double` was rounded through a dword (fstp dword) whatever
/// the target: a long double narrowed to double lost 29 bits (#103). A
/// narrowing rounds to the target's width.
#[test]
fn test_a_narrowing_float_rounds_to_its_targets_width() {
    let text = "define double @f(x86_fp80 %x) addrspace(1) {\n  %d = fptrunc x86_fp80 %x to double\n  ret double %d\n}\ndefine float @g(double %x) addrspace(1) {\n  %d = fptrunc double %x to float\n  ret float %d\n}\n";
    assert!(text.contains("fptrunc x86_fp80"), "the shape that was rounded to a float");
    assert!(listing(text, "f").contains(&"fstp qword ptr [bp-8]".to_owned()), "{:?}", listing(text, "f"));
    assert!(listing(text, "g").iter().any(|line| line.starts_with("fstp dword")), "{:?}", listing(text, "g"));
}

/// A long double is compared from a register: fcom has no 10-byte memory
/// operand, and `fcomp tbyte ptr` could not be encoded (#103).
#[test]
fn test_an_extended_float_is_never_fcoms_memory_operand() {
    let text = "@g = global x86_fp80 zeroinitializer\ndefine i16 @f(x86_fp80 %x) addrspace(1) {\n  %y = load x86_fp80, ptr @g\n  %c = fcmp ogt x86_fp80 %x, %y\n  %r = zext i1 %c to i16\n  ret i16 %r\n}\n";
    assert!(text.contains("fcmp ogt x86_fp80"), "the shape that was refused");
    let got = listing(text, "f");
    assert!(got.iter().any(|line| line.starts_with("fld tbyte")) && got.iter().any(|line| line.starts_with("fcom")), "{got:?}");
    assert!(!got.iter().any(|line| line.starts_with("fcom") && line.contains("tbyte")), "{got:?}");
}

/// A fixed-address pointer is a far one to the machine: the same code as
/// space 1 for the same access, whatever the analysis makes of it.
#[test]
fn test_a_fixed_address_pointer_selects_as_a_far_one() {
    let body = |space: u32| {
        format!("define i16 @f(ptr addrspace({space}) %p, i16 %i) addrspace(1) {{\n  %q = getelementptr i16, ptr addrspace({space}) %p, i16 %i\n  %v = load volatile i16, ptr addrspace({space}) %q\n  ret i16 %v\n}}\n")
    };
    let layout = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16\"\n";
    let listing = |space| {
        let module = llrm_mir::parse::module(&format!("{layout}{}", body(space))).expect("parses");
        let chosen = isel::selected(&module, "f", &qb(), &mut Pool::new(0), crate::backend::cpu::profile("486").expect("a target"), &crate::backend::target::BASIC, isel::code16(), &llrm_x86_code16::Code16, false, 0).expect("selected");
        format!("{chosen:?}")
    };
    assert_eq!(listing(4), listing(1));
}

/// A scope's lifetime markers are not code: a body with them selects as the
/// same body without. They were refused: `@llvm.lifetime.start.p0`.
#[test]
fn test_lifetime_markers_are_no_code() {
    let text = |markers: bool| {
        let (start, end) = if markers { ("call void @llvm.lifetime.start.p0(i64 16, ptr %x)", "call void @llvm.lifetime.end.p0(i64 16, ptr %x)") } else { ("", "") };
        format!(
            "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %a, i16 %c) addrspace(1) {{
  %x = alloca [8 x i16]
  {start}
  %p = getelementptr inbounds [8 x i16], ptr %x, i16 0, i16 %c
  store volatile i16 %a, ptr %p
  %v = load volatile i16, ptr %p
  {end}
  ret i16 %v
}}
"
        )
    };
    assert_eq!(listing(&text(true), "f"), listing(&text(false), "f"));
}

/// Two 16-byte locals, `x` and `y`: in the two arms of an `if`, or, `overlapping`, both
/// live across the same stores; with lifetime markers when `markers`.
fn scopes(markers: bool, overlapping: bool) -> String {
    let mark = |what: &str, name: &str| if markers { format!("call void @llvm.lifetime.{what}.p0(i64 16, ptr %{name})") } else { String::new() };
    let (start, end, ystart, yend) = (mark("start", "x"), mark("end", "x"), mark("start", "y"), mark("end", "y"));
    if overlapping {
        format!(
            "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %a, i16 %c) addrspace(1) {{
  %x = alloca [8 x i16]
  %y = alloca [8 x i16]
  {start}
  {ystart}
  %p = getelementptr inbounds [8 x i16], ptr %x, i16 0, i16 %c
  store volatile i16 %a, ptr %p
  %q = getelementptr inbounds [8 x i16], ptr %y, i16 0, i16 %c
  store volatile i16 %c, ptr %q
  %v = load volatile i16, ptr %p
  %w = load volatile i16, ptr %q
  {end}
  {yend}
  %r = add i16 %v, %w
  ret i16 %r
}}
"
        )
    } else {
        format!(
            "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %a, i16 %c) addrspace(1) {{
  %x = alloca [8 x i16]
  %y = alloca [8 x i16]
  %t = icmp sgt i16 %a, 0
  br i1 %t, label %b1, label %b2

b1:
  {start}
  %p = getelementptr inbounds [8 x i16], ptr %x, i16 0, i16 %c
  store volatile i16 %a, ptr %p
  %v = load volatile i16, ptr %p
  {end}
  ret i16 %v

b2:
  {ystart}
  %q = getelementptr inbounds [8 x i16], ptr %y, i16 0, i16 %c
  store volatile i16 %c, ptr %q
  %w = load volatile i16, ptr %q
  {yend}
  ret i16 %w
}}
"
        )
    }
}

/// The frame a listing reserves: `sub sp, N`.
fn frame_bytes(text: &str) -> i64 {
    let listing = listing(text, "f");
    listing.iter().find_map(|line| line.strip_prefix("sub sp, ")?.parse().ok()).unwrap_or(0)
}

/// Block locals nothing keeps live together share a slot; each had its own, so a function
/// of sibling scopes reserved the sum of them.
#[test]
fn test_block_locals_with_disjoint_lifetimes_share_a_frame_slot() {
    assert_eq!(frame_bytes(&scopes(false, false)), 32);
    assert_eq!(frame_bytes(&scopes(true, false)), 16);
}

/// Locals live together do not: the markers say they overlap.
#[test]
fn test_block_locals_live_together_keep_their_own_slots() {
    assert_eq!(frame_bytes(&scopes(true, true)), 32);
}

/// A local read after its lifetime ended is not one the markers can speak for: it keeps
/// its own slot, whatever shares around it.
#[test]
fn test_a_local_used_outside_its_lifetime_shares_no_slot() {
    let text = scopes(true, false).replace("  ret i16 %v\n", "  %again = load volatile i16, ptr %p\n  ret i16 %again\n");
    assert_eq!(frame_bytes(&text), 32);
}

/// After a second return from `setjmp` a slot another local used holds that local's value:
/// in a function that calls a routine returning twice, no slot is shared, whatever the
/// markers say.
#[test]
fn test_no_slot_is_shared_in_a_function_that_calls_setjmp() {
    let text = scopes(true, false).replace("define i16 @f(i16 %a, i16 %c) addrspace(1) {", "declare i16 @setjmp(i16) returns_twice\ndefine i16 @f(i16 %a, i16 %c) addrspace(1) {").replace("  %t = icmp sgt i16 %a, 0\n", "  %j = call i16 @setjmp(i16 %a)\n  %t = icmp sgt i16 %a, 0\n");
    assert_eq!(frame_bytes(&text), 32);
}

/// The flag reaches the allocator: the assembled procedure's body still says it calls `setjmp`.
#[test]
fn test_a_body_that_calls_setjmp_says_so_to_the_allocator() {
    let text = "declare i16 @setjmp(i16) returns_twice
define i16 @f(i16 %a) addrspace(1) {
  %j = call i16 @setjmp(i16 %a)
  ret i16 %j
}
define i16 @g(i16 %a) addrspace(1) {
  ret i16 %a
}
";
    let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Name("486"), &crate::backend::target::BASIC).expect("assembles");
    let says = |name: &str| module.procedures.iter().find(|one| one.name.contains(name)).map(|one| one.body.returns_twice);
    assert_eq!((says("f"), says("g")), (Some(true), Some(false)));
}

/// The encoded object of `fixture` at -Os when `machined` tries only `candidates`.
fn sized_with(candidates: assemble::Candidates, text: &str) -> usize {
    assemble::trying(candidates, || {
        let profile = crate::backend::cpu::tuned("486", true).expect("the 486 profile");
        let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Profile(profile), &crate::backend::target::BASIC).expect("assembles");
        crate::backend::objbuild::written_as(&module, "t.asm", crate::backend::objbuild::CodeLayout::OneSegment).expect("encodes").len()
    })
}

/// The spiller decides on the general registers alone; where the allocator's pressure is elsewhere
/// its spill code came on top of the allocator's own (QCport d_alias at -Os: 15723 bytes, 16811
/// with it, +7%). The function costs what the cheaper route costs.
#[test]
fn test_a_function_the_spiller_makes_larger_is_built_without_it() {
    let text = std::fs::read_to_string(concat!(env!("LLRM_ROOT"), "/tests/check/mir/matmul.ll")).unwrap();
    let (spiller, allocator) = (sized_with(assemble::Candidates::SpillerOnly, &text), sized_with(assemble::Candidates::AllocatorOnly, &text));
    assert!(spiller > allocator, "premise: the spiller's route is larger ({spiller} against {allocator})");
    assert_eq!(sized_with(assemble::Candidates::Both, &text), allocator);
}

/// A memcpy past the unrolled moves is `rep movsd` through es:di, the source
/// read through ss as an override and the tail by `movsw`: a refusal failed
/// every program with a copy that long.
#[test]
fn test_a_long_memcpy_is_a_string_move() {
    let text = |size: &str| {
        format!(
            "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f(i16 %n) addrspace(1) {{
  %a = alloca [70 x i8]
  %b = alloca [70 x i8]
  store i16 3, ptr %a
  call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 {size}, i1 false)
  %v = load i16, ptr %b
  ret i16 %v
}}
"
        )
    };
    let moves = |size: &str| listing(&text(size), "f").into_iter().filter(|one| one.contains("movs") || one.starts_with("shr") || one.starts_with("and") || one.starts_with("mov cx")).collect::<Vec<_>>();
    assert_eq!(
        moves("70"),
        [
            "mov cx, 17",
            "rep movs dword ptr es:[di], dword ptr ss:[si]",
            "movs word ptr es:[di], word ptr ss:[si]",
        ]
    );
    // A length not constant is the dwords it holds and the bytes it leaves.
    let dynamic = moves("%n");
    assert!(dynamic.contains(&"shr cx, 2".to_owned()) && dynamic.contains(&"and ax, 3".to_owned()), "{dynamic:?}");
    assert_eq!(dynamic.iter().filter(|one| one.starts_with("rep movs")).count(), 2, "{dynamic:?}");
}

/// A near destination's selector was made ahead of the count's shift and mask, so
/// it was live across them and took a general register: QCport's console.c grew
/// a spill. It is made just before the first move.
#[test]
fn test_a_near_selector_is_made_after_the_count_is_prepared() {
    let text = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define i16 @f(i16 %n) addrspace(1) {
  %a = alloca [70 x i8]
  %b = alloca [70 x i8]
  store i16 3, ptr %a
  call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 %n, i1 false)
  %v = load i16, ptr %b
  ret i16 %v
}
";
    let lines = listing(text, "f");
    let at = |found: &dyn Fn(&String) -> bool| lines.iter().position(found).unwrap_or_else(|| panic!("{lines:?}"));
    let (counted, selected, moved) = (at(&|one| one.starts_with("and ")), at(&|one| one.ends_with(", ss")), at(&|one| one.starts_with("rep movs")));
    assert!(counted < selected && selected < moved, "{lines:?}");
}

/// A far source is read through fs, loaded with its selector.
#[test]
fn test_a_long_memcpy_from_a_far_pointer_reads_through_fs() {
    let text = "declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
define i16 @f(ptr addrspace(1) %p) addrspace(1) {
  %b = alloca [70 x i8]
  call void @llvm.memcpy.p0.p1.i16(ptr %b, ptr addrspace(1) %p, i16 70, i1 false)
  %v = load i16, ptr %b
  ret i16 %v
}
";
    let lines = listing(text, "f");
    assert!(lines.iter().any(|one| one.starts_with("lfs ") || one.starts_with("mov fs, ")), "{lines:?}");
    assert!(lines.contains(&"rep movs dword ptr es:[di], dword ptr fs:[si]".to_owned()), "{lines:?}");
}

/// `@f` of forty volatile stores to `@g` that a branch at its entry may
/// skip: more than a short branch reaches.
fn far_branch_body() -> String {
    let stores: String = (0..40).map(|at| format!("  %p{at} = getelementptr i8, ptr @g, i16 {}\n  store volatile i16 {at}, ptr %p{at}\n", 2 * at)).collect();
    format!("@g = global [80 x i8] zeroinitializer\ndefine i16 @f(i16 %c) addrspace(1) {{\nentry:\n  %z = icmp eq i16 %c, 0\n  br i1 %z, label %skip, label %body\nbody:\n{stores}  br label %skip\nskip:\n  ret i16 %c\n}}\n")
}

/// The code segment of `text`'s object, decoded, with the offsets its fixups patch.
fn decoded_object(text: &str) -> (Vec<iced_x86::Instruction>, Vec<usize>, Vec<usize>) {
    use llrm_omf::omf;
    let module = assemble::assembled(&parsed(text), &qb(), "T_TEXT", ProfileOrName::Name("486"), &crate::backend::target::BASIC).expect("assembles");
    let bytes = crate::backend::objbuild::written_as(&module, "t.asm", crate::backend::objbuild::CodeLayout::OneSegment).expect("encodes");
    let records = omf::parse(&bytes).expect("parses");
    let (code, _, size) = omf::code_segment(&records).expect("a code segment");
    let image = omf::segment_image(&records, code, size);
    let mut decoder = iced_x86::Decoder::with_ip(16, &image, 0, iced_x86::DecoderOptions::NONE);
    let (mut insns, mut fields) = (Vec::new(), Vec::new());
    while decoder.can_decode() {
        let insn = decoder.decode();
        let offsets = decoder.get_constant_offsets(&insn);
        if offsets.has_displacement() {
            fields.push(insn.ip() as usize + offsets.displacement_offset());
        }
        insns.push(insn);
    }
    let fixed = omf::fixups(&records).into_iter().filter(|one| one.seg == Some(code)).map(|one| one.offset as usize).collect();
    (insns, fields, fixed)
}

/// A branch past a short branch's reach is encoded near and lands on its
/// target, the code it skips intact. Replaces layout_tests'
/// `test_fallthrough_relaxation_preserves_targets_and_intervening_data`, which
/// checked relaxation through BC's raise.
#[test]
fn test_a_branch_past_a_short_reach_grows_and_lands_on_its_target() {
    let (insns, _, _) = decoded_object(&far_branch_body());
    assert!(!insns.iter().any(iced_x86::Instruction::is_invalid));
    let starts: Vec<u64> = insns.iter().map(iced_x86::Instruction::ip).collect();
    let branch = insns.iter().find(|one| one.flow_control() == iced_x86::FlowControl::ConditionalBranch).expect("the entry's branch");
    assert_eq!(branch.op0_kind(), iced_x86::OpKind::NearBranch16);
    assert!(branch.len() > 2, "a short branch cannot reach: {branch}");
    let target = branch.near_branch_target();
    assert!(target - branch.next_ip() > 127, "{branch} skips {} bytes", target - branch.next_ip());
    assert!(starts.contains(&target), "{branch} lands inside an instruction");
    let skipped = insns.iter().filter(|one| one.ip() >= branch.next_ip() && one.ip() < target);
    assert_eq!(skipped.filter(|one| one.mnemonic() == iced_x86::Mnemonic::Mov && one.op0_kind() == iced_x86::OpKind::Memory).count(), 40);
}

/// Every relocated field is patched where its instruction landed, after the
/// branch before it grew. Replaces layout_tests'
/// `test_a_moved_operation_keeps_its_fixup`, which checked it through BC's raise.
#[test]
fn test_every_relocated_field_moves_with_its_instruction() {
    let (_, fields, fixed) = decoded_object(&far_branch_body());
    assert_eq!(fixed.len(), 40, "{fixed:?}");
    assert!(fixed.iter().all(|at| fields.contains(at)), "fixups {fixed:?}, fields {fields:?}");
}

/// Two adjacent volatile word stores are two word stores: the width of a
/// device access is its behaviour. storecombine paired them into one dword,
/// as it rightly pairs plain ones (#319).
#[test]
fn test_adjacent_volatile_word_stores_stay_two_word_stores() {
    let stores = |volatile: &str| {
        let text = format!("@g = global [4 x i8] zeroinitializer\ndefine void @f() addrspace(1) {{\n  store {volatile}i16 1, ptr @g\n  %p = getelementptr i8, ptr @g, i16 2\n  store {volatile}i16 2, ptr %p\n  ret void\n}}\n");
        listing(&text, "f").into_iter().filter(|line| line.starts_with("mov ")).collect::<Vec<_>>()
    };
    let volatile = stores("volatile ");
    assert_eq!(volatile.len(), 2, "{volatile:?}");
    assert!(volatile.iter().all(|line| line.starts_with("mov word ptr")), "{volatile:?}");
    let plain = stores("");
    assert_eq!(plain.len(), 1, "{plain:?}");
    assert!(plain[0].starts_with("mov dword ptr"), "{plain:?}");
}

/// A volatile load is not delayed past another volatile access: the
/// peephole folded `mov r,[g]` into its consumer after the read of `h`, so
/// a device saw `h` read before `g` (`sub`, `add [g]` and `push [g]` alike).
#[test]
fn test_a_volatile_load_is_not_folded_past_another_volatile_access() {
    let globals = "@g = global [2 x i8] zeroinitializer\n@h = global [2 x i8] zeroinitializer\ndeclare void @k(i16, i16) addrspace(1)\n";
    for body in [
        "define i16 @f() addrspace(1) {\n  %a = load volatile i16, ptr @g\n  %b = load volatile i16, ptr @h\n  %s = sub i16 %b, %a\n  ret i16 %s\n}\n",
        "define i16 @f() addrspace(1) {\n  %a = load volatile i16, ptr @g\n  %b = load volatile i16, ptr @h\n  %s = add i16 %a, 3\n  store volatile i16 %s, ptr @g\n  ret i16 %b\n}\n",
        "define void @f() addrspace(1) {\n  %a = load volatile i16, ptr @g\n  %b = load volatile i16, ptr @h\n  call addrspace(1) void @k(i16 %b, i16 %a)\n  ret void\n}\n",
    ] {
        let lines = listing(&format!("{globals}{body}"), "f");
        let first = |name: &str| lines.iter().position(|line| line.contains(&format!("word ptr {name}"))).unwrap_or_else(|| panic!("no {name}: {lines:?}"));
        assert!(first("g") < first("h"), "{lines:?}");
    }
}

/// A pointer into the stack segment is a word read and written through `ss:`, whatever DS holds:
/// the segment is the space's, not DGROUP's.
#[test]
fn a_near_stack_pointer_is_read_and_written_through_ss() {
    let body = listing(
        "define i16 @get(ptr addrspace(5) %p, i16 %i) {
b0:
  %a = getelementptr i16, ptr addrspace(5) %p, i16 %i
  %v = load i16, ptr addrspace(5) %a
  store i16 %i, ptr addrspace(5) %p
  ret i16 %v
}
",
        "get",
    );
    let text = body.join("\n");
    assert!(text.contains("ss:[") && !text.contains("es:[") && !text.contains("ds:["), "{text}");
}

/// A stack pointer made far is SS and its offset.
#[test]
fn a_near_stack_pointer_made_far_has_ss_for_its_selector() {
    let body = listing(
        "define ptr addrspace(1) @far(ptr addrspace(5) %p) {
b0:
  %w = addrspacecast ptr addrspace(5) %p to ptr addrspace(1)
  ret ptr addrspace(1) %w
}
",
        "far",
    );
    assert!(body.iter().any(|line| line.contains(", ss")), "{body:?}");
}

/// catalog.nib's `find` at -Os: a loop with one phi too many for the registers. Against a register phi stored once at
/// the top, a memory phi stores on both in-edges: the object grew by 6 bytes (#491). The price keeps the register phi.
#[test]
fn test_a_phi_stored_on_more_edges_than_its_block_runs_stays_in_a_register() {
    let text = std::fs::read_to_string(concat!(env!("LLRM_ROOT"), "/tests/check/mir/findloop.ll")).unwrap();
    let with = sized_with(assemble::Candidates::SpillerOnly, &text);
    let without = crate::backend::ssaspill::without_memory_phis(|| sized_with(assemble::Candidates::SpillerOnly, &text));
    assert_eq!(with, without);
}

/// code16 is 386+ code under 66h/67h prefixes, not 8086 code: i32 is
/// arithmetic in EAX..EDI, extends are `movsx`/`movzx` into 32-bit
/// registers, a constant multiply is a 32-bit `lea`, a long copy is
/// `rep movsd`, and on a 386 a scaled index is `[ebx+eax*2]` (the 486 prices
/// that form above a spill). A selector rewrite that keeps today's bytes
/// but narrows these to word pairs would lose what every program is priced on.
/// There is no 286 profile yet: when there is, it asserts none of these.
#[test]
fn test_code16_emits_386_forms() {
    let arithmetic = listing("define i32 @f(i32 %a, i32 %b) addrspace(1) {\n  %c = add i32 %a, %b\n  %d = mul i32 %c, 3\n  ret i32 %d\n}\n", "f");
    assert!(arithmetic.contains(&"add ebx, dword ptr [bp+10]".to_owned()), "{arithmetic:?}");
    assert!(arithmetic.contains(&"lea eax, [ebx+ebx*2]".to_owned()), "{arithmetic:?}");

    let extends = listing("define i32 @f(i8 %a, i16 %b) addrspace(1) {\n  %x = sext i8 %a to i32\n  %y = zext i16 %b to i32\n  %z = add i32 %x, %y\n  ret i32 %z\n}\n", "f");
    for want in ["movsx eax, byte ptr [bp+6]", "movzx ebx, word ptr [bp+8]", "add eax, ebx"] {
        assert!(extends.contains(&want.to_owned()), "{want}: {extends:?}");
    }

    let copy = listing(
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\ndefine i16 @f() addrspace(1) {\n  %a = alloca [70 x i8]\n  %b = alloca [70 x i8]\n  store i16 3, ptr %a\n  call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 70, i1 false)\n  %v = load i16, ptr %b\n  ret i16 %v\n}\n",
        "f",
    );
    assert!(copy.iter().any(|one| one.starts_with("rep movs dword ptr")), "{copy:?}");

    let scaled = |cpu: &str| {
        listing_on(
            cpu,
            "define i16 @f(ptr %p, ptr %q) addrspace(1) {\nentry:\n  %i = load i16, ptr %q\n  %b = load ptr, ptr %p\n  %c = icmp sge i16 %i, 0\n  br i1 %c, label %ok, label %no\nok:\n  %e = getelementptr inbounds i16, ptr %b, i16 %i\n  %v = load i16, ptr %e, !tbaa !1\n  ret i16 %v\nno:\n  ret i16 0\n}\n\n!0 = !{!\"int\"}\n!1 = !{!0, !0, i64 0}\n",
            "f",
        )
    };
    assert!(scaled("386").contains(&"mov ax, word ptr [ebx+eax*2]".to_owned()), "{:?}", scaled("386"));
    assert!(!scaled("486").iter().any(|one| one.contains("*2")), "{:?}", scaled("486"));
}

/// Selectors are generated per target directory and found by its name: the
/// 16-bit one is there, a target nobody has described is not.
#[test]
fn test_a_selector_is_found_by_its_targets_name() {
    let found = isel::selector("x86-code16").expect("the 16-bit x86 selector");
    assert_eq!(found.name, "x86-code16");
    assert!(std::ptr::eq(found, isel::code16()));
    assert!(isel::selector("x86-code99").is_none());
}

/// A type's class, its register width and its size in memory are read off one
/// classification: the patterns' `ptr`/`far`, the register a pointer takes and the bytes
/// it stores can not disagree about whether a pointer is one value or two.
#[test]
fn test_one_class_of_a_type_says_its_name_register_and_size() {
    let params = "i1 %a, i8 %b, i16 %c, i32 %d, i64 %e, ptr %p, ptr addrspace(1) %q, float %x, double %y, x86_fp80 %z";
    let check = |layout: &str, want: &[(&str, Result<u32, ()>, Result<u32, ()>)]| {
        let module = llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\ndefine void @f({params}) {{\nentry:\n  ret void\n}}\n")).expect("parses");
        let function = module.global(module.named("f").expect("f")).function().expect("a function");
        let layout = llrm_mir::datalayout::DataLayout::parse(layout).expect("a layout");
        for (&parameter, (name, width, size)) in function.parameters().iter().zip(want) {
            let ty = function.value(parameter).ty;
            assert_eq!(isel::TypeClass::of(&module.context.types, &layout, Some(ty)).name(), *name);
            assert_eq!(isel::width_of(&module, &layout, ty).map_err(|_| ()), *width, "{name} width");
            assert_eq!(isel::size_of(&module, &layout, ty).map_err(|_| ()), *size, "{name} size");
        }
    };
    check(
        "e-p:16:16-p1:32:16:16:16-i32:16-i64:16",
        &[("i1", Ok(1), Ok(1)), ("i8", Ok(1), Ok(1)), ("i16", Ok(2), Ok(2)), ("i32", Ok(4), Ok(4)), ("i64", Err(()), Ok(8)), ("ptr", Ok(2), Ok(2)), ("far", Err(()), Ok(4)), ("float", Ok(10), Ok(4)), ("float", Ok(10), Ok(8)), ("float", Ok(10), Ok(10))],
    );
    check(
        "e-p:32:32-p1:32:32-i32:32-i64:32",
        &[("i1", Ok(1), Ok(1)), ("i8", Ok(1), Ok(1)), ("i16", Ok(2), Ok(2)), ("i32", Ok(4), Ok(4)), ("i64", Err(()), Ok(8)), ("ptr", Ok(4), Ok(4)), ("ptr", Ok(4), Ok(4)), ("float", Ok(10), Ok(4)), ("float", Ok(10), Ok(8)), ("float", Ok(10), Ok(10))],
    );
}

/// A pointer of 32 bits reaches `[base+index*4]` as it is, with no proof and no widening:
/// the sum is as wide as the pointer, and its base a dword already. Selected for the
/// 16-bit target's profile it was `shl index,2` and an access through `[base+index]`: the
/// fold wanted a word range for the index and a word base to widen.
#[test]
fn test_a_dword_pointer_scales_its_index_in_the_access() {
    let text = "target datalayout = \"e-p:32:32-i32:32-i64:32\"\ndefine i32 @f(ptr %p, i32 %i) {\nentry:\n  %e = getelementptr inbounds i32, ptr %p, i32 %i\n  %v = load i32, ptr %e\n  ret i32 %v\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let selected = isel::selected(&module, "f", &qb(), &mut Pool::new(0), crate::backend::cpu::profile("486").expect("a target"), &crate::backend::target::BASIC, isel::code16(), &llrm_x86_code16::Code16, false, 0).expect("selects");
    let insns: Vec<_> = selected.body.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|insn| insn.what.as_ref()).collect();
    assert!(!insns.iter().any(|what| what.name.as_deref() == Some("shl")), "{insns:?}");
    let scaled = insns.iter().any(|what| what.sources.iter().any(|one| matches!(one, crate::model::ir::Loc::Mem(cell) if cell.scale == 4 && cell.base.is_some() && cell.index.is_some())));
    assert!(scaled, "{insns:?}");
}
