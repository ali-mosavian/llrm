//! The calls the idiom pass forms: `IRBuilder::CreateMemSet`,
//! `CreateMemCpy`, `CreateMemMove` and the `llvm.experimental.memset.pattern`
//! intrinsic, placed before an instruction. The callee is declared through
//! [`Declared`] where the module has none (`getOrInsertDeclaration`).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, CallInfo, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::{Type, TypeId};

/// Which of the two copies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Copy {
    Memcpy,
    Memmove,
}

fn space_of(
    context: &Context,
    function: &Function,
    pointer: Operand,
) -> u32 {
    let ty = function.operand_type(context, pointer).expect("a pointer has a type");
    match context.types.get(ty) {
        Type::Pointer(space) => *space,
        other => panic!("a pointer, not {other:?}"),
    }
}

fn width_of(
    context: &Context,
    function: &Function,
    length: Operand,
) -> u32 {
    let ty = function.operand_type(context, length).expect("a length has a type");
    context.types.int_bits(ty).expect("an integer length")
}

fn int(
    context: &Context,
    width: u32,
    value: u128,
) -> Operand {
    let ty = context.types.int(width);
    Operand::Constant(context.int(ty, value as i128))
}

fn align(value: Option<u64>) -> Vec<Attribute> {
    value.map(|bytes| Attribute::Int("align".to_owned(), bytes)).into_iter().collect()
}

/// A call of `name` (type `ty`) with `arguments`, placed before `before`.
fn call(
    context: &Context,
    declared: &mut Declared,
    function: &mut Function,
    before: InstId,
    name: &str,
    ty: TypeId,
    arguments: Vec<Operand>,
    argument_attrs: Vec<Vec<Attribute>>,
) -> InstId {
    let callee = declared.declare(name, ty);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let void = context.types.void();
    let info = CallInfo {
        function_type: ty,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs,
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let mut operands = arguments;
    operands.push(callee);
    let inst = function.create_instruction(Opcode::Call(Box::new(info)), void, operands, Flags::default(), None);
    function.insert(inst, Position::Before(before)).expect("a placed call");
    inst
}

/// The suffix an overloaded intrinsic's name takes for `ty`.
fn mangled(
    context: &Context,
    ty: TypeId,
) -> String {
    match context.types.get(ty) {
        Type::Int(bits) => format!("i{bits}"),
        Type::Pointer(space) => format!("p{space}"),
        Type::Float(kind) => match kind {
            llrm_mir::types::FloatKind::Float => "f32".to_owned(),
            llrm_mir::types::FloatKind::Double => "f64".to_owned(),
            llrm_mir::types::FloatKind::X86Fp80 => "f80".to_owned(),
        },
        other => panic!("a pattern of an int, float or pointer, not {other:?}"),
    }
}

/// `CreateMemSet(Ptr, Val, Size, Align, isVolatile=false)`.
pub fn create_mem_set(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    before: InstId,
    dest: Operand,
    value: Operand,
    length: Operand,
    dest_align: Option<u64>,
) -> InstId {
    let space = space_of(context, function, dest);
    let width = width_of(context, function, length);
    let types = &mut context.types;
    let (void, pointer, byte, count, flag) =
        (types.void(), types.ptr(space), types.int(8), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, byte, count, flag], variadic: false });
    let volatile = int(context, 1, 0);
    call(
        context,
        declared,
        function,
        before,
        &format!("llvm.memset.p{space}.i{width}"),
        ty,
        vec![dest, value, length, volatile],
        vec![align(dest_align), Vec::new(), Vec::new(), Vec::new()],
    )
}

/// `CreateIntrinsic(experimental_memset_pattern, {Ptr, Pattern type, Size
/// type}, {Ptr, Pattern, Count, false})`; the pattern is a constant.
pub fn create_mem_set_pattern(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    before: InstId,
    dest: Operand,
    pattern: Operand,
    count: Operand,
    dest_align: Option<u64>,
) -> InstId {
    let space = space_of(context, function, dest);
    let width = width_of(context, function, count);
    let cell = function.operand_type(context, pattern).expect("a typed pattern");
    let bits = mangled(context, cell);
    let types = &mut context.types;
    let (void, pointer, counts, flag) = (types.void(), types.ptr(space), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, cell, counts, flag], variadic: false });
    let volatile = int(context, 1, 0);
    call(
        context,
        declared,
        function,
        before,
        &format!("llvm.experimental.memset.pattern.p{space}.{bits}.i{width}"),
        ty,
        vec![dest, pattern, count, volatile],
        vec![align(dest_align), Vec::new(), Vec::new(), Vec::new()],
    )
}

/// `CreateMemCpy` or `CreateMemMove(Dst, DstAlign, Src, SrcAlign, Size,
/// isVolatile=false)`.
pub fn create_mem_copy(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    before: InstId,
    how: Copy,
    dest: Operand,
    dest_align: Option<u64>,
    source: Operand,
    source_align: Option<u64>,
    length: Operand,
) -> InstId {
    let (to, from) = (space_of(context, function, dest), space_of(context, function, source));
    let width = width_of(context, function, length);
    let types = &mut context.types;
    let (void, destination, src, count, flag) =
        (types.void(), types.ptr(to), types.ptr(from), types.int(width), types.int(1));
    let ty = types.intern(Type::Function {
        returns: void,
        parameters: vec![destination, src, count, flag],
        variadic: false,
    });
    let name = if how == Copy::Memcpy { "memcpy" } else { "memmove" };
    let volatile = int(context, 1, 0);
    call(
        context,
        declared,
        function,
        before,
        &format!("llvm.{name}.p{to}.p{from}.i{width}"),
        ty,
        vec![dest, source, length, volatile],
        vec![align(dest_align), align(source_align), Vec::new(), Vec::new()],
    )
}

#[cfg(test)]
mod tests {
    use llrm_mir::passes::Declared;

    use super::*;

    /// The calls formed carry no `!tbaa` (D10, #2314): TBAA answers may-alias
    /// for an untagged access, so the call is more conservative than LLVM's,
    /// never less.
    #[test]
    fn test_a_formed_call_carries_no_metadata() {
        let mut module = llrm_mir::parse::module(
            "define void @f(ptr %p, ptr %q, i64 %n) {
  store i8 0, ptr %p, !tbaa !0
  ret void
}

!0 = !{!1, !1, i64 0}
!1 = !{!\"char\", !2, i64 0}
!2 = !{!\"root\"}
",
        )
        .expect("a module");
        let mut declared = Declared::of(&module);
        let mut context = module.context.clone();
        let function = module
            .globals
            .iter_mut()
            .find_map(|global| match &mut global.kind {
                llrm_mir::module::GlobalKind::Function(function) => Some(function),
                _ => None,
            })
            .expect("a function");
        let before = function.layout()[0];
        let store = function.block(before).first().expect("a store");
        assert!(function.instruction(store).metadata.iter().any(|(kind, _)| kind == "tbaa"), "the store is tagged");
        let [p, q, n] = function.parameters()[..] else { panic!("three parameters") };
        let byte = int(&mut context, 8, 0);
        let set = create_mem_set(
            &mut context,
            &mut declared,
            function,
            store,
            Operand::Value(p),
            byte,
            Operand::Value(n),
            None,
        );
        let copy = create_mem_copy(
            &mut context,
            &mut declared,
            function,
            store,
            Copy::Memmove,
            Operand::Value(p),
            None,
            Operand::Value(q),
            None,
            Operand::Value(n),
        );
        assert!(function.instruction(set).metadata.is_empty());
        assert!(function.instruction(copy).metadata.is_empty());
    }
}
