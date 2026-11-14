//! A target's calling conventions, from its `calling.toml`: one table a
//! convention, in the order the file gives them, the first being the one a
//! language's own functions use. Registers are named as the target's
//! `registers.regs` names them; the schema takes no key it does not read.

use std::collections::BTreeMap;

/// Which end of the argument list is pushed first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Order {
    RightToLeft,
    LeftToRight,
}

/// Who takes the arguments off the stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cleanup {
    Caller,
    Callee,
}

/// A register kept for the caller: its full register, and the one a prologue
/// pushes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Kept {
    pub full: String,
    pub pushed: String,
}

/// How a function that returns a struct larger than a register does it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Aggregate {
    /// `hidden-pointer`: the caller passes the address of memory for it.
    pub style: String,
    /// The sizes, in bytes, of a struct returned in a register.
    pub in_register_bytes: Vec<i64>,
    /// `after-arguments`: pushed past the last argument; `register`: held in
    /// `pointer_register`.
    pub pointer: String,
    pub pointer_register: Option<String>,
    /// The address is a far pointer where the target has far data (Borland's);
    /// else a near one.
    pub pointer_far: bool,
    pub pointer_returned: String,
    pub pointer_popped_by: String,
}

/// One calling convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Convention {
    pub name: String,
    /// The calling convention MIR names this one by (`cdecl`, `watcall`); the
    /// file's first answers MIR's `ccc` as well.
    pub cc: Option<String>,
    /// What a byte argument takes on the stack, and the least a cell holds.
    pub slot_bytes: i64,
    /// The bytes the stack pointer is a multiple of at a call, where that is
    /// more than a slot: a double on the stack is then aligned (Intel486 data
    /// sheet 240440-002, Table 2.1). A slot's worth where the file gives none.
    pub stack_alignment: Option<i64>,
    pub order: Order,
    pub cleanup: Cleanup,
    pub argument_registers: Vec<String>,
    /// The vector registers a `float` or `double` argument takes, in order,
    /// none where floats travel in memory.
    pub float_argument_registers: Vec<String>,
    /// The pairs an i64 argument takes, low register first.
    pub wide_pairs: Vec<[String; 2]>,
    /// The pairs a pointer argument takes where they are not `wide_pairs`
    /// (llvm-mos: a pointer is never in A:X); empty means `wide_pairs`.
    pub pointer_pairs: Vec<[String; 2]>,
    /// The alignment of a wide argument on the stack, where it is more than a
    /// slot (SysV x86-64: an `__int128` is 16-aligned).
    pub wide_stack_align: Option<i64>,
    /// Whether a register an argument skipped is free for a later one.
    pub backfill: bool,
    /// Argument `n` takes register `n` of its own file, integer or float, and
    /// the stack slot `n` otherwise (Win64: RCX, RDX, R8, R9 and XMM0-3 share
    /// positions).
    pub positional: bool,
    /// Bytes below the arguments the caller reserves in every call and the
    /// callee may use (Win64's 32-byte home area of the register arguments).
    pub shadow_bytes: i64,
    /// The stack pointer moves only in the prologue and epilogue (Win64: "it
    /// is not valid to change the stack pointer outside the prolog/epilog"):
    /// a call's area is reserved in the frame, whatever the call needs.
    pub reserve_call_frame: bool,
    /// The MIR conventions besides C's that functions and calls in this
    /// ABI take (`serves = ["fast"]`): Win64 sends every one through
    /// `CC_X86_Win64_C`.
    pub serves: Vec<String>,
    /// The ABI guarantees the stack is a multiple of `stack_alignment` at every
    /// call, whatever the callee holds: every function that calls frames
    /// itself to it.
    pub aligned_calls: bool,
    /// The most bytes of an integer one argument register holds, where that is
    /// more than a stack slot (a long in a 32-bit register on a 16-bit
    /// target); a slot's worth where the file gives none.
    pub register_bytes: Option<i64>,
    /// A register takes the width of the value it holds (AL, AX or EAX for 1, 2
    /// or 4 bytes) rather than its own: the argument registers are named by
    /// their widest part.
    pub sized_arguments: bool,
    /// An argument that travels in memory (a float, an i64, a struct) leaves
    /// the registers free for the arguments after it.
    pub skip_memory: bool,
    /// A struct passed by value travels in memory whatever its size; else a
    /// struct of a size that returns in a register is passed as that
    /// integer.
    pub aggregate_arguments_in_memory: bool,
    /// What a call's arguments in registers are the callee's to change;
    /// everything else it keeps.
    pub arguments_clobbered: bool,
    /// The allocator leaves a copy between a value and the register an
    /// argument arrives in to itself, as the pin does for a register that is
    /// not reserved (`canJoinPhys`), instead of joining it away. Off where the
    /// file does not say.
    pub pinned_copies_unjoined: bool,
    /// The same for the registers a result leaves in.
    pub results_clobbered: bool,
    /// Functions of this name that no function of the module calls and whose
    /// address is not taken keep nothing for their caller: the run-time
    /// start-up that calls them discards every register once they return.
    pub entry_clobbers_all: Vec<String>,
    /// The convention a variadic call uses in place of this one: it moves the
    /// arguments, not the symbol.
    pub variadic: Option<String>,
    /// The convention a function of this one's takes when the callee, not the
    /// caller, removes its stack arguments (`calleepop`), where that is no
    /// convention MIR already names (`fastcc` is C's).
    pub pops_as: Option<String>,
    /// No callee-pops twin of this convention exists: the pin's `isCalleePop`
    /// is false for `fastcc` on x86-64 (no `GuaranteedTailCallOpt`), so a
    /// function never takes a `ret N` form of it and `fastcc` is not one.
    pub no_pop_twin: bool,
    /// How a symbol is written in each object format, `*` standing for its name
    /// and `^*` for its name in capitals (`spell`).
    pub symbol: BTreeMap<String, String>,
    pub return_address_bytes: i64,
    /// Where the first argument lies from the frame register, past the saved
    /// frame register and the return address.
    pub first_argument_offset: i64,
    /// The same when the call is far, where the convention has far calls.
    pub first_argument_offset_far: Option<i64>,
    /// The register a frame's cells are addressed through, and the stack
    /// pointer.
    pub frame: String,
    /// A function that needs no frame register may leave it out: its cells are
    /// addressed through the stack pointer, and the frame register is not
    /// set. gcc's `-fomit-frame-pointer`.
    pub frame_optional: bool,
    /// The registers a function keeps are pushed before the space for its
    /// locals whether or not it calls (`X86FrameLowering` on x86-64); left
    /// false where a frame with no call keeps them after.
    pub frame_saved_first: bool,
    /// A frame tuned for size is opened with `enter N,0` (4 bytes against 6).
    /// Left false where the target prices it above `push bp; mov bp,sp; sub
    /// sp,N`: the 486 takes 14 clocks against 3 (Intel 240440-002), and neither
    /// GCC nor LLVM emits it.
    pub frame_enter: bool,
    /// A size build may set the frame up after an early exit that shares its
    /// return with the late path (the exit then jumps over the epilogue). Left
    /// false where that jump outgrows its short form (QCport m16 -Os +9 B).
    pub frame_wrap_split_size: bool,
    /// The locals' base takes the widest object's alignment and the call
    /// alignment is met by a pad, as the pin's 64-bit `X86FrameLowering` lays
    /// a frame out (`calculateFrameObjectOffsets`).
    pub frame_object_aligned: bool,
    /// Bytes below the stack pointer the ABI keeps safe from signal handlers
    /// (the System V red zone): a leaf function's locals live there, with
    /// no `sub`.
    pub frame_red_zone: i64,
    /// The registers a frame of one stack slot pushes and pops instead of a
    /// `sub`/`add` of the stack pointer: the pushed one holds nothing a
    /// push may not read, the popped one is a caller-saved register no result
    /// or tail call argument is in.
    pub frame_slot_scratch: Option<(String, String)>,
    /// A sibling call may end a one-slot frame: its `pop` register is chosen among those its
    /// arguments are not in (`findDeadCallerSavedReg` of a `TCRETURN`). The target's other frames
    /// keep the `sub`/`add` where a return is a sibling call.
    pub frame_slot_sibling: bool,
    pub stack: String,
    /// The registers a callee keeps, the frame register among them.
    pub preserved: Vec<Kept>,
    pub clobbered: Vec<String>,
    /// What holds on entry and on return: `df_clear`, `x87_empty`.
    pub entry_state: Vec<String>,
    /// The registers a result leaves in, by its width in bytes, or by a class
    /// (`pointer`, `float`).
    pub results: BTreeMap<String, Vec<String>>,
    pub aggregate: Option<Aggregate>,
    pub promotion: String,
    /// How many slots an i64 or a double takes.
    pub wide_slots: i64,
    pub variadic_float: String,
    /// The register a variadic call sets, as a byte, to the number of vector
    /// registers it passes arguments in (SysV AMD64's `al`); none where the
    /// convention has no such count.
    pub variadic_vector_register: Option<String>,
    /// How a variadic function's `va_list` is made: absent, the arguments are
    /// on the stack and the list points at the first; `sysv-amd64`, the
    /// unnamed argument registers are saved in a register save area and the
    /// list is the four-field structure that names it (X86ISelLowering's
    /// `LowerVASTART`, `LowerFormalArguments`).
    pub va_list: Option<String>,
    /// For an interrupt handler: what its frame pointer addresses, lowest
    /// address first, each slot's register and bytes. Empty for the rest.
    pub interrupt_frame: Vec<(String, i64)>,
    /// For an interrupt handler: the registers its entry pushes after the
    /// general ones, in push order (the frame holds them in reverse).
    pub interrupt_saves: Vec<String>,
    /// For an interrupt handler: the registers its entry loads with the data
    /// group's selector, the first from the group, each next from the one
    /// before.
    pub interrupt_data: Vec<String>,
    /// For an interrupt handler: the frame slot each parameter is, in order. A
    /// parameter is the low word of its slot, so an assignment to it is what
    /// the handler's return restores (Borland's `bp, di, si, ds, es, dx, cx,
    /// bx, ax, ip, cs, flags`). Empty: the parameters are the frame's words in
    /// address order.
    pub interrupt_parameters: Vec<String>,
}

