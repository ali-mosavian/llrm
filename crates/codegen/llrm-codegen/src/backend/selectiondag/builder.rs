//! `SelectionDAGBuilder`: one block of MIR as a DAG, for the scalar integer
//! subset (arithmetic, casts, `icmp`, `select`, `br`, `load`, `store`,
//! `getelementptr`, and the register copies a `phi` needs).

use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::HashMap;

mod aggregates;
mod asmcall;
mod codecall;
mod mathops;
mod memops;
mod mergedcond;
mod switch;
mod tailcall;

use llrm_lir::registers::RegId;

use super::dag::{DagConfig, MemBase, MemInfo, SDNodeFlags, SDValue, SelectionDAG};
use super::fold::mask;
use super::function_lowering::FunctionLoweringInfo;
use super::isd::{CondCode, ISD};
use super::mvt::{EVT, MVT};
use crate::backend::isel::{Convention, Parameter, convention_for};

/// An instruction the builder does not translate yet; the shadow harness
/// counts these per reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused(pub &'static str);

/// Where a runtime routine leaves its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeAnswer {
    /// The registers its convention names.
    Registers,
    /// The flags, as a compare of the answer with zero leaves them
    /// (`Contract::flags_result`).
    Flags,
}

/// The target's cost model (`TTI` in the pin), held where the builder is
/// debug-printed.
#[derive(Clone)]
pub struct TtiHandle(pub std::rc::Rc<dyn llrm_mir::tti::Tti>);

impl std::fmt::Debug for TtiHandle {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.write_str("Tti")
    }
}

/// The target facts the builder asks (`TLI->` in the pin).
#[derive(Clone, Debug)]
pub struct BuilderTarget {
    /// The cost model `shouldKeepJumpConditionsTogether` prices with; none:
    /// conditions stay together.
    pub tti: Option<TtiHandle>,
    pub dag: DagConfig,
    /// `getShiftAmountTy`: x86 shifts by `i8`.
    pub shift_amount: EVT,
    /// The integer widths with a register class, ascending.
    pub legal_ints: Vec<u32>,
    /// The library routines the target's runtime has, each with the calling
    /// convention and the symbol a call of it uses (`options.libcalls`); none
    /// given, the names its `c.toml` states.
    pub libcalls: Option<Vec<(String, u32, String)>>,
    /// The stack alignment `alloca` rounds to, when not the target's own: the
    /// pin's i486-linux goldens say 16.
    pub stack_align: Option<u64>,
    /// Sibling calls become `TC_RETURN`s (tail-call step 3); off until the
    /// emitter lays them (step 5).
    pub tail_calls: bool,
    /// `-fexcess-precision=standard`: an integer too wide for its float type's
    /// mantissa is rounded to that type (x87 would keep it exact).
    pub float_store: bool,
    /// A dense switch may be a jump table: the target prices one (the MIR pass
    /// reads the same cost).
    pub jump_tables: bool,
    /// The function is tuned for size (`-Os`, `optsize`, `minsize`: the pin's
    /// `hasOptSize`): a jump table needs 40% density.
    pub opt_size: bool,
    /// The build is for the least size (`-Oz`, the pin's `minsize`).
    pub min_size: bool,
    /// At size, the most stores a constant memset is inline for; 0 where the
    /// pin's limit stands. Real mode: a dword store is 4 bytes (prefix, opcode,
    /// ModRM, disp8) against 13 for the fill (`lea di`, `mov es,ss`, `mov cx`,
    /// `rep stos`, the zero) that also takes ax, cx and di from the code
    /// around it, so four.
    pub memset_stores_size: usize,
    /// The same for the load and store pairs of a constant memcpy: a dword
    /// pair is 8 bytes against about 16 for `rep movs`, so two.
    pub memcpy_moves_size: usize,
    /// What a size-only pick gives up in clocks for a byte; none: bytes alone
    /// decide.
    pub size_trade: Option<SizeTrade>,
    /// For a program whose ABI gives its runtime routines contracts: whether
    /// the convention models each call of a `llrm.qb.` routine, by call. A call
    /// not listed, or listed with a reason, is refused.
    pub runtime_calls: Option<IdMap<InstId, Result<RuntimeAnswer, &'static str>>>,
}

/// The clock side of a size-only pick (`fill_is_smaller`): the longer form
/// is taken only while the bytes it saves, at `milliclocks_per_byte` each,
/// pay for the clocks it adds. All prices come from the target description.
#[derive(Clone, Copy, Debug)]
pub struct SizeTrade {
    /// Clocks a byte of code is worth, in thousandths.
    pub milliclocks_per_byte: i64,
    /// A `rep stos` fill: its fixed clocks, and each byte it stores.
    pub fill: i64,
    pub fill_cell: i64,
    /// One store of a register-sized and of a wide (prefixed) value.
    pub store: i64,
    pub prefix: i64,
}

impl BuilderTarget {
    /// This target with its clock price of a byte of code.
    pub fn with_size_trade(
        mut self,
        trade: Option<SizeTrade>,
    ) -> BuilderTarget {
        self.size_trade = trade;
        self
    }

    /// This target with the cost model of its CPU.
    pub fn with_tti(
        mut self,
        tti: Option<std::rc::Rc<dyn llrm_mir::tti::Tti>>,
    ) -> BuilderTarget {
        self.tti = tti.map(TtiHandle);
        self
    }

    /// This target with sibling calls made.
    pub fn with_tail_calls(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.tail_calls = on;
        self
    }

    /// This target rounding conversions to their float type.
    pub fn with_float_store(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.float_store = on;
        self
    }

    /// This target with divides by constants kept as divides (`isIntDivCheap`).
    pub fn with_int_div_cheap(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.dag.int_div_cheap = on;
        self
    }

    /// This target with jump tables on or off.
    pub fn with_jump_tables(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.jump_tables = on;
        self
    }

    /// This target tuned for size or not.
    pub fn with_opt_size(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.opt_size = on;
        self
    }

    /// This target tuned for the least size or not.
    pub fn with_min_size(
        mut self,
        on: bool,
    ) -> BuilderTarget {
        self.min_size = on;
        self
    }

    /// This target with its size limit of memcpy pairs.
    pub fn with_memcpy_moves_size(
        mut self,
        moves: usize,
    ) -> BuilderTarget {
        self.memcpy_moves_size = moves;
        self
    }

    /// This target with its size limit of memset stores.
    pub fn with_memset_stores_size(
        mut self,
        stores: usize,
    ) -> BuilderTarget {
        self.memset_stores_size = stores;
        self
    }

    /// This target with a stack alignment, whatever the target's is.
    pub fn with_stack_alignment(
        mut self,
        bytes: u64,
    ) -> BuilderTarget {
        self.stack_align = Some(bytes);
        self
    }

    /// This target with the verdict on each call of a runtime routine.
    pub fn with_runtime_call_verdicts(
        mut self,
        verdicts: Option<IdMap<InstId, Result<RuntimeAnswer, &'static str>>>,
    ) -> BuilderTarget {
        self.runtime_calls = verdicts;
        self
    }

    /// This target with the runtime routines the driver states.
    pub fn with_runtime_calls(
        mut self,
        calls: &[(String, u32, String)],
    ) -> BuilderTarget {
        self.libcalls = Some(calls.to_vec());
        self
    }

    /// This target with a runtime that has `names`, in the default convention
    /// under their own names, whatever its `c.toml` says.
    pub fn with_libcalls(
        mut self,
        names: &[&str],
    ) -> BuilderTarget {
        self.libcalls = Some(names.iter().map(|name| ((*name).to_owned(), 0, (*name).to_owned())).collect());
        self
    }

    /// i486 and P5: 32-bit pointers, `i8`/`i16`/`i32` in registers.
    pub fn m32() -> BuilderTarget {
        BuilderTarget {
            dag: DagConfig { offset_folding: true, ..DagConfig::default() },
            shift_amount: EVT::I8,
            legal_ints: vec![8, 16, 32],
            libcalls: None,
            stack_align: None,
            float_store: false,
            jump_tables: true,
            opt_size: false,
            min_size: false,
            memset_stores_size: 0,
            memcpy_moves_size: 0,
            size_trade: None,
            tail_calls: false,
            tti: None,
            runtime_calls: None,
        }
    }

    /// x86-64: 64-bit pointers, `i8`..`i64` in registers.
    pub fn m64() -> BuilderTarget {
        BuilderTarget {
            dag: DagConfig { pointer: EVT::I64, offset_folding: true, ..DagConfig::default() },
            legal_ints: vec![8, 16, 32, 64],
            ..BuilderTarget::m32()
        }
    }

    /// Real mode: 16-bit near pointers, `i8`/`i16`/`i32` in registers (the
    /// pin's `In16BitMode` has `GR32` legal).
    pub fn m16() -> BuilderTarget {
        BuilderTarget {
            dag: DagConfig { pointer: EVT::I16, offset_folding: true, ..DagConfig::default() },
            shift_amount: EVT::I8,
            legal_ints: vec![8, 16, 32],
            libcalls: None,
            stack_align: None,
            float_store: false,
            jump_tables: true,
            opt_size: false,
            min_size: false,
            memset_stores_size: 0,
            memcpy_moves_size: 0,
            size_trade: None,
            tail_calls: false,
            tti: None,
            runtime_calls: None,
        }
    }
}

/// The stack pointer the pointer type names: `sp`, `esp` or `rsp`.
pub(super) fn stack_pointer_name(pointer: EVT) -> &'static str {
    if pointer == EVT::I16 {
        "sp"
    } else if pointer == EVT::I64 {
        "rsp"
    } else {
        "esp"
    }
}

/// A selected block: its DAG and the phi inputs it exports.
pub struct BlockDag {
    pub dag: SelectionDAG,
    /// `PHINodesToUpdate`: the phi and the register its incoming value is in.
    pub phi_updates: Vec<(ValueId, u32)>,
    /// The argument registers copied to virtual registers, in the entry block.
    pub live_ins: Vec<(RegId, u32)>,
    /// The blocks a `switch` lowering made, built after it: their ids start
    /// at the function's block capacity.
    pub extra: Vec<(BlockId, BlockDag)>,
    /// The calls in tail position (`isInTailCallPosition`); step 3 makes them
    /// `TC_RETURN`s.
    pub tail_calls: Vec<InstId>,
    /// The DAG value each IR value of the block was built as.
    pub values: Vec<(ValueId, SDValue)>,
}

/// `SelectionDAGISel::SelectBasicBlock` for `block`; the entry block also
/// lowers the arguments (`LowerArguments`), so it must be built first.
pub fn build_block(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    target: &BuilderTarget,
    arch: &dyn llrm_target::Target,
    convention: &Convention,
    info: &mut FunctionLoweringInfo,
    block: BlockId,
) -> Result<BlockDag, Refused> {
    let order = function.layout().to_vec();
    let mut builder = Builder::new(
        module,
        function,
        layout,
        target,
        arch,
        convention,
        info,
        block,
        order,
        info.next_block.max(function.block_capacity() as u32),
        None,
    );
    if Some(block) == function.entry() {
        builder.lower_arguments()?;
    }
    // selection starts at the first non-phi (`getFirstNonPHIIt`)
    for inst in
        function.iter_block(block).filter(|inst| function.instruction(*inst).opcode != Opcode::Phi).collect::<Vec<_>>()
    {
        builder.visit(inst)?;
        if builder.had_tail_call {
            break;
        }
    }
    let root = builder.control_root();
    builder.dag.set_root(root);
    let switch_name = dag_name(&builder.dag, block);
    let values: Vec<(ValueId, SDValue)> = builder.node_map.iter().map(|(id, one)| (id, *one)).collect();
    let Builder { dag, phi_updates, live_ins, deferred, order, info, next_synthetic, tail_calls, .. } = builder;
    // `FinishBasicBlock`: the blocks the switch lowering made, in a layout that
    // is final
    let mut extra = Vec::new();
    let mut next = next_synthetic;
    // the blocks the lowerings made are emitted in the layout they were placed
    // in: a branch to the next block falls through, so the order they were
    // built in (a jump table's block before the header that precedes it) is
    // wrong. The pin builds them in `SwitchCases`/`JTCases` order and lays the
    // blocks out as they were inserted.
    let mut deferred = deferred;
    deferred.sort_by_key(|item| order.iter().position(|one| *one == item.block()));
    for item in deferred {
        let block = item.block();
        let mut one = Builder::new(
            module,
            function,
            layout,
            target,
            arch,
            convention,
            info,
            block,
            order.clone(),
            next,
            Some(switch_name.clone()),
        );
        one.visit_deferred(item)?;
        let root = one.control_root();
        one.dag.set_root(root);
        next = one.next_synthetic;
        extra.push((
            block,
            BlockDag {
                dag: one.dag,
                phi_updates: Vec::new(),
                live_ins: Vec::new(),
                extra: Vec::new(),
                tail_calls: one.tail_calls,
                values: Vec::new(),
            },
        ));
    }
    info.next_block = next;
    Ok(BlockDag { dag, phi_updates, live_ins, extra, tail_calls, values })
}

fn dag_name(
    dag: &SelectionDAG,
    block: BlockId,
) -> String {
    dag.block_name(block.0).to_owned()
}