/// A target's description of its conventions, by its address: one per target,
/// so what a profile that holds it is keyed by.
#[derive(Clone, Copy, Debug)]
pub struct Stated(pub &'static Calling);

impl PartialEq for Stated {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Stated {}

impl std::hash::Hash for Stated {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        std::ptr::hash(self.0, state);
    }
}

/// A target's conventions, in file order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Calling {
    pub conventions: Vec<Convention>,
    /// The ABI families a program may ask for with `-mabi=`: each names the
    /// convention an unmarked function has under it.
    pub abis: BTreeMap<String, String>,
    /// The family a program gets without the switch.
    pub default: String,
    /// The convention a function nothing outside the program reaches takes,
    /// whatever ABI the program has: no one else sees it, so the target's
    /// best serves. None where its default is that.
    pub private: Option<String>,
    /// The convention a private function takes under an ABI family that
    /// states its own (`[abi.<family>] private = "..."`): Win64 has one
    /// convention for every function, `fastcc` included.
    pub abi_private: BTreeMap<String, String>,
    /// The convention an instrument gives private functions in place of
    /// `private` (`LLRM_PRIVATE_WATCALL`), to compare the two. Never a default.
    pub private_instrument: Option<String>,
}

const KEYS: &[&str] = &[
    "cc",
    "wide_pairs",
    "pointer_pairs",
    "backfill",
    "positional",
    "shadow_bytes",
    "reserve_call_frame",
    "serves",
    "aligned_calls",
    "register_bytes",
    "sized_arguments",
    "skip_memory",
    "aggregate_arguments_in_memory",
    "arguments_clobbered",
    "pinned_copies_unjoined",
    "results_clobbered",
    "entry_clobbers_all",
    "variadic",
    "pops_as",
    "no_pop_twin",
    "symbol",
    "aggregate_pointer_register",
    "aggregate_pointer_far",
    "slot_bytes",
    "stack_alignment",
    "wide_stack_align",
    "order",
    "cleanup",
    "argument_registers",
    "float_argument_registers",
    "return_address_bytes",
    "first_argument_offset",
    "first_argument_offset_far",
    "frame",
    "frame_optional",
    "frame_saved_first",
    "frame_enter",
    "frame_wrap_split_size",
    "frame_object_aligned",
    "frame_red_zone",
    "frame_slot_push",
    "frame_slot_pop",
    "frame_slot_sibling",
    "stack",
    "preserved",
    "clobbered",
    "entry_state",
    "result",
    "aggregate",
    "aggregate_in_register_bytes",
    "aggregate_pointer",
    "aggregate_pointer_returned",
    "aggregate_pointer_popped_by",
    "promotion",
    "wide_slots",
    "variadic_float",
    "variadic_vector_register",
    "va_list",
    "like",
    "interrupt_frame",
    "interrupt_saves",
    "interrupt_data",
    "interrupt_parameters",
];

impl Calling {
    /// `text`, a `calling.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let mut conventions = Vec::new();
        let mut stated: Vec<(&String, toml::Table)> = Vec::new();
        let default = table
            .get("default")
            .and_then(toml::Value::as_str)
            .ok_or("calling.toml: `default` names the ABI a program has without -mabi=")?
            .to_owned();
        let mut abis = BTreeMap::new();
        let mut abi_private = BTreeMap::new();
        for (family, one) in
            table.get("abi").and_then(toml::Value::as_table).ok_or("calling.toml: no [abi.<family>]")?
        {
            let convention = one
                .as_table()
                .and_then(|one| one.get("convention"))
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("calling.toml: abi.{family}.convention is not a name"))?;
            abis.insert(family.clone(), convention.to_owned());
            if let Some(private) = one.as_table().and_then(|one| one.get("private")) {
                let private =
                    private.as_str().ok_or_else(|| format!("calling.toml: abi.{family}.private is not a name"))?;
                abi_private.insert(family.clone(), private.to_owned());
            }
        }
        for (name, value) in table
            .iter()
            .filter(|(name, _)| !matches!(
                name.as_str(),
                "default" | "abi" | "private" | "private_instrument"
            ))
        {
            let mut one = value.as_table().ok_or_else(|| format!("calling.toml: {name} is not a table"))?.clone();
            // `like = "other"`: everything `other` states that this does not.
            if let Some(like) = one.remove("like") {
                let like = like.as_str().ok_or_else(|| format!("calling.toml: {name}.like is not a name"))?;
                let base = stated
                    .iter()
                    .find(|(one, _)| one.as_str() == like)
                    .ok_or_else(|| format!("calling.toml: {name} is like {like}, which is not given before it"))?;
                // Each states the cc MIR names it by; it is not inherited.
                for (key, value) in base.1.iter().filter(|(key, _)| key.as_str() != "cc") {
                    one.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            conventions.push(Convention::parse(name, &one)?);
            stated.push((name, one));
        }
        if conventions.is_empty() {
            return Err("calling.toml: no convention".to_owned());
        }
        let private = table
            .get("private")
            .map(|one| one.as_str().map(str::to_owned).ok_or("calling.toml: `private` names a convention"))
            .transpose()?;
        let private_instrument = table
            .get("private_instrument")
            .map(|one| one.as_str().map(str::to_owned).ok_or("calling.toml: `private_instrument` names a convention"))
            .transpose()?;
        let calling = Self { conventions, abis, default, private, abi_private, private_instrument };
        for (key, name) in [("private", &calling.private), ("private_instrument", &calling.private_instrument)] {
            if let Some(name) = name {
                calling
                    .named(name)
                    .ok_or_else(|| format!("calling.toml: {key} is the convention {name}, which is not given"))?;
            }
        }
        for name in calling.abi_private.values() {
            calling
                .named(name)
                .ok_or_else(
                    || format!("calling.toml: an abi's private is the convention {name}, which is not given"),
                )?;
        }
        for (family, convention) in &calling.abis {
            calling
                .named(convention)
                .ok_or_else(
                    || format!("calling.toml: abi.{family} is the convention {convention}, which is not given"),
                )?;
        }
        calling.chosen(None)?;
        Ok(calling)
    }

    /// The convention a private function takes, where the target states one.
    pub fn private(&self) -> Option<&Convention> {
        self.private.as_deref().and_then(|name| self.named(name))
    }

    /// The convention a private function takes under the ABI `family` (the
    /// default's without one): the family's own where it states one, else
    /// `private`'s.
    pub fn private_under(
        &self,
        family: Option<&str>,
    ) -> Option<&Convention> {
        let family = family.unwrap_or(&self.default);
        match self.abi_private.get(family) {
            Some(name) => self.named(name),
            None => self.private(),
        }
    }

    /// The convention the comparison instrument gives a private function.
    pub fn private_instrument(&self) -> Option<&Convention> {
        self.private_instrument.as_deref().and_then(|name| self.named(name))
    }

    /// Whether a function with `convention` may take the private one instead:
    /// the ABIs' own conventions are the ones a program's functions have
    /// unless marked, and a marked one (Pascal's, an interrupt's) is a protocol
    /// its marker names.
    pub fn replaceable(
        &self,
        convention: &Convention,
    ) -> bool {
        self.abis.values().any(|name| *name == convention.name)
    }

    /// The convention an unmarked function has: the default ABI's.
    pub fn native(&self) -> &Convention {
        self.chosen(None).expect("parse checked the default")
    }

    /// The convention an unmarked function has under `-mabi=family`, or the
    /// default's without one; a family this target has not is refused,
    /// saying which it has.
    pub fn chosen(
        &self,
        family: Option<&str>,
    ) -> Result<&Convention, String> {
        let family = family.unwrap_or(&self.default);
        let found = self.abis.get(family).and_then(|name| self.named(name));
        found.ok_or_else(|| {
            format!(
                "no ABI \"{family}\": this target has {}",
                self.abis.keys().map(String::as_str).collect::<Vec<_>>().join(", ")
            )
        })
    }

    /// The names, in file order.
    pub fn names(&self) -> Vec<&str> {
        self.conventions.iter().map(|one| one.name.as_str()).collect()
    }

    /// The convention an interrupt handler is entered under: the one whose
    /// frame the file lays out.
    pub fn interrupt(&self) -> Option<&Convention> {
        self.conventions.iter().find(|one| !one.interrupt_frame.is_empty())
    }

    pub fn named(
        &self,
        name: &str,
    ) -> Option<&Convention> {
        self.conventions.iter().find(|one| one.name == name)
    }

    /// `pattern`, a symbol's OMF decoration (`*_`) as the front end records it,
    /// in `format`: the one the convention whose OMF decoration it is
    /// gives. None where no convention states `format`, or none has that
    /// pattern.
    pub fn redecorated(
        &self,
        pattern: &str,
        format: &str,
    ) -> Option<String> {
        self.conventions
            .iter()
            .find(|one| one.symbol.get("omf").is_some_and(|omf| omf == pattern))
            .and_then(|one| one.symbol.get(format))
            .cloned()
    }

    /// The convention MIR's `cc` names: the file's first answers `ccc`, whose
    /// `cc` is 0.
    pub fn by_cc(
        &self,
        cc: &str,
    ) -> Option<&Convention> {
        self.conventions.iter().find(|one| one.cc.as_deref() == Some(cc))
    }

    /// The convention a MIR calling-convention number names: `ccc` the one
    /// stating `cc = "cdecl"`, `fastcc` the one stating `cc = "fast"` or, where
    /// there is none, C's own; any other the one whose `cc` is the number's
    /// name without its `cc`.
    pub fn by_number(
        &self,
        number: u32,
    ) -> Option<&Convention> {
        match number {
            0 => self.by_cc("cdecl"),
            llrm_mir::opcode::FAST => self.by_cc("fast").or_else(|| self.by_cc("cdecl")),
            // BASIC's own (`pascal far`, QB's): the one stating `cc = "basic"`.
            llrm_mir::opcode::BASIC => self.by_cc("basic"),
            other => self
                .by_cc(llrm_mir::opcode::CONVENTIONS.iter().find(|(_, one)| *one == other)?.0.trim_end_matches("cc")),
        }
    }

    /// MIR's number for the convention a description's `cc` names: C's is
    /// `ccc`, the others `<cc>cc`.
    pub fn number_of(cc: Option<&str>) -> Option<u32> {
        match cc? {
            "cdecl" => Some(0),
            other => llrm_mir::opcode::CONVENTIONS
                .iter()
                .find(|(name, _)| name.strip_suffix("cc") == Some(other))
                .map(|(_, number)| *number),
        }
    }

    /// The convention a function in MIR convention `number` takes when its
    /// callee removes the stack arguments: the one it names (`pops_as`),
    /// else MIR's `fastcc` for C's own.
    pub fn callee_pop(
        &self,
        number: u32,
    ) -> Option<u32> {
        let from = self.by_number(number)?;
        if from.no_pop_twin {
            return None;
        }
        // The convention that removes them itself has none to take: `like`
        // carried `pops_as` to it.
        match &from.pops_as {
            Some(name) => Self::number_of(self.named(name)?.cc.as_deref()).filter(|twin| *twin != number),
            None => (number == 0).then_some(llrm_mir::opcode::FAST),
        }
    }
}

impl Convention {
    /// What `argument` is to this convention's registers, with `slot` bytes in
    /// a stack slot: a word that fits a slot, an integer that fits a
    /// register wider than a slot (a long in a 32-bit register on a 16-bit
    /// target), a pair for an integer or a pointer of two slots (an i64 in
    /// 32 bits, a long or a far pointer in 16), and memory for the rest.
    pub fn kind(
        &self,
        argument: llrm_mir::target::Argument,
        slot: i64,
    ) -> Kind {
        let register = self.register_bytes.unwrap_or(slot);
        match () {
            // A convention with vector argument registers (SysV x86-64) passes
            // a float in one.
            _ if argument.floating && !argument.memory && !self.float_argument_registers.is_empty() => {
                Kind::Float(argument.bytes)
            }
            _ if argument.memory || argument.floating => Kind::Memory(argument.bytes),
            _ if argument.bytes <= slot => Kind::Word,
            _ if argument.bytes <= register && argument.integer => Kind::Sized(argument.bytes),
            _ if argument.bytes == 2 * slot && argument.pointer && !self.pointer_pairs.is_empty() => Kind::Pointer,
            _ if argument.bytes == 2 * slot && (argument.integer || argument.pointer) => Kind::Wide,
            _ => Kind::Memory(argument.bytes),
        }
    }
}

/// What an argument is to a convention's registers: a word of at most a slot,
/// an i64, or what travels in memory whatever it is (a float, a struct by
/// value).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Word,
    /// An integer of this many bytes, more than a slot, that one register
    /// holds: a long in a 32-bit register on a 16-bit target.
    Sized(i64),
    Wide,
    /// A pointer in two registers: `Wide`, from `pointer_pairs` where the
    /// convention names them (`CCIfPtr`).
    Pointer,
    Memory(i64),
    /// A `float` or `double`: the next vector register, else the stack in a
    /// slot of its own.
    Float(i64),
}