impl<'a> Builder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
        target: &'a BuilderTarget,
        arch: &'a dyn llrm_target::Target,
        convention: &'a Convention,
        info: &'a mut FunctionLoweringInfo,
        block: BlockId,
        order: Vec<BlockId>,
        next_synthetic: u32,
        synthetic_name: Option<String>,
    ) -> Builder<'a> {
        let fp_extended = function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    llrm_mir::Attribute::Str(name, Some(value))
                        if name == llrm_mir::constantfolding::FP_EVAL && value == llrm_mir::constantfolding::FP_EVAL_EXTENDED
                ),
            );
        let mut dag = SelectionDAG::new(DagConfig { fp_extended, ..target.dag });
        let tables = info
            .tables
            .get_or_insert_with(|| {
                // `isOffsetFoldingLegal`: `shouldAssumeDSOLocal` holds for
                // every symbol under the static relocation
                // model, which these targets (DOS objects, no
                // PIC) always use.
                super::function_lowering::Tables {
                    global_names: std::rc::Rc::new(
                        module.globals.iter().map(|global| global.name.as_deref().unwrap_or("").to_string()).collect(),
                    ),
                    local_symbols: std::rc::Rc::new(if target.dag.dso_local_symbols {
                        vec![true; module.globals.len()]
                    } else {
                        module
                            .globals
                            .iter()
                            .map(|global| {
                                matches!(
                                    global.linkage,
                                    llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                                )
                            })
                            .collect()
                    }),
                    block_names: std::rc::Rc::new(
                        (0..function.block_capacity())
                            .map(|at| {
                                function.block(BlockId(at as u32)).name.clone().unwrap_or_else(|| format!("bb{at}"))
                            })
                            .collect(),
                    ),
                }
            })
            .clone();
        dag.global_names = tables.global_names;
        dag.local_symbols = tables.local_symbols;
        dag.block_names = tables.block_names;
        dag.synthetic_fill = synthetic_name.unwrap_or_else(|| dag_name(&dag, block));
        Builder {
            current: None,
            prelowered: Default::default(),
            module,
            function,
            layout,
            target,
            arch,
            convention,
            info,
            live_ins: Vec::new(),
            dag,
            node_map: IdMap::new(),
            pending_loads: Vec::new(),
            pending_exports: Vec::new(),
            constants_out: HashMap::default(),
            phi_updates: Vec::new(),
            tail_calls: Vec::new(),
            tail_candidate: None,
            sibcall: false,
            variadic_call: false,
            had_tail_call: false,
            volatile_memory: false,
            block,
            order,
            deferred: Vec::new(),
            next_synthetic,
            plan: None,
        }
    }
}

struct Builder<'a> {
    /// The instruction being visited.
    current: Option<InstId>,
    /// Compares made early, at the call whose flags they read.
    prelowered: llrm_mir::dense::IdSet<InstId>,
    module: &'a Module,
    function: &'a Function,
    layout: &'a DataLayout,
    target: &'a BuilderTarget,
    arch: &'a dyn llrm_target::Target,
    convention: &'a Convention,
    live_ins: Vec<(RegId, u32)>,
    info: &'a mut FunctionLoweringInfo,
    dag: SelectionDAG,
    node_map: IdMap<ValueId, SDValue>,
    pending_loads: Vec<SDValue>,
    pending_exports: Vec<SDValue>,
    constants_out: HashMap<ConstantId, u32>,
    phi_updates: Vec<(ValueId, u32)>,
    tail_calls: Vec<InstId>,
    /// The call being built if it is in tail position (step 1), until
    /// `visit_call` has its convention to ask the target about.
    tail_candidate: Option<InstId>,
    /// The call `visit_call` is lowering is a sibling call: `lower_call` makes
    /// a `TC_RETURN`.
    sibcall: bool,
    /// The call `visit_call` is lowering is to a variadic function: at m64
    /// `lower_call` sets `al` to the vector registers it uses.
    variadic_call: bool,
    /// A `TC_RETURN` ended the block: nothing after the call is built.
    had_tail_call: bool,
    /// The memory intrinsic being lowered is volatile.
    volatile_memory: bool,
    block: BlockId,
    /// The blocks in layout order, with the ones a switch lowering adds.
    order: Vec<BlockId>,
    deferred: Vec<switch::Deferred>,
    next_synthetic: u32,
    plan: Option<super::switchcg::SwitchPlan>,
}

fn flags_of(flags: llrm_mir::opcode::Flags) -> SDNodeFlags {
    use llrm_mir::opcode::Flags;
    SDNodeFlags::default()
        .with(SDNodeFlags::NO_UNSIGNED_WRAP, flags.contains(Flags::NUW))
        .with(SDNodeFlags::NO_SIGNED_WRAP, flags.contains(Flags::NSW))
        .with(SDNodeFlags::EXACT, flags.contains(Flags::EXACT))
        .with(SDNodeFlags::DISJOINT, flags.contains(Flags::DISJOINT))
        .with(SDNodeFlags::NON_NEG, flags.contains(Flags::NNEG))
        .with(SDNodeFlags::SAME_SIGN, flags.contains(Flags::SAMESIGN))
        .with(SDNodeFlags::NO_NANS, flags.contains(Flags::NNAN))
        .with(SDNodeFlags::NO_INFS, flags.contains(Flags::NINF))
        .with(SDNodeFlags::NO_SIGNED_ZEROS, flags.contains(Flags::NSZ))
        .with(SDNodeFlags::ALLOW_RECIPROCAL, flags.contains(Flags::ARCP))
        .with(SDNodeFlags::ALLOW_CONTRACT, flags.contains(Flags::CONTRACT))
        .with(SDNodeFlags::ALLOW_REASSOC, flags.contains(Flags::REASSOC))
}

fn float_cond_code(predicate: llrm_mir::opcode::FloatPredicate) -> CondCode {
    use llrm_mir::opcode::FloatPredicate as P;
    match predicate {
        P::False => CondCode::SetFalse,
        P::Oeq => CondCode::SetOEq,
        P::Ogt => CondCode::SetOGt,
        P::Oge => CondCode::SetOGe,
        P::Olt => CondCode::SetOLt,
        P::Ole => CondCode::SetOLe,
        P::One => CondCode::SetONe,
        P::Ord => CondCode::SetO,
        P::Ueq => CondCode::SetUEq,
        P::Ugt => CondCode::SetUGt,
        P::Uge => CondCode::SetUGe,
        P::Ult => CondCode::SetULt,
        P::Ule => CondCode::SetULe,
        P::Une => CondCode::SetUNe,
        P::Uno => CondCode::SetUo,
        P::True => CondCode::SetTrue,
    }
}

fn cond_code(predicate: IntPredicate) -> CondCode {
    match predicate {
        IntPredicate::Eq => CondCode::SetEq,
        IntPredicate::Ne => CondCode::SetNe,
        IntPredicate::Ugt => CondCode::SetUGt,
        IntPredicate::Uge => CondCode::SetUGe,
        IntPredicate::Ult => CondCode::SetULt,
        IntPredicate::Ule => CondCode::SetULe,
        IntPredicate::Sgt => CondCode::SetGt,
        IntPredicate::Sge => CondCode::SetGe,
        IntPredicate::Slt => CondCode::SetLt,
        IntPredicate::Sle => CondCode::SetLe,
    }
}