/// Where one argument goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Place {
    Registers(Vec<String>),
    /// By its bytes from the first stack argument, which is the lowest.
    Stack(i64),
}

/// Where a call's arguments go and what they take.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    pub places: Vec<Place>,
    /// The registers the arguments use, in the order they were given.
    pub used: Vec<String>,
    /// The bytes the arguments take on the stack.
    pub stack_bytes: i64,
}

impl Convention {
    fn parse(
        name: &str,
        table: &toml::Table,
    ) -> Result<Self, String> {
        let at = |key: &str| format!("calling.toml: {name}.{key}");
        if let Some(unknown) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(format!("{}: no such key", at(unknown)));
        }
        let integer = |key: &str| -> Result<i64, String> {
            table.get(key).and_then(toml::Value::as_integer).ok_or_else(|| format!("{} is not an integer", at(key)))
        };
        let text = |key: &str| -> Result<String, String> {
            table
                .get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("{} is not a string", at(key)))
        };
        let names = |value: Option<&toml::Value>, key: &str| -> Result<Vec<String>, String> {
            value
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("{} is not a list", at(key)))?
                .iter()
                .map(|one| one.as_str().map(str::to_owned).ok_or_else(|| format!("{} holds a non-string", at(key))))
                .collect()
        };
        let order = match text("order")?.as_str() {
            "right-to-left" => Order::RightToLeft,
            "left-to-right" => Order::LeftToRight,
            other => return Err(format!("{} is {other:?}, not right-to-left or left-to-right", at("order"))),
        };
        let cleanup = match text("cleanup")?.as_str() {
            "caller" => Cleanup::Caller,
            "callee" => Cleanup::Callee,
            other => return Err(format!("{} is {other:?}, not caller or callee", at("cleanup"))),
        };
        let preserved = table
            .get("preserved")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{} is not a list", at("preserved")))?
            .iter()
            .map(|one| match one {
                toml::Value::String(register) => Ok(Kept { full: register.clone(), pushed: register.clone() }),
                toml::Value::Array(pair) => match pair.iter().map(toml::Value::as_str).collect::<Vec<_>>()[..] {
                    [Some(full), Some(pushed)] => Ok(Kept { full: full.to_owned(), pushed: pushed.to_owned() }),
                    _ => Err(format!("{}: a pair is [full, pushed]", at("preserved"))),
                },
                _ => Err(format!("{}: a register or a pair", at("preserved"))),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut results = BTreeMap::new();
        for (class, registers) in
            table.get("result").and_then(toml::Value::as_table).ok_or_else(|| format!("{} is missing", at("result")))?
        {
            results.insert(class.clone(), names(Some(registers), "result")?);
        }
        let aggregate = match table.get("aggregate") {
            None => None,
            Some(_) => Some(Aggregate {
                style: text("aggregate")?,
                in_register_bytes: names_as_integers(
                    table.get("aggregate_in_register_bytes"),
                    &at("aggregate_in_register_bytes"),
                )?,
                pointer: text("aggregate_pointer")?,
                pointer_register: table
                    .contains_key("aggregate_pointer_register")
                    .then(|| text("aggregate_pointer_register"))
                    .transpose()?,
                pointer_far: table.get("aggregate_pointer_far").map_or(Ok(false), |one| {
                    one.as_bool().ok_or_else(|| format!("{} is not true or false", at("aggregate_pointer_far")))
                })?,
                pointer_returned: text("aggregate_pointer_returned")?,
                pointer_popped_by: text("aggregate_pointer_popped_by")?,
            }),
        };
        let flag = |key: &str| -> Result<bool, String> {
            table
                .get(key)
                .map_or(Ok(false), |one| one.as_bool().ok_or_else(|| format!("{} is not true or false", at(key))))
        };
        let pairs = |key: &str| -> Result<Vec<[String; 2]>, String> {
            match table.get(key) {
                None => Ok(Vec::new()),
                Some(pairs) => pairs
                    .as_array()
                    .ok_or_else(|| format!("{} is not a list", at(key)))?
                    .iter()
                    .map(|pair| {
                        let names: Vec<Option<&str>> =
                            pair.as_array().map_or(Vec::new(), |pair| pair.iter().map(toml::Value::as_str).collect());
                        match names[..] {
                            [Some(low), Some(high)] => Ok([low.to_owned(), high.to_owned()]),
                            _ => Err(format!("{}: a pair is [low, high]", at(key))),
                        }
                    })
                    .collect::<Result<Vec<_>, String>>(),
            }
        };
        let wide_pairs = pairs("wide_pairs")?;
        let pointer_pairs = pairs("pointer_pairs")?;
        let mut symbol = BTreeMap::new();
        for (format, pattern) in table.get("symbol").map_or(Ok(&toml::Table::new()), |one| {
            one.as_table().ok_or_else(|| format!("{} is not a table", at("symbol")))
        })? {
            let pattern = pattern
                .as_str()
                .filter(|pattern| pattern.contains('*'))
                .ok_or_else(|| format!("{}.{format} is a pattern with a `*` for the name", at("symbol")))?;
            symbol.insert(format.clone(), pattern.to_owned());
        }
        Ok(Self {
            name: name.to_owned(),
            cc: table.contains_key("cc").then(|| text("cc")).transpose()?,
            wide_pairs,
            pointer_pairs,
            backfill: flag("backfill")?,
            positional: flag("positional")?,
            serves: match table.get("serves") {
                Some(_) => names(table.get("serves"), "serves")?,
                None => Vec::new(),
            },
            reserve_call_frame: flag("reserve_call_frame")?,
            shadow_bytes: table.contains_key("shadow_bytes").then(|| integer("shadow_bytes")).transpose()?.unwrap_or(0),
            aligned_calls: flag("aligned_calls")?,
            register_bytes: table.contains_key("register_bytes").then(|| integer("register_bytes")).transpose()?,
            sized_arguments: flag("sized_arguments")?,
            skip_memory: flag("skip_memory")?,
            aggregate_arguments_in_memory: flag("aggregate_arguments_in_memory")?,
            arguments_clobbered: flag("arguments_clobbered")?,
            pinned_copies_unjoined: flag("pinned_copies_unjoined")?,
            results_clobbered: flag("results_clobbered")?,
            entry_clobbers_all: match table.get("entry_clobbers_all") {
                Some(_) => names(table.get("entry_clobbers_all"), "entry_clobbers_all")?,
                None => Vec::new(),
            },
            variadic: table.contains_key("variadic").then(|| text("variadic")).transpose()?,
            pops_as: table.contains_key("pops_as").then(|| text("pops_as")).transpose()?,
            no_pop_twin: flag("no_pop_twin")?,
            symbol,
            slot_bytes: integer("slot_bytes")?,
            stack_alignment: table.contains_key("stack_alignment").then(|| integer("stack_alignment")).transpose()?,
            wide_stack_align: table
                .contains_key("wide_stack_align")
                .then(|| integer("wide_stack_align"))
                .transpose()?,
            order,
            cleanup,
            argument_registers: names(table.get("argument_registers"), "argument_registers")?,
            float_argument_registers: match table.get("float_argument_registers") {
                Some(_) => names(table.get("float_argument_registers"), "float_argument_registers")?,
                None => Vec::new(),
            },
            return_address_bytes: integer("return_address_bytes")?,
            first_argument_offset: integer("first_argument_offset")?,
            first_argument_offset_far: table
                .contains_key("first_argument_offset_far")
                .then(|| integer("first_argument_offset_far"))
                .transpose()?,
            frame: text("frame")?,
            frame_optional: flag("frame_optional")?,
            frame_saved_first: flag("frame_saved_first")?,
            frame_enter: flag("frame_enter")?,
            frame_wrap_split_size: flag("frame_wrap_split_size")?,
            frame_object_aligned: flag("frame_object_aligned")?,
            frame_red_zone: table
                .contains_key("frame_red_zone")
                .then(|| integer("frame_red_zone"))
                .transpose()?
                .unwrap_or(0),
            frame_slot_sibling: flag("frame_slot_sibling")?,
            frame_slot_scratch: table
                .contains_key("frame_slot_push")
                .then(|| Ok::<_, String>((text("frame_slot_push")?, text("frame_slot_pop")?)))
                .transpose()?,
            stack: text("stack")?,
            preserved,
            clobbered: names(table.get("clobbered"), "clobbered")?,
            entry_state: names(table.get("entry_state"), "entry_state")?,
            results,
            aggregate,
            promotion: text("promotion")?,
            wide_slots: integer("wide_slots")?,
            variadic_float: text("variadic_float")?,
            variadic_vector_register: table
                .contains_key("variadic_vector_register")
                .then(|| text("variadic_vector_register"))
                .transpose()?,
            va_list: table.contains_key("va_list").then(|| text("va_list")).transpose()?,
            interrupt_saves: match table.get("interrupt_saves") {
                Some(_) => names(table.get("interrupt_saves"), "interrupt_saves")?,
                None => Vec::new(),
            },
            interrupt_data: match table.get("interrupt_data") {
                Some(_) => names(table.get("interrupt_data"), "interrupt_data")?,
                None => Vec::new(),
            },
            interrupt_parameters: match table.get("interrupt_parameters") {
                Some(_) => names(table.get("interrupt_parameters"), "interrupt_parameters")?,
                None => Vec::new(),
            },
            interrupt_frame: match table.get("interrupt_frame") {
                None => Vec::new(),
                Some(rows) => rows
                    .as_array()
                    .ok_or_else(|| format!("{} is not a list", at("interrupt_frame")))?
                    .iter()
                    .map(|row| {
                        match row.as_array().map(|pair| {
                            (pair.first().and_then(toml::Value::as_str), pair.get(1).and_then(toml::Value::as_integer))
                        }) {
                            Some((Some(register), Some(bytes))) => Ok((register.to_owned(), bytes)),
                            _ => Err(format!("{}: a slot is [register, bytes]", at("interrupt_frame"))),
                        }
                    })
                    .collect::<Result<_, _>>()?,
            },
        })
    }

    /// The registers a result `width` bytes wide leaves in, low part first; the
    /// widest entry that holds it where none is that width.
    pub fn result_registers(
        &self,
        width: i64,
    ) -> Option<&[String]> {
        let by_width =
            self.results.iter().filter_map(|(class, registers)| Some((class.parse::<i64>().ok()?, registers)));
        by_width
            .filter(|(bytes, _)| *bytes >= width)
            .min_by_key(|(bytes, _)| *bytes)
            .map(|(_, registers)| registers.as_slice())
    }

    /// The register a returned pointer (and the `sret` pointer a callee hands
    /// back) leaves in: the `pointer` entry, else the first register of a
    /// result `bytes` wide.
    pub fn pointer_result_register(
        &self,
        bytes: i64,
    ) -> Option<&str> {
        let named = self.results.get("pointer").and_then(|registers| registers.first());
        named.or_else(|| self.result_registers(bytes)?.first()).map(String::as_str)
    }

    /// The vector register a `float` or `double` result leaves in, where the
    /// convention has one.
    pub fn float_result_register(&self) -> Option<&str> {
        self.results.get("float")?.first().map(String::as_str)
    }

    /// Where `arguments` go: each takes the first register free, an i64 the
    /// first pair with both free; the first that fits no register, and all
    /// after it, go on the stack, each at least a slot. Without `backfill`,
    /// a register passed over is not free for a later argument. With
    /// `skip_memory`, only the ones that fit none go on the stack: a later
    /// argument still takes a free register.
    pub fn place(
        &self,
        arguments: &[Kind],
    ) -> Placement {
        self.place_reserving(arguments, &[])
    }

    /// The pairs a word of this kind may take.
    fn pairs_of(
        &self,
        kind: Kind,
    ) -> &[[String; 2]] {
        match kind {
            Kind::Pointer if !self.pointer_pairs.is_empty() => &self.pointer_pairs,
            _ => &self.wide_pairs,
        }
    }

    /// `place` with `reserved` registers already taken: the hidden pointer of a
    /// struct result.
    pub fn place_reserving(
        &self,
        arguments: &[Kind],
        reserved: &[&str],
    ) -> Placement {
        self.place_variadic(arguments, reserved, arguments.len())
    }

    /// `place_reserving` for a variadic call: the arguments from index `fixed`
    /// on are the unnamed ones, which go on the stack after the named ones
    /// even where registers are free, so the callee finds them in one run.
    pub fn place_variadic(
        &self,
        arguments: &[Kind],
        reserved: &[&str],
        fixed: usize,
    ) -> Placement {
        if self.positional {
            return self.place_positional(arguments, reserved);
        }
        let mut free: Vec<bool> = vec![true; self.argument_registers.len()];
        for (index, name) in self.argument_registers.iter().enumerate() {
            free[index] = !reserved.contains(&name.as_str());
        }
        let mut float_free = 0;
        let at = |name: &String| self.argument_registers.iter().position(|one| one == name);
        let mut places = Vec::new();
        let mut used = Vec::new();
        let (mut stack, mut spilled) = (0, false);
        for (index, &kind) in arguments.iter().enumerate() {
            let found = if spilled || index >= fixed {
                None
            } else {
                match kind {
                    Kind::Word | Kind::Sized(_) => free.iter().position(|&one| one).map(|first| vec![first]),
                    Kind::Wide | Kind::Pointer => self.pairs_of(kind).iter().find_map(|pair| {
                        let both = pair.iter().map(at).collect::<Option<Vec<_>>>()?;
                        both.iter().all(|&index| free[index]).then_some(both)
                    }),
                    Kind::Memory(_) => None,
                    Kind::Float(_) => None,
                }
            };
            if let (Kind::Float(_), Some(register), true) = (kind, self.float_argument_registers.get(float_free), index < fixed) {
                float_free += 1;
                used.push(register.clone());
                places.push(Place::Registers(vec![register.clone()]));
                continue;
            }
            match found {
                Some(registers) => {
                    if !self.backfill {
                        let first = *registers.iter().min().expect("a register");
                        free[..first].fill(false);
                    }
                    for &index in &registers {
                        free[index] = false;
                        used.push(self.argument_registers[index].clone());
                    }
                    places.push(Place::Registers(
                        registers.into_iter().map(|index| self.argument_registers[index].clone()).collect(),
                    ));
                }
                None => {
                    spilled = !self.skip_memory;
                    if let (Kind::Wide | Kind::Pointer, Some(align)) = (kind, self.wide_stack_align) {
                        stack = (stack + align - 1) / align * align;
                    }
                    places.push(Place::Stack(stack));
                    let bytes = match kind {
                        Kind::Word => self.slot_bytes,
                        Kind::Sized(bytes) => bytes.max(self.slot_bytes),
                        Kind::Wide | Kind::Pointer => self.slot_bytes * self.wide_slots,
                        Kind::Memory(bytes) | Kind::Float(bytes) => bytes.max(self.slot_bytes),
                    };
                    stack += (bytes + self.slot_bytes - 1) / self.slot_bytes * self.slot_bytes;
                }
            }
        }
        Placement { places, used, stack_bytes: stack }
    }

    /// `place` for a `positional` convention: argument `n` is in register `n`
    /// of its file, else in slot `n` above the shadow area, which the call
    /// reserves even with no argument on the stack.
    fn place_positional(
        &self,
        arguments: &[Kind],
        reserved: &[&str],
    ) -> Placement {
        let mut places = Vec::new();
        let mut used = Vec::new();
        let slot = |position: usize| Place::Stack(self.slot_bytes * position as i64);
        // The hidden result pointer is the first argument: it takes the first
        // position, and the named ones follow.
        let mut position = reserved.len();
        for &kind in arguments {
            // A wide integer is two words (the legalizer splits it): two
            // consecutive positions, both registers or both stack slots. A
            // pair across the fourth position is the caller's to refuse.
            let width = if matches!(kind, Kind::Wide | Kind::Pointer) { 2 } else { 1 };
            let register = match kind {
                Kind::Word | Kind::Sized(_) => self.argument_registers.get(position).map(|name| vec![name.clone()]),
                Kind::Float(_) => self.float_argument_registers.get(position).map(|name| vec![name.clone()]),
                Kind::Wide | Kind::Pointer => (position + 1 < self.argument_registers.len())
                    .then(|| self.argument_registers[position..position + 2].to_vec()),
                Kind::Memory(_) => None,
            };
            places.push(match register {
                Some(names) => {
                    used.extend(names.iter().cloned());
                    Place::Registers(names)
                }
                None => slot(position),
            });
            position += width;
        }
        let stack_bytes = (self.slot_bytes * position as i64).max(self.shadow_bytes);
        Placement { places, used, stack_bytes }
    }

    /// The registers a call changes that the callee does not keep: what the
    /// convention clobbers, the registers it passed arguments in where
    /// those are the callee's, and those it returned a result of `width`
    /// bytes in where those are.
    pub fn clobbers(
        &self,
        used: &[String],
        result_width: Option<i64>,
    ) -> Vec<String> {
        let mut out = self.clobbered.clone();
        if self.arguments_clobbered {
            out.extend(used.iter().cloned());
        }
        if self.results_clobbered {
            out.extend(result_width.and_then(|width| self.result_registers(width)).into_iter().flatten().cloned());
        }
        out.sort();
        out.dedup();
        out
    }

    /// `name` as an object in `format` (`omf`, `elf`, `macho`) spells it under
    /// this convention, where the description gives it.
    pub fn decorated(
        &self,
        format: &str,
        name: &str,
    ) -> Option<String> {
        self.symbol.get(format).map(|pattern| spell(pattern, name))
    }

    /// The registers kept for the caller that a value may be held in: all but
    /// the frame register.
    pub fn callee_saved(&self) -> Vec<&Kept> {
        self.preserved.iter().filter(|one| one.full != self.frame && one.pushed != self.frame).collect()
    }
}

/// `name` in a symbol `pattern`: `*` is the name, `^*` the name in capitals.
pub fn spell(
    pattern: &str,
    name: &str,
) -> String {
    pattern.replace("^*", &name.to_ascii_uppercase()).replace('*', name)
}

fn names_as_integers(
    value: Option<&toml::Value>,
    at: &str,
) -> Result<Vec<i64>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{at} is not a list"))?
        .iter()
        .map(|one| one.as_integer().ok_or_else(|| format!("{at} holds a non-integer")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = "default = \"c\"\n[abi.c]\nconvention = \"c\"\n[c]\nslot_bytes = 2\norder = \"left-to-right\"\ncleanup = \"callee\"\nargument_registers = []\nreturn_address_bytes = 2\nfirst_argument_offset = 4\nfirst_argument_offset_far = 6\nframe = \"bp\"\nstack = \"sp\"\npreserved = [\"bp\", [\"esi\", \"si\"]]\nclobbered = [\"ax\"]\nentry_state = []\npromotion = \"slot\"\nwide_slots = 2\nvariadic_float = \"double\"\n[c.result]\n1 = [\"eax\"]\n4 = [\"eax\", \"edx\"]\n";

    #[test]
    fn a_convention_names_its_frame_its_kept_registers_and_its_results() {
        let calling = Calling::parse(ONE).unwrap();
        let one = calling.native();
        assert_eq!(
            (one.slot_bytes, one.order, one.cleanup, one.first_argument_offset, one.first_argument_offset_far),
            (2, Order::LeftToRight, Cleanup::Callee, 4, Some(6))
        );
        // The frame register is kept, but no value is held in it.
        assert_eq!(one.preserved.len(), 2);
        assert_eq!(one.callee_saved(), [&Kept { full: "esi".into(), pushed: "si".into() }]);
        // A width no entry names takes the narrowest that holds it.
        assert_eq!(
            (one.result_registers(2).unwrap(), one.result_registers(4).unwrap(), one.result_registers(8)),
            (&["eax".to_owned(), "edx".to_owned()][..], &["eax".to_owned(), "edx".to_owned()][..], None)
        );
        assert_eq!(one.result_registers(1).unwrap(), ["eax".to_owned()]);
    }

    /// A key the schema does not read is a typo or a fact nobody reads:
    /// refused.
    #[test]
    fn a_convention_is_like_an_earlier_one_but_for_what_it_states() {
        let text = format!("{ONE}[d]\nlike = \"c\"\ncleanup = \"caller\"\n[d.result]\n1 = [\"ax\"]\n");
        let calling = Calling::parse(&text).unwrap();
        let d = calling.named("d").unwrap();
        assert_eq!((d.cleanup, d.slot_bytes, d.order), (Cleanup::Caller, 2, Order::LeftToRight));
        assert_eq!(d.results.len(), 1);
        assert!(
            Calling::parse(&text.replace("like = \"c\"", "like = \"z\"")).unwrap_err().contains("not given before it")
        );
    }

    #[test]
    fn a_key_nobody_reads_is_refused() {
        let error = Calling::parse(&ONE.replace("wide_slots = 2", "wide_slots = 2\nred_zone = 128")).unwrap_err();
        assert_eq!(error, "calling.toml: c.red_zone: no such key");
        assert!(
            Calling::parse(&ONE.replace("left-to-right", "sideways"))
                .unwrap_err()
                .contains("not right-to-left or left-to-right")
        );
    }

    /// Open Watcom's flat register convention, as `wcc386 -3r` emits it (read
    /// from its disassembly).
    const WATCALL: &str = "default = \"w\"\n[abi.w]\nconvention = \"w\"\n[w]\nslot_bytes = 4\norder = \"right-to-left\"\ncleanup = \"callee\"\nargument_registers = [\"eax\", \"edx\", \"ebx\", \"ecx\"]\nwide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\nbackfill = true\narguments_clobbered = true\nresults_clobbered = true\nreturn_address_bytes = 4\nfirst_argument_offset = 8\nframe = \"ebp\"\nstack = \"esp\"\npreserved = [\"ebx\", \"ecx\", \"edx\", \"esi\", \"edi\", \"ebp\"]\nclobbered = [\"eax\", \"flags\"]\nentry_state = []\npromotion = \"slot\"\nwide_slots = 2\nvariadic_float = \"double\"\n[w.result]\n4 = [\"eax\"]\n8 = [\"eax\", \"edx\"]\n";

    fn registers(names: &[&str]) -> Place {
        Place::Registers(names.iter().map(|one| (*one).to_owned()).collect())
    }

    /// `wcc386` put `(int a, i64 b, int c)` in EAX, EBX:ECX and EDX: the pair
    /// EDX:EBX does not exist, and EDX stayed free for `c`. A rule that
    /// took the next two registers in a row gave b EDX:EBX.
    #[test]
    fn an_i64_takes_a_fixed_pair_and_a_register_passed_over_stays_free() {
        let calling = Calling::parse(WATCALL).unwrap();
        let w = calling.native();
        let placed = w.place(&[Kind::Word, Kind::Wide, Kind::Word]);
        assert_eq!(placed.places, [registers(&["eax"]), registers(&["ebx", "ecx"]), registers(&["edx"])]);
        assert_eq!(placed.stack_bytes, 0);
        // `(i64, i64)`: EAX:EDX, then EBX:ECX.
        assert_eq!(w.place(&[Kind::Wide, Kind::Wide]).places, [registers(&["eax", "edx"]), registers(&["ebx", "ecx"])]);
    }

    /// `wcc386` sent `(int, double, int)` to EAX and two stack cells: the first
    /// argument that fits no register sends every later one to the stack,
    /// though EDX was free. Also `(int, int, int, i64)`: only ECX was left,
    /// so the i64 went to the stack.
    #[test]
    fn the_first_argument_that_fits_no_register_sends_the_rest_to_the_stack() {
        let calling = Calling::parse(WATCALL).unwrap();
        let w = calling.native();
        let placed = w.place(&[Kind::Word, Kind::Memory(8), Kind::Word]);
        assert_eq!(placed.places, [registers(&["eax"]), Place::Stack(0), Place::Stack(8)]);
        assert_eq!(placed.stack_bytes, 12);
        let placed = w.place(&[Kind::Word, Kind::Word, Kind::Word, Kind::Wide]);
        assert_eq!((placed.places[3].clone(), placed.stack_bytes), (Place::Stack(0), 8));
        // Six words: four in registers, the fifth lowest on the stack.
        let six = w.place(&[Kind::Word; 6]);
        assert_eq!(
            (six.places[4].clone(), six.places[5].clone(), six.stack_bytes),
            (Place::Stack(0), Place::Stack(4), 8)
        );
    }

    /// The unnamed arguments of a variadic call went to the registers the named ones left free, where a callee that
    /// walks the stack for them found nothing. They go on the stack after the named ones: `(int, int)` with one
    /// named is EAX and then stack offset 0, though EDX was free.
    #[test]
    fn the_unnamed_arguments_go_on_the_stack_after_the_named_ones() {
        let calling = Calling::parse(WATCALL).unwrap();
        let w = calling.native();
        let placed = w.place_variadic(&[Kind::Word, Kind::Word, Kind::Wide], &[], 1);
        assert_eq!(placed.places, [registers(&["eax"]), Place::Stack(0), Place::Stack(4)]);
        assert_eq!(placed.stack_bytes, 12);
        assert_eq!(w.place_variadic(&[Kind::Word, Kind::Word], &[], 2).places, w.place(&[Kind::Word, Kind::Word]).places);
    }

    const WIN64: &str = "default = \"w\"\n[abi.w]\nconvention = \"w\"\n[w]\nslot_bytes = 8\norder = \"right-to-left\"\ncleanup = \"caller\"\nargument_registers = [\"rcx\", \"rdx\", \"r8\", \"r9\"]\nfloat_argument_registers = [\"xmm0\", \"xmm1\", \"xmm2\", \"xmm3\"]\npositional = true\nshadow_bytes = 32\nreturn_address_bytes = 8\nfirst_argument_offset = 48\nframe = \"rbp\"\nstack = \"rsp\"\npreserved = [[\"ebx\", \"rbx\"]]\nclobbered = [\"rax\"]\nentry_state = []\npromotion = \"slot\"\nwide_slots = 1\nvariadic_float = \"double\"\n[w.result]\n8 = [\"rax\"]\n";

    /// Win64 shares positions between the integer and float registers: the
    /// second argument is RDX or XMM1 whatever the first was, the fifth is the
    /// stack slot above the 32-byte home area, and a call with no stack
    /// argument still reserves it. SysV's counters placed `(i, d, i, d)` in
    /// RCX, XMM0, RDX, XMM1.
    #[test]
    fn a_positional_convention_numbers_its_registers_by_argument_not_by_file() {
        let calling = Calling::parse(WIN64).unwrap();
        let placed = calling.native().place(&[Kind::Word, Kind::Float(8), Kind::Word, Kind::Float(8), Kind::Word]);
        assert_eq!(
            placed.places,
            [registers(&["rcx"]), registers(&["xmm1"]), registers(&["r8"]), registers(&["xmm3"]), Place::Stack(32)]
        );
        assert_eq!(placed.stack_bytes, 40);
        assert_eq!(calling.native().place(&[]).stack_bytes, 32);
    }

    /// A struct result's hidden pointer is RCX under Win64, so the named
    /// arguments are positions 1, 2, ...: RDX, XMM2, and the fourth is above
    /// the shadow area. Placed from position 0 the first one lost its register.
    #[test]
    fn a_positional_convention_counts_the_hidden_result_pointer_as_position_zero() {
        let calling = Calling::parse(WIN64).unwrap();
        let placed = calling.native().place_reserving(&[Kind::Word, Kind::Float(8), Kind::Word, Kind::Word], &["rcx"]);
        assert_eq!(placed.places, [registers(&["rdx"]), registers(&["xmm2"]), registers(&["r9"]), Place::Stack(32)]);
        assert_eq!(placed.stack_bytes, 40);
    }

    /// gcc's regparm(3) puts `(float, int b, double, int d, int e)` in EAX,
    /// EDX, ECX (b, d, e) and the float and the double on the stack: it
    /// counts integers only. With `skip_memory` off the float stopped the rest,
    /// as Open Watcom's does (b, d, e to the stack).
    #[test]
    fn an_argument_in_memory_leaves_the_registers_to_the_later_ones_where_the_convention_says_so() {
        let regparm = WATCALL
            .replace("wide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\n", "")
            .replace("\"ebx\", ", "")
            .replace("backfill = true\n", "skip_memory = true\n");
        let calling = Calling::parse(&regparm).unwrap();
        let placed =
            calling.native().place(&[Kind::Memory(4), Kind::Word, Kind::Memory(8), Kind::Word, Kind::Word, Kind::Word]);
        assert_eq!(
            placed.places,
            [
                Place::Stack(0),
                registers(&["eax"]),
                Place::Stack(4),
                registers(&["edx"]),
                registers(&["ecx"]),
                Place::Stack(12)
            ]
        );
        let watcom = Calling::parse(WATCALL).unwrap();
        let placed = watcom.native().place(&[Kind::Memory(4), Kind::Word]);
        assert_eq!(placed.places, [Place::Stack(0), Place::Stack(4)]);
    }

    /// A long that reaches the stack takes its own two words: `(int, int, int,
    /// long)` has the long at the stack's start and `(long, long, long,
    /// long, long)` the fifth four bytes past the fourth. A word's worth put
    /// the next one in its middle.
    #[test]
    fn a_sized_argument_takes_its_bytes_on_the_stack() {
        let regparm = WATCALL
            .replace("wide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\n", "")
            .replace("\"ebx\", ", "")
            .replace("slot_bytes = 4", "slot_bytes = 2");
        let calling = Calling::parse(&regparm).unwrap();
        let placed = calling.native().place(&[Kind::Sized(4); 6]);
        assert_eq!(placed.places[3..], [Place::Stack(0), Place::Stack(4), Place::Stack(8)]);
        assert_eq!(placed.stack_bytes, 12);
    }

    /// A function whose callers remove its stack arguments takes, when the
    /// callee is to, the convention its description names (`pops_as`), or
    /// MIR's `fastcc` where it is C's own and names none.
    #[test]
    fn a_convention_names_the_one_it_takes_when_its_callee_pops() {
        let text = format!(
            "{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\ncleanup = \"caller\"\n[r]\nlike = \"w\"\ncc = \"regparm3\"\ncleanup = \"caller\"\npops_as = \"rp\"\n[rp]\nlike = \"r\"\ncc = \"regparm3pop\"\ncleanup = \"callee\"\n"
        );
        let calling = Calling::parse(&text).unwrap();
        assert_eq!(calling.callee_pop(0), Some(llrm_mir::opcode::FAST));
        assert_eq!(calling.callee_pop(llrm_mir::opcode::REGPARM3), Some(llrm_mir::opcode::REGPARM3POP));
        assert_eq!(calling.callee_pop(llrm_mir::opcode::REGPARM3POP), None);
        assert_eq!(calling.callee_pop(llrm_mir::opcode::WATCALL), None);
    }

    /// A call changes EAX, the registers its arguments went in and EDX for an
    /// i64 result, and keeps the rest: `wcc386` held a value in ECX across
    /// `h2(i64, int)` and one in EDX across `h1(int)`.
    #[test]
    fn a_call_changes_the_registers_of_its_arguments_and_results_only() {
        let calling = Calling::parse(WATCALL).unwrap();
        let w = calling.native();
        let used = w.place(&[Kind::Word]).used;
        assert_eq!(w.clobbers(&used, Some(4)), ["eax", "flags"]);
        let used = w.place(&[Kind::Wide, Kind::Word]).used;
        assert_eq!(w.clobbers(&used, Some(8)), ["eax", "ebx", "edx", "flags"]);
    }

    /// An ELF object has no leading underscore and an OMF one a trailing one:
    /// the description says so for each convention, and a pattern the front
    /// end recorded (OMF's) is turned into the format's.
    #[test]
    fn a_symbol_is_decorated_as_its_object_format_spells_it() {
        let text = format!(
            "{WATCALL}[w.symbol]\nomf = \"*_\"\nelf = \"*\"\nmacho = \"_*\"\n[c]\nlike = \"w\"\ncc = \"cdecl\"\n[c.symbol]\nomf = \"_*\"\nelf = \"*\"\nmacho = \"_*\"\n"
        );
        let calling = Calling::parse(&text).unwrap();
        assert_eq!(calling.named("w").unwrap().decorated("omf", "f").as_deref(), Some("f_"));
        assert_eq!(calling.named("c").unwrap().decorated("macho", "f").as_deref(), Some("_f"));
        assert_eq!(
            (
                calling.redecorated("*_", "elf").as_deref(),
                calling.redecorated("_*", "elf").as_deref(),
                calling.redecorated("_*", "macho").as_deref()
            ),
            (Some("*"), Some("*"), Some("_*"))
        );
        assert_eq!((calling.redecorated("^", "elf"), calling.redecorated("*_", "coff")), (None, None));
    }

    /// An ABI family may state its own private convention (`[abi.<family>]
    /// private`), and a convention the conventions it serves (`serves`); the
    /// others take the file's `private`. Win64's `fastcc` function went to
    /// System V's registers for want of both.
    #[test]
    fn an_abi_family_states_its_own_private_convention_and_the_ones_it_serves() {
        let text = format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\nserves = [\"fast\"]\n").replace(
            "default = \"w\"\n[abi.w]\nconvention = \"w\"\n",
            "default = \"stack\"\nprivate = \"w\"\n[abi.reg]\nconvention = \"w\"\nprivate = \"c\"\n[abi.stack]\nconvention = \"c\"\n",
        );
        let calling = Calling::parse(&text).unwrap();
        let named = |one: Option<&Convention>| one.map(|one| one.name.clone());
        assert_eq!(named(calling.private_under(None)), Some("w".to_owned()));
        assert_eq!(named(calling.private_under(Some("reg"))), Some("c".to_owned()));
        assert_eq!(calling.named("c").unwrap().serves, ["fast"]);
        assert!(calling.named("w").unwrap().serves.is_empty());
    }

    /// The default ABI is the file's `default`, not its first entry: a second
    /// family is chosen by name, and one the target has not is refused with
    /// the ones it has.
    #[test]
    fn an_abi_family_names_the_convention_an_unmarked_function_has() {
        let text = format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\n").replace(
            "default = \"w\"\n[abi.w]\nconvention = \"w\"\n",
            "default = \"stack\"\n[abi.reg]\nconvention = \"w\"\n[abi.stack]\nconvention = \"c\"\n",
        );
        let calling = Calling::parse(&text).unwrap();
        assert_eq!((calling.native().name.as_str(), calling.chosen(Some("reg")).unwrap().name.as_str()), ("c", "w"));
        assert_eq!(calling.chosen(Some("gcc")).unwrap_err(), "no ABI \"gcc\": this target has reg, stack");
        assert!(
            Calling::parse(&text.replace("default = \"stack\"", "default = \"nowhere\""))
                .unwrap_err()
                .contains("no ABI \"nowhere\"")
        );
    }

    /// A private function takes the convention `private` names under any ABI;
    /// only the ABIs' own conventions may be replaced, and a name the file
    /// does not give is refused.
    #[test]
    fn a_private_function_has_the_convention_the_description_names() {
        let stack = format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\n[p]\nlike = \"c\"\ncc = \"pascal\"\n").replace("default = \"w\"\n[abi.w]\nconvention = \"w\"\n", "default = \"stack\"\nprivate = \"w\"\n[abi.reg]\nconvention = \"w\"\n[abi.stack]\nconvention = \"c\"\n");
        let calling = Calling::parse(&stack).unwrap();
        assert_eq!(calling.private().map(|one| one.name.as_str()), Some("w"));
        let named = |name: &str| calling.named(name).unwrap();
        assert!(calling.replaceable(named("c")) && calling.replaceable(named("w")) && !calling.replaceable(named("p")));
        assert!(
            Calling::parse(&stack.replace("private = \"w\"", "private = \"nowhere\""))
                .unwrap_err()
                .contains("private is the convention nowhere")
        );
        assert!(Calling::parse(&stack.replace("private = \"w\"\n", "")).unwrap().private().is_none());
    }

    #[test]
    fn a_convention_is_found_by_the_cc_mir_names_it_by() {
        let calling = Calling::parse(&format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\n")).unwrap();
        assert_eq!(calling.by_cc("cdecl").map(|one| one.name.as_str()), Some("c"));
        assert!(calling.by_cc("pascal").is_none());
    }
}