impl Builder<'_> {
    fn type_of(
        &self,
        ty: TypeId,
    ) -> Result<EVT, Refused> {
        match self.module.context.types.get(ty) {
            Type::Int(bits) if (1..=128).contains(bits) => Ok(EVT::integer(*bits)),
            Type::Pointer(0) => Ok(self.target.dag.pointer),
            // a pair (selector, offset) is `far`; a flat space as wide as a
            // near pointer is one
            Type::Pointer(space) if self.layout.is_pair(*space) => Ok(MVT::Far.into()),
            Type::Pointer(space) if self.layout.pointer(*space).bits == self.layout.pointer(0).bits => {
                Ok(self.target.dag.pointer)
            }
            Type::Pointer(_) => Err(Refused("pointer in another address space")),
            Type::Float(kind) => Ok(EVT::Simple(match kind {
                llrm_mir::types::FloatKind::Float => MVT::F32,
                llrm_mir::types::FloatKind::Double => MVT::F64,
                llrm_mir::types::FloatKind::X86Fp80 => MVT::F80,
            })),
            _ => Err(Refused("aggregate or vector value")),
        }
    }

    /// The register type a value of `vt` is carried in across blocks:
    /// itself when legal, else the next legal width up.
    fn register_type(
        &self,
        vt: EVT,
    ) -> Result<EVT, Refused> {
        if vt.is_floating_point() || vt == MVT::Far.into() {
            return Ok(vt);
        }
        let bits = vt.size_in_bits();
        self.target
            .legal_ints
            .iter()
            .find(|legal| **legal >= bits)
            .map(|legal| EVT::integer(*legal))
            .ok_or(Refused("integer wider than a register"))
    }

    /// How a value of `vt` is carried across blocks and calls: the type of
    /// each register and how many (an integer of twice the widest register
    /// is two of them).
    fn parts(
        &self,
        vt: EVT,
    ) -> Result<(EVT, usize), Refused> {
        match self.register_type(vt) {
            Ok(reg_vt) => Ok((reg_vt, 1)),
            Err(_) => {
                let widest = EVT::integer(*self.target.legal_ints.last().expect("a legal integer"));
                if vt.is_integer() && vt.size_in_bits() == 2 * widest.size_in_bits() {
                    Ok((widest, 2))
                } else {
                    Err(Refused("integer wider than two registers"))
                }
            }
        }
    }

    /// `parts` for a value carried across blocks in virtual registers: a far
    /// pointer is two `i16` registers (offset, then selector), as the pin
    /// splits any expanded type (`RegsForValue`).
    fn register_parts(
        &self,
        vt: EVT,
    ) -> Result<(EVT, usize), Refused> {
        if vt == MVT::Far.into() {
            return Ok((EVT::I16, 2));
        }
        self.parts(vt)
    }

    /// `getNumRegistersForCallingConv`: an integer a convention returns in
    /// `registers` registers is that many equal parts, even when it is a
    /// register type itself (an `i32` in `dx:ax` at real mode).
    fn parts_in(
        &self,
        vt: EVT,
        registers: usize,
    ) -> Result<(EVT, usize), Refused> {
        if registers == 2 && vt == MVT::Far.into() {
            return Ok((EVT::I16, 2));
        }
        if registers == 2 && vt.is_integer() && vt.size_in_bits() % 2 == 0 {
            return Ok((EVT::integer(vt.size_in_bits() / 2), 2));
        }
        self.parts(vt)
    }

    /// `getCopyToParts` for two parts: the high half is extracted first.
    fn split_value(
        &mut self,
        value: SDValue,
        part: EVT,
    ) -> [SDValue; 2] {
        let pointer = self.target.dag.pointer;
        let one = self.dag.get_constant(1, pointer);
        let zero = self.dag.get_constant(0, pointer);
        let high = self.dag.get_node(ISD::ExtractElement, part, &[value, one]);
        let low = self.dag.get_node(ISD::ExtractElement, part, &[value, zero]);
        [low, high]
    }

    fn operand_type(
        &self,
        operand: Operand,
    ) -> Result<EVT, Refused> {
        let ty = self.function.operand_type(&self.module.context, operand).ok_or(Refused("block operand"))?;
        self.type_of(ty)
    }

    // ---- roots (SelectionDAGBuilder::updateRoot and friends)

    fn update_root(
        &mut self,
        pending: Vec<SDValue>,
    ) -> SDValue {
        let mut root = self.dag.get_root();
        if pending.is_empty() {
            return root;
        }
        let mut pending = pending;
        if self.dag.opcode(root) != ISD::EntryToken && !pending.iter().any(|chain| self.dag.operand(*chain, 0) == root)
        {
            pending.push(root);
        }
        root = if pending.len() == 1 { pending[0] } else { self.dag.get_token_factor(&pending) };
        self.dag.set_root(root);
        root
    }

    fn memory_root(&mut self) -> SDValue {
        let pending = std::mem::take(&mut self.pending_loads);
        self.update_root(pending)
    }

    /// `getRoot`.
    fn root(&mut self) -> SDValue {
        self.memory_root()
    }

    fn control_root(&mut self) -> SDValue {
        let pending = std::mem::take(&mut self.pending_exports);
        self.update_root(pending)
    }

    // ---- values

    fn get_value(
        &mut self,
        operand: Operand,
    ) -> Result<SDValue, Refused> {
        match operand {
            Operand::Constant(constant) => self.constant_value(constant),
            Operand::Block(_) => Err(Refused("block as a value")),
            Operand::Value(value) => {
                if let Some(found) = self.node_map.get(&value) {
                    return Ok(*found);
                }
                if let Some(copy) = self.copy_from_regs(value)? {
                    return Ok(copy);
                }
                let made = self.value_impl(value)?;
                self.node_map.insert(value, made);
                Ok(made)
            }
        }
    }

    /// `LowerArguments` with X86's `LowerFormalArguments` over the target's
    /// `Convention`: a stack parameter is a load from a fixed object (indices
    /// -1, -2, ... in parameter order), a register parameter the copy of a
    /// live-in virtual register. A parameter read outside the entry block is
    /// also copied to a register of its own.
    fn lower_arguments(&mut self) -> Result<(), Refused> {
        let entry = self.function.entry();
        let mut outside = llrm_support::hash::HashSet::default();
        for &block in self.function.layout() {
            for inst in self.function.iter_block(block) {
                let instruction = self.function.instruction(inst);
                for operand in &instruction.operands {
                    if let Operand::Value(value) = operand {
                        if Some(block) != entry || instruction.opcode == Opcode::Switch {
                            outside.insert(*value);
                        }
                    }
                }
            }
        }
        let mut fixed = 0;
        for (at, &arg) in self.function.parameters().to_vec().iter().enumerate() {
            let vt = self.type_of(self.function.value(arg).ty)?;
            // the registers a convention names split a value as
            // `getNumRegisters` does
            let (mut part_vt, parts) = match self.convention.parameters.get(at) {
                Some(Parameter::Registers(regs)) => self.parts_in(vt, regs.len())?,
                _ => self.parts(vt)?,
            };
            // `CCPromoteToType<i32>`: a byte or word in a register is the
            // dword, whose high bits `zeroext`/`signext` assert
            if matches!(
                self.convention.parameters.get(at),
                Some(Parameter::Registers(_))
            )
                && parts == 1
                && vt.is_integer()
                && vt.size_in_bits() < 32
                && self.target.dag.pointer.size_in_bits() >= 32
            {
                part_vt = EVT::I32;
            }
            let entry_node = self.dag.entry_node();
            let mut pieces = Vec::new();
            match self.convention.parameters.get(at) {
                // a byval parameter is its bytes in the caller's frame: the
                // value is their address
                Some(Parameter::Bytes(offset)) => {
                    fixed += 1;
                    self.dag.fixed_offsets.insert(-fixed, *offset);
                    pieces.push(self.dag.get_frame_index(self.target.dag.pointer, -fixed));
                }
                Some(Parameter::Cell(offset)) => {
                    for part in 0..parts {
                        fixed += 1;
                        self.dag
                            .fixed_offsets
                            .insert(-fixed, offset + part as i64 * i64::from(part_vt.size_in_bits() / 8));
                        let ptr = self.dag.get_frame_index(self.target.dag.pointer, -fixed);
                        // The fixed object is a whole stack slot
                        // (`CreateFixedObject`),
                        // aligned as one: a narrow argument's load is as
                        // aligned as the slot, which
                        // lets it widen (`loadi32` of an any-extend).
                        let align = (self.layout.align(&self.module.context.types, self.function.value(arg).ty) as u32)
                            .max(self.target.dag.pointer.size_in_bits() / 8);
                        let info = MemInfo {
                            mem_vt: part_vt,
                            align,
                            volatile: false,
                            base: MemBase::Stack(-fixed),
                            ext: None,
                            space: 0,
                        };
                        let load = self.dag.get_load(part_vt, entry_node, ptr, info);
                        // `LowerMemArgument` makes the object immutable unless
                        // it is byval
                        // (`isImmutableObjectIndex`): the load is
                        // `isDereferenceableInvariantLoad`.
                        self.dag.invariant_loads.insert(load.node);
                        pieces.push(load);
                    }
                }
                Some(Parameter::Registers(regs)) if regs.len() == parts => {
                    for reg in regs {
                        let vreg = self.info.new_reg();
                        self.live_ins.push((*reg, vreg));
                        pieces.push(self.dag.get_copy_from_reg(entry_node, vreg, part_vt));
                    }
                }
                _ => return Err(Refused("argument passed in several places or by value")),
            }
            let value = if parts == 2 {
                self.dag.get_node(ISD::BuildPair, vt, &pieces)
            } else if vt.is_integer() && vt.size_in_bits() < part_vt.size_in_bits() {
                // a narrow argument arrives in a register-sized slot, and
                // `zeroext`/`signext` says what is in the bits above it
                // (`getCopyFromParts` with the `AssertOp` of `LowerArguments`)
                let attrs = self.function.parameter_attrs.get(at).map_or(&[][..], Vec::as_slice);
                let assert = if llrm_mir::memory::has(attrs, "signext") {
                    Some(ISD::AssertSext)
                } else if llrm_mir::memory::has(attrs, "zeroext") {
                    Some(ISD::AssertZext)
                } else {
                    None
                };
                let piece = match assert {
                    Some(code) => {
                        let narrow = self.dag.get_value_type_node(vt);
                        self.dag.get_node(code, part_vt, &[pieces[0], narrow])
                    }
                    None => pieces[0],
                };
                self.dag.get_node(ISD::Truncate, vt, &[piece])
            } else {
                pieces[0]
            };
            self.node_map.insert(arg, value);
            if self.function.parameter_attrs.get(at).is_some_and(|attrs| {
                attrs
                    .iter()
                    .any(
                        |attr| matches!(
                            attr,
                            llrm_mir::opcode::Attribute::Type(name, _) if name == "sret"
                        ),
                    )
            }) {
                // X86 keeps the sret pointer in a register to return it
                let reg = self.info.new_reg();
                self.info.sret_reg = Some(reg);
                let copy = self.dag.get_copy_to_reg(entry_node, reg, value);
                self.dag.set_root(copy);
            }
            if outside.contains(&arg) {
                let reg = self.info.create_reg(arg);
                self.copy_value_to_register(Operand::Value(arg), reg, ISD::AnyExtend)?;
            }
        }
        // `LowerFormalArguments`: the first variadic argument is the next
        // fixed object, which `va_start` stores the address of
        if self.module.signature(self.function.ty).2 {
            self.dag.var_args_frame_index = Some(-(fixed + 1));
            if self.target.dag.pointer == EVT::I64 {
                self.save_variadic_registers()?;
            }
        }
        Ok(())
    }

    /// X86's `LowerFormalArguments` for a SysV x86-64 variadic function: the
    /// argument registers the named parameters did not take are stored in the
    /// register save area (one 8-byte slot each, in order) that `va_start`
    /// names. The vector registers' half of the area (48..176) is left
    /// unwritten until floating point is selected at m64 (DEVIATE SD238: a
    /// `double` passed to such a function is not read back).
    fn save_variadic_registers(&mut self) -> Result<(), Refused> {
        let calling = self.arch.calling();
        let Some(found) = calling.by_number(self.function.calling_convention) else {
            return Err(Refused("va_start at m64 in a function with no convention"));
        };
        let named: usize = self
            .convention
            .parameters
            .iter()
            .map(|parameter| match parameter {
                Parameter::Registers(regs) => regs.len(),
                _ => 0,
            })
            .sum();
        let registers: Vec<RegId> =
            found.argument_registers.iter().map(|name| self.arch.register_named(name)).collect();
        let Some(save) = self.info.variadic_save else {
            return Err(Refused("va_start at m64 with no register save area"));
        };
        let pointer = self.target.dag.pointer;
        for (at, &register) in registers.iter().enumerate().skip(named) {
            let vreg = self.info.new_reg();
            self.live_ins.push((register, vreg));
            let entry = self.dag.entry_node();
            let value = self.dag.get_copy_from_reg(entry, vreg, pointer);
            let base = self.dag.get_frame_index(pointer, save);
            let slot = self.dag.get_constant(at as u128 * 8, pointer);
            let address = self.dag.get_node(ISD::Add, pointer, &[base, slot]);
            let info =
                MemInfo { mem_vt: pointer, align: 8, volatile: false, base: MemBase::Stack(save), ext: None, space: 0 };
            let root = self.control_root();
            let store = self.dag.get_store(root, value, address, info);
            self.dag.set_root(store);
        }
        // The vector argument registers named parameters did not take go to
        // the other half of the area, if `al` says any were used
        // (`VASTART_SAVE_XMM_REGS`).
        let vector_registers = found.float_argument_registers.len();
        let named_vectors = self
            .convention
            .parameters
            .iter()
            .map(|parameter| match parameter {
                Parameter::Registers(regs) => regs.iter().filter(|reg| reg.is_xmm()).count(),
                _ => 0,
            })
            .sum::<usize>();
        if vector_registers > named_vectors {
            let vreg = self.info.new_reg();
            self.live_ins.push((RegId::AL, vreg));
            let entry = self.dag.entry_node();
            let al = self.dag.get_copy_from_reg(entry, vreg, EVT::I8);
            let frame = self.dag.get_constant_with(save as u128, EVT::I32, true, false);
            let first = self
                .dag
                .get_constant_with(
                    (found.argument_registers.len() * 8 + named_vectors * 16) as u128,
                    EVT::I32,
                    true,
                    false,
                );
            let root = self.control_root();
            let saved = self.dag.get_node(super::x86isd::VASTART_SAVE_XMM_REGS, EVT::OTHER, &[root, al, frame, first]);
            self.dag.set_root(saved);
        }
        self.dag.var_args_save = Some((save, named as u32 * 8));
        Ok(())
    }

    /// `visitInvoke`: the call, then the branch to the normal block. The
    /// unwind edge is a CFG edge only; QB's runtime enters its pad, so no
    /// `EH_LABEL`s or tables (DEVIATE SD102).
    fn visit_invoke(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        info: &llrm_mir::opcode::CallInfo,
    ) -> Result<(), Refused> {
        // the arguments, the normal and unwind blocks, then the callee
        let [.., Operand::Block(normal), Operand::Block(_), callee] = instruction.operands[..] else {
            return Err(Refused("invoke without its blocks"));
        };
        let mut call = instruction.clone();
        call.operands.truncate(instruction.operands.len() - 3);
        call.operands.push(callee);
        self.visit_call(&call, info)?;
        // `visitInvoke`: the result, if read outside this block, is made a
        // virtual register here (a terminator is skipped by the export in
        // `visit`).
        if let Some(result) = instruction.result {
            if let Some(reg) = self.info.value_regs.get(&result).copied() {
                self.copy_value_to_register(Operand::Value(result), reg, ISD::AnyExtend)?;
            }
        }
        let root = self.control_root();
        let dest = self.dag.get_basic_block(normal.0);
        let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
        self.dag.set_root(node);
        Ok(())
    }

    /// `visitCall` through `LowerCallTo` and X86's `LowerCall`, for a direct
    /// call of a function whose arguments and result fit single registers or
    /// the stack: `callseq_start`, the stack stores, the register copies,
    /// An exchange `llrm_mir::atomic::lowered` left: `atomicrmw xchg`, or a
    /// store that must be ordered, is `ATOMIC_SWAP`. The machine locks `xchg`
    /// without asking, so every ordering is met.
    fn visit_atomic(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        info: llrm_mir::opcode::AtomicInfo,
    ) -> Result<(), Refused> {
        use llrm_mir::opcode::{AtomicKind, RmwOp};
        let arguments = &instruction.operands[..instruction.operands.len() - 1];
        let (pointer, value) = match info.kind {
            AtomicKind::Rmw(RmwOp::Xchg) => (arguments[0], arguments[1]),
            AtomicKind::Store => (arguments[1], arguments[0]),
            _ => return Err(Refused("an atomic other than an exchange")),
        };
        if self.operand_type(pointer)? == MVT::Far.into() {
            return Err(Refused("an atomic access through a far pointer"));
        }
        let ty = self.function.operand_type(&self.module.context, value).ok_or(Refused("block operand"))?;
        let (src, ptr) = (self.get_value(value)?, self.get_value(pointer)?);
        let mem = self.mem_info(ty, info.align, true, pointer)?;
        let root = self.root();
        let swap = self.dag.get_atomic_swap(root, ptr, src, mem);
        self.dag.set_root(SDValue { node: swap.node, res: 1 });
        if let Some(result) = instruction.result {
            self.set_value(result, swap);
        }
        Ok(())
    }

    /// `X86ISD::CALL`, `callseq_end`, then the result read back.
    fn visit_call(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        info: &llrm_mir::opcode::CallInfo,
    ) -> Result<(), Refused> {
        let (callee, arguments) = instruction.operands.split_last().expect("a call names its callee");
        // A direct call names a function; an indirect one calls the value.
        let direct = match *callee {
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Global(global) => Some(global),
                // a constant expression (`inttoptr`, a cast of a global): the
                // value is the callee, as for a pointer in a register
                _ => None,
            },
            _ => None,
        };
        if let Some(atomic) = direct
            .and_then(|global| self.module.global(global).name.owned())
            .and_then(|name| llrm_mir::atomic::decode(&name))
        {
            return self.visit_atomic(instruction, atomic);
        }
        if let Some(intrinsic) = direct.and_then(|global| self.module.global(global).intrinsic()) {
            let name = direct.and_then(|global| self.module.global(global).name.owned()).unwrap_or_default();
            if intrinsic == llrm_mir::intrinsics::Intrinsic::Asm {
                return self.visit_asm(instruction, &name);
            }
            if intrinsic == llrm_mir::intrinsics::Intrinsic::Code {
                return self.visit_code(instruction, &name);
            }
            // the refusal names the intrinsic; the few that are never built are
            // leaked once each
            return self
                .visit_intrinsic(instruction, info, &intrinsic)
                .map_err(
                    |why| if why.0 == "intrinsic call" {
                        Refused(Box::leak(format!("intrinsic call {name}").into_boxed_str()))
                    } else {
                        why
                    },
                );
        }
        let mut flags_answer = false;
        if let Some(verdicts) = &self.target.runtime_calls
            && direct
                .and_then(|global| self.module.global(global).name.owned())
                .is_some_and(|name| name.starts_with(llrm_hir::mir::RUNTIME))
        {
            match self.current.and_then(|inst| verdicts.get(&inst)) {
                Some(Ok(answer)) => flags_answer = *answer == RuntimeAnswer::Flags,
                Some(Err(why)) => return Err(Refused(why)),
                None => return Err(Refused("call of a QB runtime routine nobody checked")),
            }
        }
        let mut arg_types = Vec::new();
        for argument in arguments {
            arg_types
                .push(self.function.operand_type(&self.module.context, *argument).ok_or(Refused("block argument"))?);
        }
        let first = self.arch.first_argument_offset(false);
        let (result_type, _, variadic) = self.module.signature(info.function_type);
        let plan = convention_for(
            self.module,
            self.layout,
            self.arch,
            first,
            info.calling_convention,
            result_type,
            variadic,
            &arg_types,
            &info.argument_attrs,
        )
        .map_err(|_| Refused("call the convention does not describe"))?;
        if let Some(tail) = self.tail_candidate.take()
            && self.eligible_for_sibling_call(info, &plan, variadic, direct, first, arguments)
        {
            self.tail_calls.push(tail);
            self.sibcall = self.target.tail_calls;
        }
        // A call of a function with more arguments than it declares (an
        // old-style `()` definition): the callee pops what it declares, the
        // rest is the caller's to remove.
        let mut plan = plan;
        // A runtime routine pops what its contract says, which the verdict
        // checked against what is pushed: its declaration names no parameters.
        let runtime = is_runtime_routine(self, direct);
        let declared = direct.and_then(|id| self.module.global(id).function()).map(|one| one.parameters().len());
        if let Some(declared) =
            declared.filter(|declared| !runtime && *declared < arg_types.len() && !variadic && plan.popped > 0)
        {
            plan.popped = convention_for(
                self.module,
                self.layout,
                self.arch,
                first,
                info.calling_convention,
                result_type,
                variadic,
                &arg_types[..declared],
                &info.argument_attrs,
            )
            .map_err(|_| Refused("call the convention does not describe"))?
            .popped;
        }
        // Evaluate the callee and the arguments before the call sequence
        // starts.
        let called = self.get_value(*callee)?;
        let callee_node = match direct {
            Some(global) => self.dag.get_target_global_address(self.target.dag.pointer, global.0, 0),
            None => called,
        };
        let mut values = Vec::new();
        for argument in arguments {
            let value = self.get_value(*argument)?;
            let value = self.pass_float_bits(*argument, value);
            self.parts(self.dag.value_type(value))?;
            values.push(value);
        }
        let attrs = |at: usize| info.argument_attrs.get(at).map_or(&[][..], Vec::as_slice);
        let extends: Vec<ISD> = (0..values.len())
            .map(|at| {
                if llrm_mir::memory::has(attrs(at), "signext") {
                    ISD::SignExtend
                } else if llrm_mir::memory::has(attrs(at), "zeroext") {
                    ISD::ZeroExtend
                } else {
                    ISD::AnyExtend
                }
            })
            .collect();
        let byvals: Vec<Option<(u64, u64)>> = (0..values.len())
            .map(|at| {
                let attrs = attrs(at);
                let ty = attrs
                    .iter()
                    .find_map(
                        |attr| match attr {
                            llrm_mir::opcode::Attribute::Type(name, ty) if name == "byval" => Some(*ty),
                            _ => None,
                        },
                    )?;
                let align = attrs
                    .iter()
                    .find_map(
                        |attr| match attr {
                            llrm_mir::opcode::Attribute::Int(name, value) if name == "align" => Some(*value),
                            _ => None,
                        },
                    );
                Some((self.layout.alloc_size(&self.module.context.types, ty), align.unwrap_or(4)))
            })
            .collect();
        if flags_answer {
            // only a compare with zero reads them
            let reads = instruction
                .result
                .is_none_or(
                    |value| self.function
                        .users(value)
                        .iter()
                        .all(
                            |one| {
                                let user = self.function.instruction(one.user);
                                matches!(user.opcode, Opcode::ICmp(_))
                                    && matches!(
                                        user.operands.get(1),
                                        Some(Operand::Constant(zero)) if matches!(self.module.context.get(*zero).kind, ConstantKind::Null | ConstantKind::Zero | ConstantKind::Int(0))
                                    )
                            },
                        ),
                );
            if !reads {
                return Err(Refused("a runtime routine's flags read other than by a compare with zero"));
            }
        }
        self.variadic_call = variadic;
        let result =
            self.lower_call(callee_node, values, &extends, &byvals, &plan, first, result_type, flags_answer)?;
        if let (true, Some(answer)) = (flags_answer, result) {
            self.settle_flags_answer(instruction, answer)?;
        }
        if let (Some(value), Some(result)) = (instruction.result, result) {
            self.set_value(value, result);
        }
        Ok(())
    }

    /// A runtime routine's answer in the flags: a compare of it that feeds only
    /// branches reads the flags where they are. Any other compare is made at
    /// once and tied to the call's chain, so no instruction that rewrites the
    /// flags comes between the call and it.
    fn settle_flags_answer(
        &mut self,
        call: &llrm_mir::module::Instruction,
        answer: SDValue,
    ) -> Result<(), Refused> {
        let Some(value) = call.result else { return Ok(()) };
        self.set_value(value, answer);
        for one in self.function.users(value) {
            let compare = self.function.instruction(one.user).clone();
            let Opcode::ICmp(predicate) = compare.opcode else {
                return Err(Refused("flags read other than by a compare"));
            };
            if self.function.parent(one.user) != Some(self.block) {
                return Err(Refused("a runtime routine's flags read in another block"));
            }
            let result = compare.result.expect("a compare has a result");
            let branches = self
                .function
                .users(result)
                .iter()
                .all(
                    |user| self.function.instruction(user.user).opcode == Opcode::Br
                        && self.function.parent(user.user) == Some(self.block),
                );
            if branches {
                continue;
            }
            self.visit_icmp(&compare, predicate)?;
            let settled = self.get_value(Operand::Value(result))?;
            let root = self.root();
            let reg = self.info.new_reg();
            let byte = self.dag.get_node(ISD::ZeroExtend, EVT::I8, &[settled]);
            let tied = self.dag.get_copy_to_reg(root, reg, byte);
            self.dag.set_root(tied);
            // Later users read the saved byte: the compare itself would be
            // selected where it is used, from flags a later call has rewritten.
            let saved = self.dag.get_copy_from_reg(tied, reg, EVT::I8);
            let kept = self.dag.get_node(ISD::Truncate, self.dag.value_type(settled), &[saved]);
            self.set_value(result, kept);
            self.prelowered.insert(one.user);
        }
        Ok(())
    }

    /// `LowerCall` for arguments already evaluated: the sequence from
    /// `callseq_start` to the result read back; the result, if any.
    /// DEVIATE SD256: at optsize an `f32` argument that is only a memory value
    /// (its load has no other user) is passed as the dword it is. The pin
    /// loads it onto the x87 stack and stores it to the argument slot (`fld;
    /// fstp`); the integer load folds (`visit_bitcast`) and the store becomes
    /// `push dword ptr [m]`.
    fn pass_float_bits(
        &mut self,
        argument: Operand,
        value: SDValue,
    ) -> SDValue {
        let sole = matches!(
            argument,
            Operand::Value(result) if self.function.users(result).into_iter().count() == 1
        );
        if sole
            && self.has_opt_size()
            && self.dag.value_type(value) == EVT::Simple(MVT::F32)
            && self.dag.opcode(value) == ISD::Load
        {
            return self.dag.get_node(ISD::BitCast, EVT::Simple(MVT::I32), &[value]);
        }
        value
    }

    pub(super) fn lower_call(
        &mut self,
        callee_node: SDValue,
        values: Vec<SDValue>,
        extends: &[ISD],
        byvals: &[Option<(u64, u64)>],
        plan: &Convention,
        first: i64,
        result_type: TypeId,
        flags_answer: bool,
    ) -> Result<Option<SDValue>, Refused> {
        let types = &self.module.context.types;
        // a sibling call reuses the caller's frame: no call sequence, and its
        // stack arguments are already where the callee reads them
        let sibling = std::mem::take(&mut self.sibcall);
        let bytes = if sibling { 0 } else { (plan.stack_arguments - first).max(0) as u64 };
        let root = self.root();
        let mut chain = if sibling { root } else { self.dag.get_callseq_start(root, bytes) };
        let mut stores = Vec::new();
        let mut copies: Vec<(RegId, SDValue)> = Vec::new();
        let mut stack_pointer = None;
        for (at, (value, parameter)) in values.iter().zip(&plan.parameters).enumerate() {
            let vt = self.dag.value_type(*value);
            let (part_vt, parts) = match parameter {
                Parameter::Registers(regs) => self.parts_in(vt, regs.len())?,
                _ => self.parts(vt)?,
            };
            let pieces = if parts == 2 { self.split_value(*value, part_vt).to_vec() } else { vec![*value] };
            match parameter {
                Parameter::Cell(_) | Parameter::Bytes(_) if sibling => {}
                Parameter::Cell(displacement) => {
                    for (index, piece) in pieces.iter().enumerate() {
                        // a narrow argument fills its stack slot
                        // (`CCPromoteToType`)
                        let slot = EVT::integer(self.arch.stack_slot_bytes() as u32 * 8);
                        let narrow = self.dag.value_type(*piece);
                        let piece = &if narrow.is_integer() && narrow.bits_lt(slot) {
                            self.dag.get_node(extends.get(at).copied().unwrap_or(ISD::AnyExtend), slot, &[*piece])
                        } else {
                            *piece
                        };
                        let sp = *stack_pointer.get_or_insert_with(|| {
                            let sp = self.arch.register_named(stack_pointer_name(self.target.dag.pointer));
                            self.dag.get_copy_from_phys_reg_plain(chain, sp, self.target.dag.pointer)
                        });
                        let part_bytes = i64::from(part_vt.size_in_bits() / 8);
                        let offset = self
                            .dag
                            .get_constant(
                                (displacement - first + index as i64 * part_bytes) as u128,
                                self.target.dag.pointer,
                            );
                        let address = self.dag.get_node(ISD::Add, self.target.dag.pointer, &[sp, offset]);
                        let vt = self.dag.value_type(*piece);
                        let mem = MemInfo {
                            mem_vt: vt,
                            align: 1,
                            volatile: false,
                            base: MemBase::CallArgument((displacement - first + index as i64 * part_bytes) as i32),
                            ext: None,
                            space: 0,
                        };
                        stores.push(self.dag.get_store(chain, *piece, address, mem));
                    }
                }
                Parameter::Bytes(displacement) => {
                    // `CreateCopyOfByValArgument`: the pointee copied into the
                    // slot, always inline
                    let (size, align) =
                        byvals.get(at).copied().flatten().ok_or(Refused("byval argument of an unknown size"))?;
                    let sp = *stack_pointer.get_or_insert_with(|| {
                        let sp = self.arch.register_named(stack_pointer_name(self.target.dag.pointer));
                        self.dag.get_copy_from_phys_reg_plain(chain, sp, self.target.dag.pointer)
                    });
                    let offset = self.dag.get_constant((displacement - first) as u128, self.target.dag.pointer);
                    let address = self.dag.get_node(ISD::Add, self.target.dag.pointer, &[sp, offset]);
                    let _size = self.dag.get_constant(u128::from(size), self.target.dag.pointer);
                    let copy =
                        self.always_inline_memcpy(chain, address, *value, size, align)?.ok_or(Refused("byval copy"))?;
                    stores.push(copy);
                }
                Parameter::Registers(regs) if regs.len() == pieces.len() => {
                    for (reg, piece) in regs.iter().copied().zip(pieces) {
                        // a narrow argument fills its register
                        // (`CCPromoteToType`)
                        let width = self.arch.registers().bytes(reg).map_or(0, |bytes| bytes as u32 * 8);
                        let vt = self.dag.value_type(piece);
                        // At -m16 a word argument goes in the word register
                        // (`AX`, not `EAX`): the callee reads AX, and a word
                        // required in a dword register is a dword to the
                        // allocator.
                        let word = (vt == EVT::I16 && self.target.dag.pointer == EVT::I16)
                            .then(|| self.arch.registers().view(self.arch.registers().root(reg), 16))
                            .flatten();
                        if let Some(word) = word {
                            copies.push((word, piece));
                            continue;
                        }
                        // and a byte one in the byte register (`DL`, not
                        // `EDX`): the callee truncates
                        // it, so extending it is a 6-byte
                        // `mov edx, 12` for a 2-byte `mov dl, 12`
                        let byte = (vt == EVT::I8 && self.target.dag.pointer == EVT::I16)
                            .then(|| self.arch.registers().view(self.arch.registers().root(reg), 8))
                            .flatten();
                        if let Some(byte) = byte {
                            copies.push((byte, piece));
                            continue;
                        }
                        // A byte or word is promoted to `i32`, a dword goes
                        // in the dword register (`EDI`, not `RDI`)
                        let promoted = width.min(vt.size_in_bits().max(32));
                        let piece = if vt.is_integer() && vt.size_in_bits() < promoted {
                            let code = extends.get(at).copied().unwrap_or(ISD::AnyExtend);
                            self.dag.get_node(code, EVT::integer(promoted), &[piece])
                        } else {
                            piece
                        };
                        let reg = if vt.is_integer() && promoted < width {
                            self.arch.registers().view(self.arch.registers().root(reg), promoted).unwrap_or(reg)
                        } else {
                            reg
                        };
                        copies.push((reg, piece));
                    }
                }
                _ => return Err(Refused("argument passed in several places or by value")),
            }
        }
        // X86's `LowerCall`: a variadic call under SysV x86-64 passes the
        // number of vector registers it uses in `al`
        if std::mem::take(&mut self.variadic_call) && self.target.dag.pointer == EVT::I64 {
            let vectors = plan
                .parameters
                .iter()
                .map(|parameter| match parameter {
                    Parameter::Registers(regs) => regs.iter().filter(|reg| reg.is_xmm()).count(),
                    _ => 0,
                })
                .sum::<usize>();
            let count = self.dag.get_constant(vectors as u128, EVT::I8);
            copies.push((RegId::AL, count));
        }
        if !stores.is_empty() {
            chain = self.dag.get_token_factor(&stores);
        }
        let mut glue: Option<SDValue> = None;
        for (reg, value) in &copies {
            let copy = self.dag.get_copy_to_phys_reg(chain, *reg, *value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
        }
        let mut ops = vec![chain, callee_node];
        for (reg, value) in &copies {
            ops.push(self.dag.get_phys_register(self.dag.value_type(*value), *reg));
        }
        ops.push(self.dag.get_register_mask());
        ops.extend(glue);
        if sibling {
            // `X86ISD::TC_RETURN`: the callee, the stack adjustment (none: the
            // arguments are in place), the registers
            let fpdiff = self.dag.get_constant_with(0, EVT::I32, true, false);
            ops.insert(2, fpdiff);
            // the pin has no 16-bit pattern: m16 has a node of its own
            let node = if self.target.dag.pointer == EVT::I16 {
                super::x86isd::LLRM_TC_RETURN
            } else {
                super::x86isd::TC_RETURN
            };
            let tc = self.dag.get_node_multi(node, &[EVT::OTHER], &ops);
            self.dag.set_root(tc);
            self.had_tail_call = true;
            return Ok(None);
        }
        let call = self.dag.get_node_multi(super::x86isd::CALL, &[EVT::OTHER, EVT::GLUE], &ops);
        let end = self
            .dag
            .get_callseq_end(
                SDValue { node: call.node, res: 0 },
                bytes,
                plan.popped as u64,
                SDValue { node: call.node, res: 1 },
            );
        let mut chain = SDValue { node: end.node, res: 0 };
        let glue = SDValue { node: end.node, res: 1 };
        if types.is_void(result_type) {
            self.dag.set_root(chain);
            return Ok(None);
        }
        if self.is_aggregate(result_type) {
            // `LowerCallResult` over the leaves `ComputeValueVTs` gives: one
            // return register each
            let leaves = self.leaves(result_type)?;
            // A struct of one float is answered as the float is: in st(0), or
            // in the vector register the convention names.
            if let ([(leaf, _)], true) = (leaves.as_slice(), plan.returns.is_empty()) {
                let leaf_vt = self.type_of(*leaf)?;
                if leaf_vt.is_floating_point() {
                    let piece = self.float_call_result(leaf_vt, plan.float_result, chain, glue);
                    return Ok(Some(self.merge(&[piece])));
                }
            }
            if plan.returns.len() != leaves.len() {
                return Err(Refused("an aggregate result that is not one register a member"));
            }
            let mut glue = glue;
            let mut pieces = Vec::new();
            for ((leaf, _), reg) in leaves.into_iter().zip(plan.returns.clone()) {
                let leaf_vt = self.type_of(leaf)?;
                // `getCopyFromParts`: a member between the legal widths comes
                // in a register of the register type and is truncated
                let reg_vt = match self.register_type(leaf_vt) {
                    Ok(reg_vt) if leaf_vt.is_integer() => reg_vt,
                    _ => leaf_vt,
                };
                let reg = self.sized_register(reg, reg_vt);
                let copy = self.dag.get_copy_from_phys_reg(chain, reg, reg_vt, glue);
                chain = SDValue { node: copy.node, res: 1 };
                glue = SDValue { node: copy.node, res: 2 };
                pieces.push(if reg_vt == leaf_vt { copy } else { self.dag.get_node(ISD::Truncate, leaf_vt, &[copy]) });
            }
            self.dag.set_root(chain);
            return Ok(Some(self.merge(&pieces)));
        }
        let vt = self.type_of(result_type)?;
        if flags_answer {
            let flags = self.dag.get_copy_from_phys_reg(chain, super::dagisel_adapter::EFLAGS_ID, EVT::I32, glue);
            self.dag.set_root(SDValue { node: flags.node, res: 1 });
            let flags = SDValue { node: flags.node, res: 0 };
            return Ok(Some(self.dag.get_node(super::x86isd::LLRM_CALL_FLAGS, vt, &[flags])));
        }
        if vt.is_floating_point() {
            return Ok(Some(self.float_call_result(vt, plan.float_result, chain, glue)));
        }
        let (part_vt, parts) = self.parts_in(vt, plan.returns.len())?;
        if plan.returns.len() != parts {
            return Err(Refused("result in registers the value does not fill"));
        }
        // each register read after the one before, on its chain and glue
        let mut glue = glue;
        let mut pieces = Vec::new();
        for reg in plan.returns.clone() {
            let reg = self.sized_register(reg, part_vt);
            let copy = self.dag.get_copy_from_phys_reg(chain, reg, part_vt, glue);
            chain = SDValue { node: copy.node, res: 1 };
            glue = SDValue { node: copy.node, res: 2 };
            pieces.push(copy);
        }
        self.dag.set_root(chain);
        let result = if parts == 2 { self.dag.get_node(ISD::BuildPair, vt, &pieces) } else { pieces[0] };
        Ok(Some(result))
    }

    /// The convention names a result register by its widest view; the DAG
    /// names the one the value's width is (`AX` for an `i16`, not `EAX`).
    fn sized_register(
        &self,
        reg: RegId,
        vt: EVT,
    ) -> RegId {
        self.arch.registers().named(reg, i64::from(vt.size_in_bits().div_ceil(8)))
    }

    /// `visitRet` and X86's `LowerReturn`: the value copied to the return
    /// register, then `RET_GLUE chain, popped, register, glue`.
    /// X86's `LowerCallResult` for a float: read from the vector register the
    /// convention names, else popped from st(0), which the allocator knows as
    /// `FP0`.
    fn float_call_result(
        &mut self,
        vt: EVT,
        xmm: Option<RegId>,
        chain: SDValue,
        glue: SDValue,
    ) -> SDValue {
        if let Some(xmm) = xmm {
            let copy = self.dag.get_copy_from_phys_reg(chain, xmm, vt, glue);
            self.dag.set_root(SDValue { node: copy.node, res: 1 });
            return SDValue { node: copy.node, res: 0 };
        }
        let fp0 = self.dag.get_phys_register(vt, RegId::MM0);
        let pop =
            self.dag.get_node_multi(super::x86isd::POP_FROM_X87_REG, &[vt, EVT::OTHER, EVT::GLUE], &[chain, fp0, glue]);
        self.dag.set_root(SDValue { node: pop.node, res: 1 });
        pop
    }

    fn visit_ret(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<(), Refused> {
        let mut chain = self.control_root();
        let mut ops = Vec::new();
        let mut glue = None;
        let aggregate = instruction
            .operands
            .first()
            .and_then(|&operand| self.function.operand_type(&self.module.context, operand))
            .is_some_and(|ty| self.is_aggregate(ty));
        if let (true, Some(&operand)) = (aggregate, instruction.operands.first()) {
            // `LowerReturn` over the leaves: one return register each
            let parts = self.value_parts(operand)?;
            // A struct of one float leaves as the float does: in st(0), or in
            // the vector register the convention names.
            let float_leaf = match (parts.as_slice(), self.convention.returns.is_empty()) {
                ([leaf], true) if self.dag.value_type(*leaf).is_floating_point() => Some(*leaf),
                _ => None,
            };
            if let Some(leaf) = float_leaf {
                let vt = self.dag.value_type(leaf);
                if let Some(xmm) = self.convention.float_result {
                    let copy = self.dag.get_copy_to_phys_reg(chain, xmm, leaf, glue);
                    chain = SDValue { node: copy.node, res: 0 };
                    glue = Some(SDValue { node: copy.node, res: 1 });
                    ops.push(self.dag.get_phys_register(vt, xmm));
                } else {
                    ops.push(leaf);
                }
            } else {
                if parts.len() != self.convention.returns.len() {
                    return Err(Refused("an aggregate result that is not one register a member"));
                }
                for (piece, reg) in parts.into_iter().zip(self.convention.returns.clone()) {
                    // `getCopyToParts`: a member between the legal widths
                    // (`i48`) is any-extended to the
                    // register type
                    let piece = match self.register_type(self.dag.value_type(piece)) {
                        Ok(reg_vt)
                            if reg_vt != self.dag.value_type(piece) && self.dag.value_type(piece).is_integer() =>
                        {
                            self.dag.get_node(ISD::AnyExtend, reg_vt, &[piece])
                        }
                        _ => piece,
                    };
                    let piece_vt = self.dag.value_type(piece);
                    let reg = self.sized_register(reg, piece_vt);
                    let copy = self.dag.get_copy_to_phys_reg(chain, reg, piece, glue);
                    chain = SDValue { node: copy.node, res: 0 };
                    glue = Some(SDValue { node: copy.node, res: 1 });
                    ops.push(self.dag.get_phys_register(piece_vt, reg));
                }
            }
        } else if let Some(&operand) = instruction.operands.first() {
            let value = self.get_value(operand)?;
            let vt = self.dag.value_type(value);
            if let (true, Some(xmm)) = (vt.is_floating_point(), self.convention.float_result) {
                // `LowerReturn` with SSE: copied to the vector register
                let copy = self.dag.get_copy_to_phys_reg(chain, xmm, value, glue);
                chain = SDValue { node: copy.node, res: 0 };
                glue = Some(SDValue { node: copy.node, res: 1 });
                ops.push(self.dag.get_phys_register(vt, xmm));
            } else if vt.is_floating_point() {
                // x87 returns in st(0), which no register names: the value is
                // RET_GLUE's operand
                ops.push(value);
            } else {
                let (part_vt, parts) = self.parts_in(vt, self.convention.returns.len())?;
                // a narrow integer fills its register (`getCopyToParts`, by the
                // return's `zeroext`/`signext`)
                let value = if parts == 1 && vt.is_integer() && vt.size_in_bits() < part_vt.size_in_bits() {
                    let attrs = &self.function.return_attrs;
                    let code = if llrm_mir::memory::has(attrs, "signext") {
                        ISD::SignExtend
                    } else if llrm_mir::memory::has(attrs, "zeroext") {
                        ISD::ZeroExtend
                    } else {
                        ISD::AnyExtend
                    };
                    self.dag.get_node(code, part_vt, &[value])
                } else {
                    value
                };
                let vt = self.dag.value_type(value);
                if self.convention.returns.len() != parts || (parts == 1 && part_vt != vt) {
                    return Err(Refused("result in registers the value does not fill"));
                }
                let pieces = if parts == 2 { self.split_value(value, part_vt).to_vec() } else { vec![value] };
                for (piece, reg) in pieces.into_iter().zip(self.convention.returns.clone()) {
                    let reg = self.sized_register(reg, part_vt);
                    let copy = self.dag.get_copy_to_phys_reg(chain, reg, piece, glue);
                    chain = SDValue { node: copy.node, res: 0 };
                    glue = Some(SDValue { node: copy.node, res: 1 });
                    ops.push(self.dag.get_phys_register(part_vt, reg));
                }
            }
        }
        if let Some(reg) = self.info.sret_reg {
            // the pointer is returned in eax as well
            let pointer = self.target.dag.pointer;
            let saved = self.dag.get_copy_from_reg(chain, reg, pointer);
            let eax = self.sized_register(self.arch.register_named("eax"), pointer);
            let copy = self.dag.get_copy_to_phys_reg(chain, eax, saved, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            ops.push(self.dag.get_phys_register(pointer, eax));
        }
        let popped = self.dag.get_constant_with(self.convention.popped as u128, EVT::I32, true, false);
        let mut all = vec![chain, popped];
        all.extend(ops);
        all.extend(glue);
        let node = self.dag.get_node(super::x86isd::RET_GLUE, EVT::OTHER, &all);
        self.dag.set_root(node);
        Ok(())
    }

    /// `getNonRegisterValue`.
    fn non_register_value(
        &mut self,
        operand: Operand,
    ) -> Result<SDValue, Refused> {
        match operand {
            Operand::Value(value) => {
                if let Some(found) = self.node_map.get(&value) {
                    return Ok(*found);
                }
                let made = self.value_impl(value)?;
                self.node_map.insert(value, made);
                Ok(made)
            }
            other => self.get_value(other),
        }
    }

    fn constant_value(
        &mut self,
        id: ConstantId,
    ) -> Result<SDValue, Refused> {
        let constant = self.module.context.get(id);
        if self.is_aggregate(constant.ty) {
            return self.aggregate_constant(constant.ty, id);
        }
        let vt = self.type_of(constant.ty)?;
        match &constant.kind {
            // A far constant is its offset and its selector (a null pointer is
            // two zero words): `get_constant` of the pair's type panicked
            // ('integer constant of Far', 35 QCport functions).
            ConstantKind::Int(bits) if vt == MVT::Far.into() => {
                let offset = self.dag.get_constant(*bits & 0xFFFF, EVT::I16);
                let segment = self.dag.get_constant((*bits >> 16) & 0xFFFF, EVT::I16);
                Ok(self.dag.get_node(ISD::BuildPair, vt, &[offset, segment]))
            }
            ConstantKind::Null | ConstantKind::Zero if vt == MVT::Far.into() => {
                let zero = self.dag.get_constant(0, EVT::I16);
                Ok(self.dag.get_node(ISD::BuildPair, vt, &[zero, zero]))
            }
            ConstantKind::Int(bits) => Ok(self.dag.get_constant(*bits, vt)),
            // MIR holds an `x86_fp80` constant a double holds exactly as that
            // double; the DAG's is the 80-bit encoding.
            ConstantKind::Float(bits) if vt == EVT::Simple(MVT::F80) => {
                Ok(self.dag.get_constant_fp(f80_from_double_bits(*bits), vt))
            }
            ConstantKind::Float80(bits) => Ok(self.dag.get_constant_fp(*bits, vt)),
            ConstantKind::Float(bits) => Ok(self.dag.get_constant_fp(u128::from(*bits), vt)),
            ConstantKind::Null | ConstantKind::Zero if vt.is_floating_point() => Ok(self.dag.get_constant_fp(0, vt)),
            ConstantKind::Null | ConstantKind::Zero => Ok(self.dag.get_constant(0, vt)),
            ConstantKind::Undef => Ok(self.dag.get_undef(vt)),
            ConstantKind::Poison => Ok(self.dag.get_poison(vt)),
            // a far global: its offset and the selector of its segment
            ConstantKind::Global(global) if vt == MVT::Far.into() => {
                let offset = self.dag.get_global_address(EVT::I16, global.0, 0);
                let segment = self.segment_of(Some(global.0));
                Ok(self.dag.get_node(ISD::BuildPair, vt, &[offset, segment]))
            }
            ConstantKind::Global(global) => Ok(self.dag.get_global_address(vt, global.0, 0)),
            ConstantKind::Expr(llrm_mir::context::ConstantExpr::GetElementPtr {
                source,
                inbounds,
                nusw,
                nuw,
                operands,
            }) => {
                let base = self.constant_value(operands[0])?;
                let indices: Vec<Operand> = operands[1..].iter().map(|id| Operand::Constant(*id)).collect();
                if self.dag.value_type(base) == MVT::Far.into() {
                    let pointer = Operand::Constant(operands[0]);
                    return self.far_gep(pointer, base, *source, *nuw, *nusw || *inbounds, *inbounds, &indices);
                }
                self.gep_value(base, *source, *nuw, *nusw || *inbounds, *inbounds, &indices)
            }
            ConstantKind::Expr(llrm_mir::context::ConstantExpr::Cast { op, value }) => {
                let operand = self.constant_value(*value)?;
                if *op == CastOp::AddrSpaceCast
                    || self.dag.value_type(operand) == MVT::Far.into()
                    || vt == MVT::Far.into()
                {
                    return self.pointer_cast(*op, Operand::Constant(*value), operand, constant.ty);
                }
                self.cast_value(*op, operand, vt, SDNodeFlags::default())
            }
            // `getValueImpl` visits a constant expression as the instruction it
            // is: `add (ptrtoint @a), 3` is an add of the two.
            ConstantKind::Expr(llrm_mir::context::ConstantExpr::Binary { op, lhs, rhs }) => {
                let opcode = match op {
                    BinaryOp::Add => ISD::Add,
                    BinaryOp::Sub => ISD::Sub,
                    BinaryOp::Xor => ISD::Xor,
                    _ => return Err(Refused("a constant binary operator")),
                };
                let (lhs, rhs) = (self.constant_value(*lhs)?, self.constant_value(*rhs)?);
                Ok(self.dag.get_node(opcode, vt, &[lhs, rhs]))
            }
            _ => Err(Refused("aggregate constant")),
        }
    }

    /// `getValueImpl` for an instruction or argument.
    fn value_impl(
        &mut self,
        value: ValueId,
    ) -> Result<SDValue, Refused> {
        if let Some(index) = self.info.static_allocas.get(&value) {
            let vt = self.type_of(self.function.value(value).ty)?;
            return Ok(self.dag.get_frame_index(vt, *index));
        }
        match self.function.value(value).def {
            ValueDef::Argument(_) => Err(Refused("argument read before the entry block was built")),
            // an exported value read before its definition's block: its
            // register
            ValueDef::Instruction(_) => {
                let reg = *self.info.value_regs.get(&value).ok_or(Refused("value with no register"))?;
                let ty = self.function.value(value).ty;
                if self.is_aggregate(ty) {
                    return self.copy_aggregate_from_regs(reg, ty);
                }
                let vt = self.type_of(ty)?;
                Ok(self.copy_from_reg(reg, vt)?)
            }
        }
    }

    fn copy_from_regs(
        &mut self,
        value: ValueId,
    ) -> Result<Option<SDValue>, Refused> {
        let Some(reg) = self.info.value_regs.get(&value).copied() else { return Ok(None) };
        let ty = self.function.value(value).ty;
        if self.is_aggregate(ty) {
            return Ok(Some(self.copy_aggregate_from_regs(reg, ty)?));
        }
        let vt = self.type_of(ty)?;
        Ok(Some(self.copy_from_reg(reg, vt)?))
    }

    /// `RegsForValue::getCopyFromRegs`: the register's type, truncated to the
    /// value's when the register is wider.
    fn copy_from_reg(
        &mut self,
        reg: u32,
        vt: EVT,
    ) -> Result<SDValue, Refused> {
        let (reg_vt, parts) = self.register_parts(vt)?;
        let entry = self.dag.entry_node();
        if parts == 2 {
            // each part's copy chained to the one before; the halves paired
            let regs = self.info.part_regs(reg, 2);
            let low = self.dag.get_copy_from_reg(entry, regs[0], reg_vt);
            let high = self.dag.get_copy_from_reg(SDValue { node: low.node, res: 1 }, regs[1], reg_vt);
            return Ok(self.dag.get_node(ISD::BuildPair, vt, &[low, high]));
        }
        let copy = self.dag.get_copy_from_reg(entry, reg, reg_vt);
        Ok(if reg_vt == vt { copy } else { self.dag.get_node(ISD::Truncate, vt, &[copy]) })
    }

    /// `CopyValueToVirtualRegister`.
    fn copy_value_to_register(
        &mut self,
        operand: Operand,
        reg: u32,
        extend: ISD,
    ) -> Result<(), Refused> {
        if self.function.operand_type(&self.module.context, operand).is_some_and(|ty| self.is_aggregate(ty)) {
            return self.copy_aggregate_to_registers(operand, reg);
        }
        let value = self.non_register_value(operand)?;
        self.copy_leaf_to_register(value, reg, extend)
    }

    /// `copy_value_to_register` of one scalar value.
    fn copy_leaf_to_register(
        &mut self,
        value: SDValue,
        reg: u32,
        extend: ISD,
    ) -> Result<(), Refused> {
        let vt = self.dag.value_type(value);
        let (reg_vt, parts) = self.register_parts(vt)?;
        let entry = self.dag.entry_node();
        if parts == 2 {
            let regs = self.info.part_regs(reg, 2);
            let halves = self.split_value(value, reg_vt);
            let copies: Vec<SDValue> =
                halves.iter().zip(&regs).map(|(half, reg)| self.dag.get_copy_to_reg(entry, *reg, *half)).collect();
            let chain = self.dag.get_token_factor(&copies);
            self.pending_exports.push(chain);
            return Ok(());
        }
        let value = if reg_vt == vt { value } else { self.dag.get_node(extend, reg_vt, &[value]) };
        let chain = self.dag.get_copy_to_reg(entry, reg, value);
        self.pending_exports.push(chain);
        Ok(())
    }

    fn set_value(
        &mut self,
        value: ValueId,
        node: SDValue,
    ) {
        self.node_map.insert(value, node);
    }

    // ---- instructions

    fn visit(
        &mut self,
        inst: InstId,
    ) -> Result<(), Refused> {
        self.current = Some(inst);
        // `++SDNodeOrder`: what the nodes built for this instruction carry.
        self.dag.cur_order += 1;
        let at = self.dag.cur_order as usize;
        if self.dag.lines.len() <= at {
            self.dag.lines.resize(at + 1, None);
        }
        self.dag.lines[at] = crate::backend::isel::dbg_line(self.module, self.function, inst).filter(|at| at.line != 0);
        self.dag.inst_order.push((inst, self.dag.cur_order));
        if self.prelowered.contains(&inst) || self.info.prezeroed.contains(&inst) {
            // a compare settled with its call's flags is read in other blocks
            // through its register all the same
            if self.prelowered.contains(&inst)
                && let Some(result) = self.function.instruction(inst).result
                && let Some(reg) = self.info.value_regs.get(&result).copied()
            {
                self.copy_value_to_register(Operand::Value(result), reg, ISD::AnyExtend)?;
            }
            return Ok(());
        }
        let instruction = self.function.instruction(inst).clone();
        if instruction.opcode.is_terminator() {
            self.handle_phi_nodes_in_successor_blocks()?;
        }
        match &instruction.opcode {
            Opcode::Binary(op) => self.visit_binary(&instruction, *op)?,
            Opcode::Cast(op) => self.visit_cast(&instruction, *op)?,
            Opcode::ICmp(predicate) => self.visit_icmp(&instruction, *predicate)?,
            Opcode::FCmp(predicate) => self.visit_fcmp(&instruction, *predicate)?,
            Opcode::FNeg => self.visit_fneg(&instruction)?,
            Opcode::Select => self.visit_select(&instruction)?,
            Opcode::Freeze => self.visit_freeze(&instruction)?,
            Opcode::ExtractValue(indices) => self.visit_extract_value(&instruction, indices)?,
            Opcode::InsertValue(indices) => self.visit_insert_value(&instruction, indices)?,
            Opcode::Load { align, volatile } => self.visit_load(inst, &instruction, *align, *volatile)?,
            Opcode::Store { align, volatile } => self.visit_store(&instruction, *align, *volatile)?,
            Opcode::GetElementPtr { source } => self.visit_gep(&instruction, *source)?,
            Opcode::Br => self.visit_br(&instruction)?,
            Opcode::Alloca { .. }
                if instruction.result.is_some_and(|result| self.info.static_allocas.contains_key(&result)) => {}
            Opcode::Alloca { allocated, align, .. } => self.visit_alloca(&instruction, *allocated, *align)?,
            Opcode::Unreachable => {}
            Opcode::Ret => self.visit_ret(&instruction)?,
            Opcode::Switch => self.visit_switch(&instruction)?,
            Opcode::Call(info) => {
                self.tail_candidate = self.in_tail_position(inst).then_some(inst);
                self.visit_call(&instruction, info)?;
                self.tail_candidate = None;
            }
            Opcode::Invoke(info) => self.visit_invoke(&instruction, info)?,
            // QB's pad is entered by the runtime, which `ehprepare` set up
            // and whose selector it took out: nothing to build
            Opcode::LandingPad { .. } => {
                if instruction.result.is_some_and(|value| !self.function.users(value).is_empty()) {
                    return Err(Refused("a landing pad's value read after the backend prepared it"));
                }
            }
            other => return Err(Refused(Box::leak(format!("instruction {}", other.mnemonic()).into_boxed_str()))),
        }
        if !instruction.opcode.is_terminator() {
            if let Some(result) = instruction.result {
                if let Some(reg) = self.info.value_regs.get(&result).copied() {
                    self.copy_value_to_register(Operand::Value(result), reg, ISD::AnyExtend)?;
                }
            }
        }
        Ok(())
    }

    fn visit_binary(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        op: BinaryOp,
    ) -> Result<(), Refused> {
        let opcode = match op {
            BinaryOp::Add => ISD::Add,
            BinaryOp::Sub => ISD::Sub,
            BinaryOp::Mul => ISD::Mul,
            BinaryOp::UDiv => ISD::UDiv,
            BinaryOp::SDiv => ISD::SDiv,
            BinaryOp::URem => ISD::URem,
            BinaryOp::SRem => ISD::SRem,
            BinaryOp::Shl => ISD::Shl,
            BinaryOp::LShr => ISD::Srl,
            BinaryOp::AShr => ISD::Sra,
            BinaryOp::And => ISD::And,
            BinaryOp::Or => ISD::Or,
            BinaryOp::Xor => ISD::Xor,
            BinaryOp::FAdd => ISD::FAdd,
            BinaryOp::FSub => ISD::FSub,
            BinaryOp::FMul => ISD::FMul,
            BinaryOp::FDiv => ISD::FDiv,
            BinaryOp::FRem => ISD::FRem,
        };
        let lhs = self.get_value(instruction.operands[0])?;
        let mut rhs = self.get_value(instruction.operands[1])?;
        let mut flags = flags_of(instruction.flags);
        if matches!(opcode, ISD::Shl | ISD::Srl | ISD::Sra) {
            // visitShift: the amount in the target's shift type, and only these
            // flags
            rhs = self.dag.get_zext_or_trunc(rhs, self.target.shift_amount);
            flags = SDNodeFlags(
                flags.0 & (SDNodeFlags::NO_UNSIGNED_WRAP.0 | SDNodeFlags::NO_SIGNED_WRAP.0 | SDNodeFlags::EXACT.0),
            );
        } else if matches!(opcode, ISD::SDiv) {
            flags = SDNodeFlags(flags.0 & SDNodeFlags::EXACT.0);
        }
        let vt = self.dag.value_type(lhs);
        let node = self.dag.get_node_flags(opcode, vt, &[lhs, rhs], flags);
        self.set_value(instruction.result.expect("a binary operator has a result"), node);
        Ok(())
    }

    fn visit_cast(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        op: CastOp,
    ) -> Result<(), Refused> {
        let operand = self.get_value(instruction.operands[0])?;
        let dest = self.type_of(instruction.ty)?;
        let result = if self.is_pointer_cast(op, instruction)? {
            self.pointer_cast(op, instruction.operands[0], operand, instruction.ty)?
        } else if op == CastOp::FPRound {
            // A store of the type rounds it: no slot first (the old selector's
            // single `fstp`).
            if self.read_only_by_stores(instruction) { operand } else { self.round_through_memory(operand, dest) }
        } else {
            let converted = self.cast_value(op, operand, dest, flags_of(instruction.flags))?;
            // an integer wider than the float's mantissa is rounded to its type
            let mantissa = match dest {
                EVT::Simple(MVT::F32) => 24,
                EVT::Simple(MVT::F64) => 53,
                _ => 64,
            };
            let wide = self.dag.value_type(operand).size_in_bits() > mantissa;
            // As the pin: a conversion that only feeds float arithmetic stays
            // exact, where `fimul`/`fiadd` fold it.
            if self.target.float_store
                && matches!(op, CastOp::SIToFP | CastOp::UIToFP)
                && wide
                && !self.read_only_by_float_arithmetic(instruction)
            {
                self.round_through_memory(converted, dest)
            } else {
                converted
            }
        };
        self.set_value(instruction.result.expect("a cast has a result"), result);
        Ok(())
    }

    /// Whether every reader of `instruction`'s value is a float `fadd`, `fsub`
    /// or `fmul`.
    fn read_only_by_float_arithmetic(
        &self,
        instruction: &llrm_mir::module::Instruction,
    ) -> bool {
        let Some(result) = instruction.result else { return false };
        let users = self.function.users(result);
        !users.is_empty()
            && users.iter().all(|one| {
                matches!(
                    self.function.instruction(one.user).opcode,
                    Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul)
                )
            })
    }

    /// Whether every reader of `instruction`'s value stores it: a store's
    /// value, or a call's argument (passed on the stack, so stored, at the
    /// argument's own type).
    fn read_only_by_stores(
        &self,
        instruction: &llrm_mir::module::Instruction,
    ) -> bool {
        let Some(result) = instruction.result else { return false };
        let users = self.function.users(result);
        !users.is_empty()
            && users.iter().all(|one| {
                let user = self.function.instruction(one.user);
                match user.opcode {
                    Opcode::Store { .. } => one.index == 0,
                    Opcode::Call(_) => (one.index as usize) + 1 < user.operands.len(),
                    _ => false,
                }
            })
    }

    /// `fpround`: the value stored to a stack slot of its own type and loaded
    /// back, so an x87 register holding it wider is rounded to the type's
    /// precision (what the old selector does with `fstp`/`fld`). The pin never
    /// sees this cast: its same-type `FP_ROUND` is a no-op.
    fn round_through_memory(
        &mut self,
        operand: SDValue,
        vt: EVT,
    ) -> SDValue {
        let align = vt.size_in_bits().div_ceil(8);
        let index = self.info.new_stack_object(align, align);
        let slot = self.dag.get_frame_index(self.target.dag.pointer, index);
        let info = MemInfo { mem_vt: vt, align, volatile: false, base: MemBase::Stack(index), ext: None, space: 0 };
        let root = self.root();
        let store = self.dag.get_store(root, operand, slot, info);
        self.dag.set_root(store);
        let value = self.dag.get_load(vt, store, slot, info);
        self.pending_loads.push(SDValue { node: value.node, res: 1 });
        value
    }

    /// Whether a cast moves a pointer between address spaces or to or from
    /// a two-word pointer.
    fn is_pointer_cast(
        &self,
        op: CastOp,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<bool, Refused> {
        let far: EVT = MVT::Far.into();
        Ok(matches!(op, CastOp::AddrSpaceCast)
            || (matches!(op, CastOp::PtrToInt | CastOp::IntToPtr)
                && (self.operand_type(instruction.operands[0])? == far || self.type_of(instruction.ty)? == far)))
    }

    /// A cast between address spaces: a far and a huge pointer are the same
    /// two words, `(offset, selector)`; a near pointer's offset is the
    /// first, and its selector `SS` for a frame object; a bare segment is a
    /// selector with offset 0.
    fn pointer_cast(
        &mut self,
        op: CastOp,
        pointer: Operand,
        operand: SDValue,
        to: TypeId,
    ) -> Result<SDValue, Refused> {
        let types = &self.module.context.types;
        let roles = self.arch.layout().spaces.roles;
        let from_ty = self.function.operand_type(&self.module.context, pointer).ok_or(Refused("block operand"))?;
        let far: EVT = MVT::Far.into();
        let word = self.target.dag.pointer;
        let (dest, from) = (self.type_of(to)?, self.dag.value_type(operand));
        let space_of = |ty: TypeId| match types.get(ty) {
            llrm_mir::types::Type::Pointer(space) => Some(*space),
            _ => None,
        };
        let (from_space, to_space) = (space_of(from_ty), space_of(to));
        if from == far && dest == far {
            return Ok(operand);
        }
        if from == far {
            let [offset, selector] = self.split_value(operand, word);
            return match (op, to_space) {
                (CastOp::AddrSpaceCast, Some(space)) if roles.segment == Some(space) => Ok(selector),
                (CastOp::AddrSpaceCast, Some(_)) => Ok(offset),
                (CastOp::PtrToInt, None) if dest == EVT::I32 => {
                    Ok(self.dag.get_node(ISD::BuildPair, dest, &[offset, selector]))
                }
                // truncated, as LLVM's is: segment:offset's low word
                (CastOp::PtrToInt, None) if dest == word => Ok(offset),
                _ => Err(Refused("cast of a far pointer")),
            };
        }
        if dest == far {
            return match (op, from_space) {
                (CastOp::IntToPtr, None) if from == EVT::I32 => {
                    let [offset, selector] = self.split_value(operand, word);
                    Ok(self.dag.get_node(ISD::BuildPair, far, &[offset, selector]))
                }
                (CastOp::AddrSpaceCast, Some(space)) if roles.segment == Some(space) => {
                    let zero = self.dag.get_constant(0, word);
                    Ok(self.dag.get_node(ISD::BuildPair, far, &[zero, operand]))
                }
                (CastOp::AddrSpaceCast, Some(space)) if space == roles.stack => {
                    let selector = self.stack_selector();
                    Ok(self.dag.get_node(ISD::BuildPair, far, &[operand, selector]))
                }
                (CastOp::AddrSpaceCast, Some(space)) if space == roles.near => {
                    let selector = self.near_selector(pointer);
                    Ok(self.dag.get_node(ISD::BuildPair, far, &[operand, selector]))
                }
                _ => Err(Refused("cast to a far pointer")),
            };
        }
        // a near pointer cast to the segment space (`VARSEG`) is the selector
        // of its segment, not its offset
        if op == CastOp::AddrSpaceCast
            && roles.segment.is_some()
            && to_space == roles.segment
            && from_space.is_some_and(|space| space == roles.near || space == roles.stack)
        {
            return Ok(self.near_selector(pointer));
        }
        // one word to one word: the near and stack spaces share their offsets
        match op {
            CastOp::AddrSpaceCast if from == dest => Ok(operand),
            _ => Err(Refused("cast between address spaces")),
        }
    }

    /// The selector of the segment the near pointer `pointer` points into: a
    /// frame object is in SS; a global is in its own segment; any other near
    /// pointer is into DGROUP.
    fn near_selector(
        &mut self,
        pointer: Operand,
    ) -> SDValue {
        let (base, _) = llrm_mir::valuetracking::underlying(&self.module.context, self.layout, self.function, pointer);
        let framed = matches!(
            base,
            Operand::Value(value) if matches!(
                self.function.value(value).def,
                ValueDef::Instruction(def) if matches!(self.function.instruction(def).opcode, Opcode::Alloca { .. })
            )
        );
        match base {
            _ if framed => self.stack_selector(),
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Global(global) => self.segment_of(Some(global.0)),
                _ => self.segment_of(None),
            },
            _ => self.segment_of(None),
        }
    }

    /// The selector of the segment `global` lives in (`SEGMENT_OF`).
    fn segment_of(
        &mut self,
        global: Option<u32>,
    ) -> SDValue {
        self.dag.get_segment_of(global)
    }

    /// `SS`, the stack's selector.
    fn stack_selector(&mut self) -> SDValue {
        let ss = self.sized_register(self.arch.register_named("ss"), EVT::I16);
        let entry = self.dag.entry_node();
        self.dag.get_copy_from_phys_reg_plain(entry, ss, EVT::I16)
    }

    /// The node a cast makes of `operand` (`visitTrunc`, `visitZExt`, ...).
    fn cast_value(
        &mut self,
        op: CastOp,
        operand: SDValue,
        dest: EVT,
        flags: SDNodeFlags,
    ) -> Result<SDValue, Refused> {
        let far: EVT = MVT::Far.into();
        if self.dag.value_type(operand) == far || dest == far {
            return Err(Refused("cast of a far pointer constant"));
        }
        Ok(match op {
            CastOp::Trunc => self.dag.get_node_flags(ISD::Truncate, dest, &[operand], flags),
            CastOp::ZExt => self.dag.get_node_flags(ISD::ZeroExtend, dest, &[operand], flags),
            CastOp::SExt => self.dag.get_node(ISD::SignExtend, dest, &[operand]),
            CastOp::PtrToInt | CastOp::IntToPtr => self.dag.get_zext_or_trunc(operand, dest),
            CastOp::BitCast => {
                if dest != self.dag.value_type(operand) {
                    self.dag.get_node(ISD::BitCast, dest, &[operand])
                } else if let Some(bits) = self.dag.constant(operand) {
                    self.dag.get_constant_with(bits, dest, false, true)
                } else {
                    operand
                }
            }
            CastOp::FPExt => self.dag.get_node(ISD::FpExtend, dest, &[operand]),
            CastOp::FPTrunc => {
                let zero = self.dag.get_constant_with(0, self.target.dag.pointer, true, false);
                self.dag.get_node(ISD::FpRound, dest, &[operand, zero])
            }
            CastOp::FPToSI => self.dag.get_node(ISD::FpToSInt, dest, &[operand]),
            CastOp::FPToUI => self.dag.get_node(ISD::FpToUInt, dest, &[operand]),
            CastOp::SIToFP => self.dag.get_node(ISD::SIntToFp, dest, &[operand]),
            CastOp::UIToFP => self.dag.get_node_flags(ISD::UIntToFp, dest, &[operand], flags),
            other => return Err(Refused(Box::leak(format!("cast {other:?}").into_boxed_str()))),
        })
    }

    fn visit_icmp(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        predicate: IntPredicate,
    ) -> Result<(), Refused> {
        let lhs = self.get_value(instruction.operands[0])?;
        let rhs = self.get_value(instruction.operands[1])?;
        let dest = self.type_of(instruction.ty)?;
        let cc = self.dag.get_cond_code(cond_code(predicate));
        let flags = flags_of(instruction.flags);
        let node =
            self.dag.get_node_flags(ISD::SetCC, dest, &[lhs, rhs, cc], SDNodeFlags(flags.0 & SDNodeFlags::SAME_SIGN.0));
        self.set_value(instruction.result.expect("a compare has a result"), node);
        Ok(())
    }

    fn visit_fcmp(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        predicate: llrm_mir::opcode::FloatPredicate,
    ) -> Result<(), Refused> {
        let lhs = self.get_value(instruction.operands[0])?;
        let rhs = self.get_value(instruction.operands[1])?;
        let dest = self.type_of(instruction.ty)?;
        let cc = self.dag.get_cond_code(float_cond_code(predicate));
        let flags = flags_of(instruction.flags);
        let flags = SDNodeFlags(
            flags.0
                & (SDNodeFlags::NO_NANS.0
                    | SDNodeFlags::NO_INFS.0
                    | SDNodeFlags::NO_SIGNED_ZEROS.0
                    | SDNodeFlags::ALLOW_RECIPROCAL.0
                    | SDNodeFlags::ALLOW_CONTRACT.0
                    | SDNodeFlags::ALLOW_REASSOC.0),
        );
        let node = self.dag.get_node_flags(ISD::SetCC, dest, &[lhs, rhs, cc], flags);
        self.set_value(instruction.result.expect("a compare has a result"), node);
        Ok(())
    }

    fn visit_fneg(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<(), Refused> {
        let operand = self.get_value(instruction.operands[0])?;
        let vt = self.dag.value_type(operand);
        let node = self.dag.get_node_flags(ISD::FNeg, vt, &[operand], flags_of(instruction.flags));
        self.set_value(instruction.result.expect("an fneg has a result"), node);
        Ok(())
    }

    /// `visitExtractValue`: result `k` of the multi-result node the aggregate
    /// is. Only that is built; an aggregate in memory or registers is not.
    /// `visitAlloca` for a dynamic one: the size scaled by the element size,
    /// rounded up to the stack alignment, and `DYNAMIC_STACKALLOC` on the
    /// root.
    fn visit_alloca(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        allocated: TypeId,
        align: Option<u64>,
    ) -> Result<(), Refused> {
        let types = &self.module.context.types;
        let pointer = self.target.dag.pointer;
        // no operand is the array size 1 of an `i32`
        let count = match instruction.operands.first() {
            Some(operand) => self.get_value(*operand)?,
            None => self.dag.get_constant(1, EVT::I32),
        };
        let element = self.layout.alloc_size(types, allocated);
        let element = self.dag.get_constant(u128::from(element), EVT::I64);
        let count = self.dag.get_zext_or_trunc(count, pointer);
        let element = self.dag.get_zext_or_trunc(element, pointer);
        let size = self.dag.get_node(ISD::Mul, pointer, &[count, element]);
        let stack_align = self.target.stack_align.unwrap_or_else(|| self.arch.stack_alignment().max(1) as u64);
        let mask = stack_align - 1;
        // an alignment no greater than the stack's is the stack's: operand 0
        let wanted = self.layout.pref_align(types, allocated).max(align.unwrap_or(1));
        let alignment = if wanted > stack_align { wanted } else { 0 };
        let mask_node = self.dag.get_constant(u128::from(mask), pointer);
        let size = self.dag.get_node_flags(ISD::Add, pointer, &[size, mask_node], SDNodeFlags::NO_UNSIGNED_WRAP);
        let high = self.dag.get_constant(u128::from(!mask) & self::mask(pointer.size_in_bits()), pointer);
        let size = self.dag.get_node(ISD::And, pointer, &[size, high]);
        let alignment = self.dag.get_constant(u128::from(alignment), pointer);
        let root = self.root();
        let node = self.dag.get_node_multi(ISD::DynamicStackAlloc, &[pointer, EVT::OTHER], &[root, size, alignment]);
        self.dag.set_root(SDValue { node: node.node, res: 1 });
        self.set_value(instruction.result.ok_or(Refused("an alloca without a result"))?, node);
        Ok(())
    }

    fn visit_freeze(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<(), Refused> {
        let value = self.get_value(instruction.operands[0])?;
        let vt = self.dag.value_type(value);
        let node = self.dag.get_node(ISD::Freeze, vt, &[value]);
        self.set_value(instruction.result.expect("a freeze has a result"), node);
        Ok(())
    }

    fn visit_select(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<(), Refused> {
        if self.is_aggregate(instruction.ty) {
            return self.visit_aggregate_select(instruction);
        }
        let cond = self.get_value(instruction.operands[0])?;
        let then = self.get_value(instruction.operands[1])?;
        let other = self.get_value(instruction.operands[2])?;
        let vt = self.dag.value_type(then);
        let node = self.dag.get_node(ISD::Select, vt, &[cond, then, other]);
        self.set_value(instruction.result.expect("a select has a result"), node);
        Ok(())
    }

    fn mem_info(
        &self,
        ty: TypeId,
        align: Option<u64>,
        volatile: bool,
        pointer: Operand,
    ) -> Result<MemInfo, Refused> {
        let mem_vt = self.type_of(ty)?;
        let align = align.unwrap_or_else(|| self.layout.align(&self.module.context.types, ty)) as u32;
        let base = match pointer {
            Operand::Constant(id) => match self.module.context.get(id).kind {
                ConstantKind::Global(global) => MemBase::Global(global.0),
                _ => MemBase::Unknown,
            },
            Operand::Value(value) => {
                self.info.static_allocas.get(&value).map_or(MemBase::Unknown, |index| MemBase::Stack(*index))
            }
            Operand::Block(_) => MemBase::Unknown,
        };
        // a far access goes through the offset half after `LES`, so `ES`
        // (llrm's own space 259); the pin's 256..258 are `gs`, `fs`, `ss`
        let space =
            match self.function.operand_type(&self.module.context, pointer).map(|ty| self.module.context.types.get(ty))
            {
                Some(Type::Pointer(_)) if self.operand_type(pointer)? == MVT::Far.into() => 259,
                Some(Type::Pointer(space)) if (256..259).contains(space) => *space,
                _ => 0,
            };
        Ok(MemInfo { mem_vt, align, volatile, base, ext: None, space })
    }

    fn visit_load(
        &mut self,
        inst: llrm_mir::module::InstId,
        instruction: &llrm_mir::module::Instruction,
        align: Option<u64>,
        volatile: bool,
    ) -> Result<(), Refused> {
        if self.is_aggregate(instruction.ty) {
            return self.visit_aggregate_load(instruction, align, volatile);
        }
        let ptr = self.get_value(instruction.operands[0])?;
        let info = self.mem_info(instruction.ty, align, volatile, instruction.operands[0])?;
        // `DAG.getRoot()`, not the builder's: a non-volatile load leaves the
        // pending loads alone
        let root = if volatile { self.root() } else { self.dag.get_root() };
        let address = self.dag.get_object_ptr_offset(ptr, 0);
        let load = self.dag.get_load(info.mem_vt, root, address, info);
        if crate::backend::isel::reads_invariant_memory(self.module, self.function, self.layout, inst) {
            self.dag.invariant_loads.insert(load.node);
        }
        let chain = SDValue { node: load.node, res: 1 };
        let token = self.dag.get_token_factor(&[chain]);
        if volatile {
            self.dag.set_root(token);
        } else {
            self.pending_loads.push(token);
        }
        self.set_value(instruction.result.expect("a load has a result"), load);
        Ok(())
    }

    fn visit_store(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        align: Option<u64>,
        volatile: bool,
    ) -> Result<(), Refused> {
        if self
            .function
            .operand_type(&self.module.context, instruction.operands[0])
            .is_some_and(|ty| self.is_aggregate(ty))
        {
            return self.visit_aggregate_store(instruction, align, volatile);
        }
        let value_type = self
            .function
            .operand_type(&self.module.context, instruction.operands[0])
            .ok_or(Refused("block operand"))?;
        let src = self.get_value(instruction.operands[0])?;
        let ptr = self.get_value(instruction.operands[1])?;
        let info = self.mem_info(value_type, align, volatile, instruction.operands[1])?;
        let root = if volatile { self.root() } else { self.memory_root() };
        let address = self.dag.get_object_ptr_offset(ptr, 0);
        let store = self.dag.get_store(root, src, address, info);
        let node = self.dag.get_token_factor(&[store]);
        self.dag.set_root(node);
        Ok(())
    }

    /// `visitGetElementPtr`.
    fn visit_gep(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
        source: TypeId,
    ) -> Result<(), Refused> {
        use llrm_mir::opcode::Flags;
        let base = self.get_value(instruction.operands[0])?;
        let nuw = instruction.flags.contains(Flags::NUW);
        let inbounds = instruction.flags.contains(Flags::INBOUNDS);
        let nusw = instruction.flags.contains(Flags::NUSW) || inbounds;
        let n = if self.dag.value_type(base) == MVT::Far.into() {
            self.far_gep(instruction.operands[0], base, source, nuw, nusw, inbounds, &instruction.operands[1..])?
        } else {
            self.gep_value(base, source, nuw, nusw, inbounds, &instruction.operands[1..])?
        };
        self.set_value(instruction.result.expect("a gep has a result"), n);
        Ok(())
    }

    /// A GEP on a two-word pointer. A far one wraps in its offset word and
    /// keeps the selector; a huge one (a displacement that carries) is the
    /// whole byte displacement added by `HUGE_ADD`, whose stride the
    /// legalizer knows.
    fn far_gep(
        &mut self,
        pointer: Operand,
        base: SDValue,
        source: TypeId,
        nuw: bool,
        nusw: bool,
        inbounds: bool,
        indices: &[Operand],
    ) -> Result<SDValue, Refused> {
        let ty = self.function.operand_type(&self.module.context, pointer).ok_or(Refused("block operand"))?;
        let llrm_mir::types::Type::Pointer(space) = self.module.context.types.get(ty) else {
            return Err(Refused("far gep on a non-pointer"));
        };
        let pointer_vt = self.target.dag.pointer;
        if self.layout.carries(*space) {
            let zero = self.dag.get_constant(0, EVT::I32);
            let delta = self.gep_value(zero, source, nuw, nusw, inbounds, indices)?;
            return Ok(self.dag.get_node(super::x86_lowering::x86isd::HUGE_ADD, MVT::Far.into(), &[base, delta]));
        }
        let [offset, selector] = self.split_value(base, pointer_vt);
        let moved = self.gep_value(offset, source, nuw, nusw, inbounds, indices)?;
        Ok(self.dag.get_node(ISD::BuildPair, MVT::Far.into(), &[moved, selector]))
    }

    /// `visitGetElementPtr`'s walk over the indices, for an instruction or a
    /// constant expression.
    fn gep_value(
        &mut self,
        base: SDValue,
        source: TypeId,
        nuw: bool,
        nusw: bool,
        inbounds: bool,
        indices: &[Operand],
    ) -> Result<SDValue, Refused> {
        let types = &self.module.context.types;
        let mut n = base;
        let ptr_vt = self.dag.value_type(n);
        let index_bits = ptr_vt.size_in_bits();
        let mut current = source;
        for (at, index) in indices.iter().enumerate() {
            let constant = match index {
                Operand::Constant(id) => match self.module.context.get(*id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    ConstantKind::Null | ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            };
            let element = if at == 0 {
                self.layout.alloc_size(types, current)
            } else if let Type::Array { element, .. } | Type::Vector { element, .. } = types.get(current) {
                current = *element;
                self.layout.alloc_size(types, current)
            } else {
                let field = constant.ok_or(Refused("variable struct index"))? as usize;
                let offset = self.layout.struct_layout(types, current).1[field];
                current = types.member(current, field as u64).ok_or(Refused("bad struct field"))?;
                if field != 0 {
                    let flags = SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw || nusw);
                    let flags = flags.with(SDNodeFlags::IN_BOUNDS, inbounds);
                    let offset = self.dag.get_constant(u128::from(offset), ptr_vt);
                    n = self.dag.get_mem_base_plus_offset(n, offset, flags);
                }
                continue;
            };
            if constant == Some(0) {
                continue;
            }
            if let Some(bits) = constant {
                let idx_ty = self.index_operand_width(*index)?;
                let signed = ((bits << (128 - idx_ty)) as i128) >> (128 - idx_ty);
                let offs = (signed as u128).wrapping_mul(u128::from(element)) & mask(index_bits);
                let non_negative = offs >> (index_bits - 1) == 0;
                let flags = SDNodeFlags::default()
                    .with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw || (non_negative && nusw))
                    .with(SDNodeFlags::IN_BOUNDS, inbounds);
                let offs = self.dag.get_constant(offs, ptr_vt);
                n = self.dag.get_mem_base_plus_offset(n, offs, flags);
                continue;
            }
            let idx = self.get_value(*index)?;
            let mut idx = self.dag.get_sext_or_trunc(idx, ptr_vt);
            let scale_flags =
                SDNodeFlags::default().with(SDNodeFlags::NO_SIGNED_WRAP, nusw).with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw);
            if element != 1 {
                if element.is_power_of_two() {
                    let amount = self.dag.get_constant(u128::from(element.trailing_zeros()), self.target.shift_amount);
                    idx = self.dag.get_node_flags(ISD::Shl, ptr_vt, &[idx, amount], scale_flags);
                } else {
                    let scale = self.dag.get_constant(u128::from(element), ptr_vt);
                    idx = self.dag.get_node_flags(ISD::Mul, ptr_vt, &[idx, scale], scale_flags);
                }
            }
            let add_flags =
                SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, nuw).with(SDNodeFlags::IN_BOUNDS, inbounds);
            n = self.dag.get_mem_base_plus_offset(n, idx, add_flags);
        }
        Ok(n)
    }

    fn index_operand_width(
        &self,
        operand: Operand,
    ) -> Result<u32, Refused> {
        Ok(self.operand_type(operand)?.size_in_bits())
    }

    /// `visitUncondBr` and `visitCondBr` (through `visitSwitchCase`), with
    /// `FindMergedConditions` (`mergedcond.rs`).
    fn visit_br(
        &mut self,
        instruction: &llrm_mir::module::Instruction,
    ) -> Result<(), Refused> {
        let next = self.next_block();
        let block_of = |operand: Operand| match operand {
            Operand::Block(block) => block,
            _ => unreachable!("a branch target is a block"),
        };
        if instruction.operands.len() == 1 {
            let target = block_of(instruction.operands[0]);
            if Some(target) != next {
                let root = self.control_root();
                let dest = self.dag.get_basic_block(target.0);
                let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
                self.dag.set_root(node);
            }
            return Ok(());
        }
        let (on_true, on_false) = (block_of(instruction.operands[1]), block_of(instruction.operands[2]));
        if self.visit_merged_branch(instruction, on_true, on_false)? {
            return Ok(());
        }
        let mut cond = self.get_value(instruction.operands[0])?;
        let (true_prob, false_prob) =
            (self.edge_probability(self.block, on_true), self.edge_probability(self.block, on_false));
        self.add_successor_with_prob(self.block, on_true, true_prob);
        self.add_successor_with_prob(self.block, on_false, false_prob);
        let (mut on_true, mut on_false) = (on_true, on_false);
        if Some(on_true) == next {
            std::mem::swap(&mut on_true, &mut on_false);
            let one = self.dag.get_constant(1, self.dag.value_type(cond));
            cond = self.dag.get_node(ISD::Xor, self.dag.value_type(cond), &[cond, one]);
        }
        let root = self.control_root();
        let dest = self.dag.get_basic_block(on_true.0);
        let brcond = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, cond, dest]);
        let fall = self.dag.get_basic_block(on_false.0);
        let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[brcond, fall]);
        self.dag.set_root(node);
        Ok(())
    }

    /// `getEdgeProbability(Src, Dst)` of the IR's `BranchProbabilityInfo`.
    fn edge_probability(
        &mut self,
        from: BlockId,
        to: BlockId,
    ) -> super::switchcg::BranchProbability {
        let bpi = self
            .info
            .bpi
            .get_or_insert_with(
                || llrm_analysis::bpi::BranchProbabilityInfo::of(
                    llrm_analysis::bpi::Module {
                        context: &self.module.context,
                        declarations: &self.module.globals,
                        metadata: &self.module.metadata,
                        layout: self.layout,
                    },
                    self.function,
                ),
            );
        super::switchcg::BranchProbability(bpi.get_edge_probability_to(self.function, from, to).get_numerator())
    }

    /// `addSuccessorWithProb`: the edge `from` to `to` of the machine CFG.
    fn add_successor_with_prob(
        &mut self,
        from: BlockId,
        to: BlockId,
        probability: super::switchcg::BranchProbability,
    ) {
        self.info.edge_probs.push((from, to, probability.0));
    }

    fn next_block(&self) -> Option<BlockId> {
        let at = self.order.iter().position(|block| *block == self.block)?;
        self.order.get(at + 1).copied()
    }

    /// `HandlePHINodesInSuccessorBlocks`: the incoming value of each phi of
    /// each successor copied to a register, once per successor.
    fn handle_phi_nodes_in_successor_blocks(&mut self) -> Result<(), Refused> {
        let successors: Vec<BlockId> = self.function.successors(self.block).iter().copied().collect();
        for successor in successors {
            let phis: Vec<InstId> = self
                .function
                .iter_block(successor)
                .take_while(|inst| self.function.instruction(*inst).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                let instruction = self.function.instruction(phi).clone();
                let result = instruction.result.expect("a phi has a result");
                if self.function.users(result).is_empty() {
                    continue;
                }
                let incoming = instruction
                    .operands
                    .chunks(2)
                    .find(|pair| pair[1] == Operand::Block(self.block))
                    .map(|pair| pair[0])
                    .ok_or(Refused("phi without an input from a predecessor"))?;
                let reg = match incoming {
                    Operand::Constant(constant) => match self.constants_out.get(&constant) {
                        Some(reg) => *reg,
                        None => {
                            let reg = self.info.new_reg();
                            self.constants_out.insert(constant, reg);
                            let extend = match self.module.context.get(constant).kind {
                                ConstantKind::Int(_) => ISD::ZeroExtend,
                                _ => ISD::AnyExtend,
                            };
                            self.copy_value_to_register(incoming, reg, extend)?;
                            reg
                        }
                    },
                    Operand::Value(value) => match self.info.value_regs.get(&value) {
                        Some(reg) => *reg,
                        None => {
                            let reg = self.info.new_reg();
                            self.copy_value_to_register(incoming, reg, ISD::AnyExtend)?;
                            reg
                        }
                    },
                    Operand::Block(_) => return Err(Refused("block as a phi input")),
                };
                let parts = if self.is_aggregate(instruction.ty) {
                    self.aggregate_registers(instruction.ty)?.iter().map(|(_, parts)| parts).sum()
                } else {
                    let phi_type = self.type_of(instruction.ty)?;
                    self.register_parts(phi_type)?.1
                };
                for reg in self.info.part_regs(reg, parts) {
                    self.phi_updates.push((result, reg));
                }
            }
        }
        self.constants_out.clear();
        Ok(())
    }
}

/// The x87 extended encoding (sign, 15-bit exponent, explicit-integer 64-bit
/// mantissa) of the IEEE double with these bits.
pub(super) fn f80_from_double_bits(bits: u64) -> u128 {
    let sign = u128::from(bits >> 63) << 79;
    let exponent = ((bits >> 52) & 0x7FF) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let (exponent, mantissa) = match (exponent, fraction) {
        (0, 0) => return sign,
        // infinity and NaN keep their payload below the integer bit
        (0x7FF, _) => (0x7FFF, 1u64 << 63 | fraction << 11),
        // a double denormal is a normal extended number
        (0, _) => {
            let top = 63 - fraction.leading_zeros() as i32;
            (16383 + top - 1074, fraction << (63 - top))
        }
        _ => (exponent - 1023 + 16383, 1u64 << 63 | fraction << 11),
    };
    sign | (exponent as u128) << 64 | u128::from(mantissa)
}

#[cfg(test)]
mod f80_tests {
    use llrm_mir::datalayout::DataLayout;

    use super::super::dag::NodeData;
    use super::super::function_lowering::FunctionLoweringInfo;
    use super::super::isd::ISD;
    use super::super::mvt::MVT;
    use super::{BuilderTarget, build_block, f80_from_double_bits};

    /// A long double constant in a block is the x87 encoding in the DAG, for
    /// the pool and for `fldz`/`fld1`.
    #[test]
    fn a_long_double_constant_is_built_in_the_extended_encoding() {
        let text = "target datalayout = \"e-p:32:32-i64:32-f80:32-n8:16:32\"\ndefine void @f(ptr %p) {\n  store x86_fp80 3.5, ptr %p\n  ret void\n}\n";
        let module = llrm_mir::parse::module(text).expect("parses");
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let (id, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("f");
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        let dag = build_block(
            &module,
            function,
            &layout,
            &BuilderTarget::m32(),
            &llrm_x86_m32::M32,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{}", why.0))
        .dag;
        let bits: Vec<u128> = dag
            .nodes()
            .iter()
            .filter(|node| node.opcode == ISD::ConstantFP && node.values[0] == MVT::F80.into())
            .filter_map(|node| match node.data {
                NodeData::ConstFp { bits } => Some(bits),
                _ => None,
            })
            .collect();
        assert_eq!(bits, [0x4000_E000_0000_0000_0000]);
    }

    /// `long double a = 1.0l; b = 3.5l; c = -2.25l;` stored the double's bits
    /// as the extended encoding, so the pool held garbage and the program
    /// printed 0 for 126 (long_double_constant_store.c).
    #[test]
    fn a_double_is_re_encoded_as_x87_extended() {
        for (value, extended) in [
            (1.0f64, 0x3FFF_8000_0000_0000_0000u128),
            (3.5, 0x4000_E000_0000_0000_0000),
            (-2.25, 0xC000_9000_0000_0000_0000),
            (0.0, 0),
            (-0.0, 1 << 79),
            (0.5, 0x3FFE_8000_0000_0000_0000),
            (f64::INFINITY, 0x7FFF_8000_0000_0000_0000),
            (f64::MIN_POSITIVE / 2.0, 0x3C00_8000_0000_0000_0000),
        ] {
            assert_eq!(f80_from_double_bits(value.to_bits()), extended, "{value}");
        }
    }
}

/// Whether `direct` names a `llrm.qb.` routine.
fn is_runtime_routine(
    builder: &Builder<'_>,
    direct: Option<llrm_mir::context::GlobalId>,
) -> bool {
    direct
        .and_then(|global| builder.module.global(global).name.owned())
        .is_some_and(|name| name.starts_with(llrm_hir::mir::RUNTIME))
}