#[cfg(test)]
mod interrupt_frame_tests {
    use super::*;

    /// The interrupt frame was a constant in llrm-mir beside the description; a
    /// convention states it now. A slot that is not `[register, bytes]` was
    /// nobody's error before and is refused.
    #[test]
    fn test_an_interrupt_frame_is_read_from_the_description() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../llrm-x86-m16/src/machines/calling.toml"))
                .unwrap();
        let calling = Calling::parse(&text).unwrap();
        let frame = &calling.interrupt().expect("m16 has an interrupt handler").interrupt_frame;
        assert_eq!(frame.len(), 15);
        assert_eq!(frame[0], ("gs".to_owned(), 2));
        assert_eq!(frame.iter().map(|slot| slot.1).sum::<i64>(), 46);
        let bad = text.replace("[\"gs\", 2]", "[\"gs\"]");
        assert!(Calling::parse(&bad).is_err());
    }

    /// Borland's `interrupt` register parameters were the frame's words in
    /// address order; the description names each one's slot, and every name is a
    /// slot of the frame.
    #[test]
    fn test_interrupt_parameters_name_slots_of_the_frame() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../llrm-x86-m16/src/machines/calling.toml"))
                .unwrap();
        let handler = Calling::parse(&text).unwrap();
        let handler = handler.interrupt().expect("m16 has an interrupt handler");
        assert_eq!(handler.interrupt_parameters[..3], ["ebp", "edi", "esi"]);
        for name in &handler.interrupt_parameters {
            assert!(handler.interrupt_frame.iter().any(|slot| &slot.0 == name), "{name}");
        }
    }
}
