//! A target's calling conventions, from its `calling.toml`: one table a convention, in the order
//! the file gives them, the first being the one a language's own functions use. Registers are
//! named as the target's `registers.regs` names them; the schema takes no key it does not read.

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

/// A register kept for the caller: its full register, and the one a prologue pushes.
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
    /// `after-arguments`: pushed past the last argument; `register`: held in `pointer_register`.
    pub pointer: String,
    pub pointer_register: Option<String>,
    /// The address is a far pointer where the target has far data (Borland's); else a near one.
    pub pointer_far: bool,
    pub pointer_returned: String,
    pub pointer_popped_by: String,
}

/// One calling convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Convention {
    pub name: String,
    /// The calling convention MIR names this one by (`cdecl`, `watcall`); the file's first answers
    /// MIR's `ccc` as well.
    pub cc: Option<String>,
    /// What a byte argument takes on the stack, and the least a cell holds.
    pub slot_bytes: i64,
    pub order: Order,
    pub cleanup: Cleanup,
    pub argument_registers: Vec<String>,
    /// The pairs an i64 argument takes, low register first.
    pub wide_pairs: Vec<[String; 2]>,
    /// Whether a register an argument skipped is free for a later one.
    pub backfill: bool,
    /// The most bytes of an integer one argument register holds, where that is more than a stack slot (a long in a 32-bit
    /// register on a 16-bit target); a slot's worth where the file gives none.
    pub register_bytes: Option<i64>,
    /// A register takes the width of the value it holds (AL, AX or EAX for 1, 2 or 4 bytes) rather than its own: the argument
    /// registers are named by their widest part.
    pub sized_arguments: bool,
    /// An argument that travels in memory (a float, an i64, a struct) leaves the registers free for the arguments after it.
    pub skip_memory: bool,
    /// A struct passed by value travels in memory whatever its size; else a struct of a size that returns in a register is passed
    /// as that integer.
    pub aggregate_arguments_in_memory: bool,
    /// What a call's arguments in registers are the callee's to change; everything else it keeps.
    pub arguments_clobbered: bool,
    /// The same for the registers a result leaves in.
    pub results_clobbered: bool,
    /// The convention a variadic call uses in place of this one: it moves the arguments, not the symbol.
    pub variadic: Option<String>,
    /// How a symbol is written in each object format, `*` standing for its name and `^*` for its name in capitals (`spell`).
    pub symbol: BTreeMap<String, String>,
    pub return_address_bytes: i64,
    /// Where the first argument lies from the frame register, past the saved frame register and
    /// the return address.
    pub first_argument_offset: i64,
    /// The same when the call is far, where the convention has far calls.
    pub first_argument_offset_far: Option<i64>,
    /// The register a frame's cells are addressed through, and the stack pointer.
    pub frame: String,
    /// A function that needs no frame register may leave it out: its cells are addressed through the
    /// stack pointer, and the frame register is not set. gcc's `-fomit-frame-pointer`.
    pub frame_optional: bool,
    /// A frame tuned for size is opened with `enter N,0` (4 bytes against 6). Left false where the target prices it
    /// above `push bp; mov bp,sp; sub sp,N`: the 486 takes 14 clocks against 3 (Intel 240440-002), and neither GCC nor LLVM emits it.
    pub frame_enter: bool,
    pub stack: String,
    /// The registers a callee keeps, the frame register among them.
    pub preserved: Vec<Kept>,
    pub clobbered: Vec<String>,
    /// What holds on entry and on return: `df_clear`, `x87_empty`.
    pub entry_state: Vec<String>,
    /// The registers a result leaves in, by its width in bytes, or by a class (`pointer`, `float`).
    pub results: BTreeMap<String, Vec<String>>,
    pub aggregate: Option<Aggregate>,
    pub promotion: String,
    /// How many slots an i64 or a double takes.
    pub wide_slots: i64,
    pub variadic_float: String,
    /// For an interrupt handler: what its frame pointer addresses, lowest address first, each slot's register and
    /// bytes. Empty for the rest.
    pub interrupt_frame: Vec<(String, i64)>,
}

/// A target's conventions, in file order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Calling {
    pub conventions: Vec<Convention>,
    /// The ABI families a program may ask for with `-mabi=`: each names the convention an unmarked function has under it.
    pub abis: BTreeMap<String, String>,
    /// The family a program gets without the switch.
    pub default: String,
    /// The convention a function nothing outside the program reaches takes, whatever ABI the program has: no one else sees it,
    /// so the target's best serves. None where its default is that.
    pub private: Option<String>,
}

const KEYS: &[&str] = &[
    "cc",
    "wide_pairs",
    "backfill",
    "register_bytes",
    "sized_arguments",
    "skip_memory",
    "aggregate_arguments_in_memory",
    "arguments_clobbered",
    "results_clobbered",
    "variadic",
    "symbol",
    "aggregate_pointer_register",
    "aggregate_pointer_far",
    "slot_bytes",
    "order",
    "cleanup",
    "argument_registers",
    "return_address_bytes",
    "first_argument_offset",
    "first_argument_offset_far",
    "frame",
    "frame_optional",
    "frame_enter",
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
    "like",
    "interrupt_frame",
];

impl Calling {
    /// `text`, a `calling.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let mut conventions = Vec::new();
        let mut stated: Vec<(&String, toml::Table)> = Vec::new();
        let default = table.get("default").and_then(toml::Value::as_str).ok_or("calling.toml: `default` names the ABI a program has without -mabi=")?.to_owned();
        let mut abis = BTreeMap::new();
        for (family, one) in table.get("abi").and_then(toml::Value::as_table).ok_or("calling.toml: no [abi.<family>]")? {
            let convention = one.as_table().and_then(|one| one.get("convention")).and_then(toml::Value::as_str).ok_or_else(|| format!("calling.toml: abi.{family}.convention is not a name"))?;
            abis.insert(family.clone(), convention.to_owned());
        }
        for (name, value) in table.iter().filter(|(name, _)| !matches!(name.as_str(), "default" | "abi" | "private")) {
            let mut one = value.as_table().ok_or_else(|| format!("calling.toml: {name} is not a table"))?.clone();
            // `like = "other"`: everything `other` states that this does not.
            if let Some(like) = one.remove("like") {
                let like = like.as_str().ok_or_else(|| format!("calling.toml: {name}.like is not a name"))?;
                let base = stated.iter().find(|(one, _)| one.as_str() == like).ok_or_else(|| format!("calling.toml: {name} is like {like}, which is not given before it"))?;
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
        let private = table.get("private").map(|one| one.as_str().map(str::to_owned).ok_or("calling.toml: `private` names a convention")).transpose()?;
        let calling = Self { conventions, abis, default, private };
        if let Some(name) = &calling.private {
            calling.named(name).ok_or_else(|| format!("calling.toml: private is the convention {name}, which is not given"))?;
        }
        for (family, convention) in &calling.abis {
            calling.named(convention).ok_or_else(|| format!("calling.toml: abi.{family} is the convention {convention}, which is not given"))?;
        }
        calling.chosen(None)?;
        Ok(calling)
    }

    /// The convention a private function takes, where the target states one.
    pub fn private(&self) -> Option<&Convention> {
        self.private.as_deref().and_then(|name| self.named(name))
    }

    /// Whether a function with `convention` may take the private one instead: the ABIs' own conventions are the ones a program's
    /// functions have unless marked, and a marked one (Pascal's, an interrupt's) is a protocol its marker names.
    pub fn replaceable(&self, convention: &Convention) -> bool {
        self.abis.values().any(|name| *name == convention.name)
    }

    /// The convention an unmarked function has: the default ABI's.
    pub fn native(&self) -> &Convention {
        self.chosen(None).expect("parse checked the default")
    }

    /// The convention an unmarked function has under `-mabi=family`, or the default's without one; a family this target
    /// has not is refused, saying which it has.
    pub fn chosen(&self, family: Option<&str>) -> Result<&Convention, String> {
        let family = family.unwrap_or(&self.default);
        let found = self.abis.get(family).and_then(|name| self.named(name));
        found.ok_or_else(|| format!("no ABI \"{family}\": this target has {}", self.abis.keys().map(String::as_str).collect::<Vec<_>>().join(", ")))
    }

    /// The names, in file order.
    pub fn names(&self) -> Vec<&str> {
        self.conventions.iter().map(|one| one.name.as_str()).collect()
    }

    /// The convention an interrupt handler is entered under: the one whose frame the file lays out.
    pub fn interrupt(&self) -> Option<&Convention> {
        self.conventions.iter().find(|one| !one.interrupt_frame.is_empty())
    }

    pub fn named(&self, name: &str) -> Option<&Convention> {
        self.conventions.iter().find(|one| one.name == name)
    }

    /// `pattern`, a symbol's OMF decoration (`*_`) as the front end records it, in `format`: the one the convention
    /// whose OMF decoration it is gives. None where no convention states `format`, or none has that pattern.
    pub fn redecorated(&self, pattern: &str, format: &str) -> Option<String> {
        self.conventions.iter().find(|one| one.symbol.get("omf").is_some_and(|omf| omf == pattern)).and_then(|one| one.symbol.get(format)).cloned()
    }

    /// The convention MIR's `cc` names: the file's first answers `ccc`, whose `cc` is 0.
    pub fn by_cc(&self, cc: &str) -> Option<&Convention> {
        self.conventions.iter().find(|one| one.cc.as_deref() == Some(cc))
    }
}

/// What an argument is to a convention's registers: a word of at most a slot, an i64, or what
/// travels in memory whatever it is (a float, a struct by value).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Word,
    /// An integer of this many bytes, more than a slot, that one register holds: a long in a 32-bit register on a 16-bit target.
    Sized(i64),
    Wide,
    Memory(i64),
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
    fn parse(name: &str, table: &toml::Table) -> Result<Self, String> {
        let at = |key: &str| format!("calling.toml: {name}.{key}");
        if let Some(unknown) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(format!("{}: no such key", at(unknown)));
        }
        let integer = |key: &str| -> Result<i64, String> { table.get(key).and_then(toml::Value::as_integer).ok_or_else(|| format!("{} is not an integer", at(key))) };
        let text = |key: &str| -> Result<String, String> { table.get(key).and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("{} is not a string", at(key))) };
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
        for (class, registers) in table.get("result").and_then(toml::Value::as_table).ok_or_else(|| format!("{} is missing", at("result")))? {
            results.insert(class.clone(), names(Some(registers), "result")?);
        }
        let aggregate = match table.get("aggregate") {
            None => None,
            Some(_) => Some(Aggregate {
                style: text("aggregate")?,
                in_register_bytes: names_as_integers(table.get("aggregate_in_register_bytes"), &at("aggregate_in_register_bytes"))?,
                pointer: text("aggregate_pointer")?,
                pointer_register: table.contains_key("aggregate_pointer_register").then(|| text("aggregate_pointer_register")).transpose()?,
                pointer_far: table.get("aggregate_pointer_far").map_or(Ok(false), |one| one.as_bool().ok_or_else(|| format!("{} is not true or false", at("aggregate_pointer_far"))))?,
                pointer_returned: text("aggregate_pointer_returned")?,
                pointer_popped_by: text("aggregate_pointer_popped_by")?,
            }),
        };
        let flag = |key: &str| -> Result<bool, String> { table.get(key).map_or(Ok(false), |one| one.as_bool().ok_or_else(|| format!("{} is not true or false", at(key)))) };
        let wide_pairs = match table.get("wide_pairs") {
            None => Vec::new(),
            Some(pairs) => pairs
                .as_array()
                .ok_or_else(|| format!("{} is not a list", at("wide_pairs")))?
                .iter()
                .map(|pair| {
                    let names: Vec<Option<&str>> = pair.as_array().map_or(Vec::new(), |pair| pair.iter().map(toml::Value::as_str).collect());
                    match names[..] {
                        [Some(low), Some(high)] => Ok([low.to_owned(), high.to_owned()]),
                        _ => Err(format!("{}: a pair is [low, high]", at("wide_pairs"))),
                    }
                })
                .collect::<Result<Vec<_>, String>>()?,
        };
        let mut symbol = BTreeMap::new();
        for (format, pattern) in table.get("symbol").map_or(Ok(&toml::Table::new()), |one| one.as_table().ok_or_else(|| format!("{} is not a table", at("symbol"))))? {
            let pattern = pattern.as_str().filter(|pattern| pattern.contains('*')).ok_or_else(|| format!("{}.{format} is a pattern with a `*` for the name", at("symbol")))?;
            symbol.insert(format.clone(), pattern.to_owned());
        }
        Ok(Self {
            name: name.to_owned(),
            cc: table.contains_key("cc").then(|| text("cc")).transpose()?,
            wide_pairs,
            backfill: flag("backfill")?,
            register_bytes: table.contains_key("register_bytes").then(|| integer("register_bytes")).transpose()?,
            sized_arguments: flag("sized_arguments")?,
            skip_memory: flag("skip_memory")?,
            aggregate_arguments_in_memory: flag("aggregate_arguments_in_memory")?,
            arguments_clobbered: flag("arguments_clobbered")?,
            results_clobbered: flag("results_clobbered")?,
            variadic: table.contains_key("variadic").then(|| text("variadic")).transpose()?,
            symbol,
            slot_bytes: integer("slot_bytes")?,
            order,
            cleanup,
            argument_registers: names(table.get("argument_registers"), "argument_registers")?,
            return_address_bytes: integer("return_address_bytes")?,
            first_argument_offset: integer("first_argument_offset")?,
            first_argument_offset_far: table.contains_key("first_argument_offset_far").then(|| integer("first_argument_offset_far")).transpose()?,
            frame: text("frame")?,
            frame_optional: flag("frame_optional")?,
            frame_enter: flag("frame_enter")?,
            stack: text("stack")?,
            preserved,
            clobbered: names(table.get("clobbered"), "clobbered")?,
            entry_state: names(table.get("entry_state"), "entry_state")?,
            results,
            aggregate,
            promotion: text("promotion")?,
            wide_slots: integer("wide_slots")?,
            variadic_float: text("variadic_float")?,
            interrupt_frame: match table.get("interrupt_frame") {
                None => Vec::new(),
                Some(rows) => rows
                    .as_array()
                    .ok_or_else(|| format!("{} is not a list", at("interrupt_frame")))?
                    .iter()
                    .map(|row| match row.as_array().map(|pair| (pair.first().and_then(toml::Value::as_str), pair.get(1).and_then(toml::Value::as_integer))) {
                        Some((Some(register), Some(bytes))) => Ok((register.to_owned(), bytes)),
                        _ => Err(format!("{}: a slot is [register, bytes]", at("interrupt_frame"))),
                    })
                    .collect::<Result<_, _>>()?,
            },
        })
    }

    /// The registers a result `width` bytes wide leaves in, low part first; the widest entry that
    /// holds it where none is that width.
    pub fn result_registers(&self, width: i64) -> Option<&[String]> {
        let by_width = self.results.iter().filter_map(|(class, registers)| Some((class.parse::<i64>().ok()?, registers)));
        by_width.filter(|(bytes, _)| *bytes >= width).min_by_key(|(bytes, _)| *bytes).map(|(_, registers)| registers.as_slice())
    }

    /// Where `arguments` go: each takes the first register free, an i64 the first pair with both
    /// free; the first that fits no register, and all after it, go on the stack, each at least a
    /// slot. Without `backfill`, a register passed over is not free for a later argument. With `skip_memory`, only
    /// the ones that fit none go on the stack: a later argument still takes a free register.
    pub fn place(&self, arguments: &[Kind]) -> Placement {
        let mut free: Vec<bool> = vec![true; self.argument_registers.len()];
        let at = |name: &String| self.argument_registers.iter().position(|one| one == name);
        let mut places = Vec::new();
        let mut used = Vec::new();
        let (mut stack, mut spilled) = (0, false);
        for &kind in arguments {
            let found = if spilled {
                None
            } else {
                match kind {
                    Kind::Word | Kind::Sized(_) => free.iter().position(|&one| one).map(|first| vec![first]),
                    Kind::Wide => self.wide_pairs.iter().find_map(|pair| {
                        let both = pair.iter().map(at).collect::<Option<Vec<_>>>()?;
                        both.iter().all(|&index| free[index]).then_some(both)
                    }),
                    Kind::Memory(_) => None,
                }
            };
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
                    places.push(Place::Registers(registers.into_iter().map(|index| self.argument_registers[index].clone()).collect()));
                }
                None => {
                    spilled = !self.skip_memory;
                    places.push(Place::Stack(stack));
                    let bytes = match kind {
                        Kind::Word => self.slot_bytes,
                        Kind::Sized(bytes) => bytes.max(self.slot_bytes),
                        Kind::Wide => self.slot_bytes * self.wide_slots,
                        Kind::Memory(bytes) => bytes.max(self.slot_bytes),
                    };
                    stack += (bytes + self.slot_bytes - 1) / self.slot_bytes * self.slot_bytes;
                }
            }
        }
        Placement { places, used, stack_bytes: stack }
    }

    /// The registers a call changes that the callee does not keep: what the convention clobbers,
    /// the registers it passed arguments in where those are the callee's, and those it returned a
    /// result of `width` bytes in where those are.
    pub fn clobbers(&self, used: &[String], result_width: Option<i64>) -> Vec<String> {
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

    /// `name` as an object in `format` (`omf`, `elf`, `macho`) spells it under this convention, where the description gives it.
    pub fn decorated(&self, format: &str, name: &str) -> Option<String> {
        self.symbol.get(format).map(|pattern| spell(pattern, name))
    }

    /// The registers kept for the caller that a value may be held in: all but the frame register.
    pub fn callee_saved(&self) -> Vec<&Kept> {
        self.preserved.iter().filter(|one| one.full != self.frame && one.pushed != self.frame).collect()
    }
}

/// `name` in a symbol `pattern`: `*` is the name, `^*` the name in capitals.
pub fn spell(pattern: &str, name: &str) -> String {
    pattern.replace("^*", &name.to_ascii_uppercase()).replace('*', name)
}

fn names_as_integers(value: Option<&toml::Value>, at: &str) -> Result<Vec<i64>, String> {
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
        assert_eq!((one.slot_bytes, one.order, one.cleanup, one.first_argument_offset, one.first_argument_offset_far), (2, Order::LeftToRight, Cleanup::Callee, 4, Some(6)));
        // The frame register is kept, but no value is held in it.
        assert_eq!(one.preserved.len(), 2);
        assert_eq!(one.callee_saved(), [&Kept { full: "esi".into(), pushed: "si".into() }]);
        // A width no entry names takes the narrowest that holds it.
        assert_eq!((one.result_registers(2).unwrap(), one.result_registers(4).unwrap(), one.result_registers(8)), (&["eax".to_owned(), "edx".to_owned()][..], &["eax".to_owned(), "edx".to_owned()][..], None));
        assert_eq!(one.result_registers(1).unwrap(), ["eax".to_owned()]);
    }

    /// A key the schema does not read is a typo or a fact nobody reads: refused.
    #[test]
    fn a_convention_is_like_an_earlier_one_but_for_what_it_states() {
        let text = format!("{ONE}[d]\nlike = \"c\"\ncleanup = \"caller\"\n[d.result]\n1 = [\"ax\"]\n");
        let calling = Calling::parse(&text).unwrap();
        let d = calling.named("d").unwrap();
        assert_eq!((d.cleanup, d.slot_bytes, d.order), (Cleanup::Caller, 2, Order::LeftToRight));
        assert_eq!(d.results.len(), 1);
        assert!(Calling::parse(&text.replace("like = \"c\"", "like = \"z\"")).unwrap_err().contains("not given before it"));
    }

    #[test]
    fn a_key_nobody_reads_is_refused() {
        let error = Calling::parse(&ONE.replace("wide_slots = 2", "wide_slots = 2\nred_zone = 128")).unwrap_err();
        assert_eq!(error, "calling.toml: c.red_zone: no such key");
        assert!(Calling::parse(&ONE.replace("left-to-right", "sideways")).unwrap_err().contains("not right-to-left or left-to-right"));
    }

    /// Open Watcom's flat register convention, as `wcc386 -3r` emits it (read from its disassembly).
    const WATCALL: &str = "default = \"w\"\n[abi.w]\nconvention = \"w\"\n[w]\nslot_bytes = 4\norder = \"right-to-left\"\ncleanup = \"callee\"\nargument_registers = [\"eax\", \"edx\", \"ebx\", \"ecx\"]\nwide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\nbackfill = true\narguments_clobbered = true\nresults_clobbered = true\nreturn_address_bytes = 4\nfirst_argument_offset = 8\nframe = \"ebp\"\nstack = \"esp\"\npreserved = [\"ebx\", \"ecx\", \"edx\", \"esi\", \"edi\", \"ebp\"]\nclobbered = [\"eax\", \"flags\"]\nentry_state = []\npromotion = \"slot\"\nwide_slots = 2\nvariadic_float = \"double\"\n[w.result]\n4 = [\"eax\"]\n8 = [\"eax\", \"edx\"]\n";

    fn registers(names: &[&str]) -> Place {
        Place::Registers(names.iter().map(|one| (*one).to_owned()).collect())
    }

    /// `wcc386` put `(int a, i64 b, int c)` in EAX, EBX:ECX and EDX: the pair EDX:EBX does not exist,
    /// and EDX stayed free for `c`. A rule that took the next two registers in a row gave b EDX:EBX.
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

    /// `wcc386` sent `(int, double, int)` to EAX and two stack cells: the first argument that fits no
    /// register sends every later one to the stack, though EDX was free. Also `(int, int, int, i64)`:
    /// only ECX was left, so the i64 went to the stack.
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
        assert_eq!((six.places[4].clone(), six.places[5].clone(), six.stack_bytes), (Place::Stack(0), Place::Stack(4), 8));
    }

    /// gcc's regparm(3) puts `(float, int b, double, int d, int e)` in EAX, EDX, ECX (b, d, e) and the float and the double on the
    /// stack: it counts integers only. With `skip_memory` off the float stopped the rest, as Open Watcom's does (b, d, e to the stack).
    #[test]
    fn an_argument_in_memory_leaves_the_registers_to_the_later_ones_where_the_convention_says_so() {
        let regparm = WATCALL.replace("wide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\n", "").replace("\"ebx\", ", "").replace("backfill = true\n", "skip_memory = true\n");
        let calling = Calling::parse(&regparm).unwrap();
        let placed = calling.native().place(&[Kind::Memory(4), Kind::Word, Kind::Memory(8), Kind::Word, Kind::Word, Kind::Word]);
        assert_eq!(placed.places, [Place::Stack(0), registers(&["eax"]), Place::Stack(4), registers(&["edx"]), registers(&["ecx"]), Place::Stack(12)]);
        let watcom = Calling::parse(WATCALL).unwrap();
        let placed = watcom.native().place(&[Kind::Memory(4), Kind::Word]);
        assert_eq!(placed.places, [Place::Stack(0), Place::Stack(4)]);
    }

    /// A long that reaches the stack takes its own two words: `(int, int, int, long)` has the long at the stack's start and
    /// `(long, long, long, long, long)` the fifth four bytes past the fourth. A word's worth put the next one in its middle.
    #[test]
    fn a_sized_argument_takes_its_bytes_on_the_stack() {
        let regparm = WATCALL.replace("wide_pairs = [[\"eax\", \"edx\"], [\"ebx\", \"ecx\"]]\n", "").replace("\"ebx\", ", "").replace("slot_bytes = 4", "slot_bytes = 2");
        let calling = Calling::parse(&regparm).unwrap();
        let placed = calling.native().place(&[Kind::Sized(4); 6]);
        assert_eq!(placed.places[3..], [Place::Stack(0), Place::Stack(4), Place::Stack(8)]);
        assert_eq!(placed.stack_bytes, 12);
    }

    /// A call changes EAX, the registers its arguments went in and EDX for an i64 result, and keeps
    /// the rest: `wcc386` held a value in ECX across `h2(i64, int)` and one in EDX across `h1(int)`.
    #[test]
    fn a_call_changes_the_registers_of_its_arguments_and_results_only() {
        let calling = Calling::parse(WATCALL).unwrap();
        let w = calling.native();
        let used = w.place(&[Kind::Word]).used;
        assert_eq!(w.clobbers(&used, Some(4)), ["eax", "flags"]);
        let used = w.place(&[Kind::Wide, Kind::Word]).used;
        assert_eq!(w.clobbers(&used, Some(8)), ["eax", "ebx", "edx", "flags"]);
    }

    /// An ELF object has no leading underscore and an OMF one a trailing one: the description says so for each
    /// convention, and a pattern the front end recorded (OMF's) is turned into the format's.
    #[test]
    fn a_symbol_is_decorated_as_its_object_format_spells_it() {
        let text = format!("{WATCALL}[w.symbol]\nomf = \"*_\"\nelf = \"*\"\nmacho = \"_*\"\n[c]\nlike = \"w\"\ncc = \"cdecl\"\n[c.symbol]\nomf = \"_*\"\nelf = \"*\"\nmacho = \"_*\"\n");
        let calling = Calling::parse(&text).unwrap();
        assert_eq!(calling.named("w").unwrap().decorated("omf", "f").as_deref(), Some("f_"));
        assert_eq!(calling.named("c").unwrap().decorated("macho", "f").as_deref(), Some("_f"));
        assert_eq!((calling.redecorated("*_", "elf").as_deref(), calling.redecorated("_*", "elf").as_deref(), calling.redecorated("_*", "macho").as_deref()), (Some("*"), Some("*"), Some("_*")));
        assert_eq!((calling.redecorated("^", "elf"), calling.redecorated("*_", "coff")), (None, None));
    }

    /// The default ABI is the file's `default`, not its first entry: a second family is chosen by name, and one the target has not is
    /// refused with the ones it has.
    #[test]
    fn an_abi_family_names_the_convention_an_unmarked_function_has() {
        let text = format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\n").replace("default = \"w\"\n[abi.w]\nconvention = \"w\"\n", "default = \"stack\"\n[abi.reg]\nconvention = \"w\"\n[abi.stack]\nconvention = \"c\"\n");
        let calling = Calling::parse(&text).unwrap();
        assert_eq!((calling.native().name.as_str(), calling.chosen(Some("reg")).unwrap().name.as_str()), ("c", "w"));
        assert_eq!(calling.chosen(Some("gcc")).unwrap_err(), "no ABI \"gcc\": this target has reg, stack");
        assert!(Calling::parse(&text.replace("default = \"stack\"", "default = \"nowhere\"")).unwrap_err().contains("no ABI \"nowhere\""));
    }

    /// A private function takes the convention `private` names under any ABI; only the ABIs' own conventions may be replaced, and a
    /// name the file does not give is refused.
    #[test]
    fn a_private_function_has_the_convention_the_description_names() {
        let stack = format!("{WATCALL}[c]\nlike = \"w\"\ncc = \"cdecl\"\n[p]\nlike = \"c\"\ncc = \"pascal\"\n").replace("default = \"w\"\n[abi.w]\nconvention = \"w\"\n", "default = \"stack\"\nprivate = \"w\"\n[abi.reg]\nconvention = \"w\"\n[abi.stack]\nconvention = \"c\"\n");
        let calling = Calling::parse(&stack).unwrap();
        assert_eq!(calling.private().map(|one| one.name.as_str()), Some("w"));
        let named = |name: &str| calling.named(name).unwrap();
        assert!(calling.replaceable(named("c")) && calling.replaceable(named("w")) && !calling.replaceable(named("p")));
        assert!(Calling::parse(&stack.replace("private = \"w\"", "private = \"nowhere\"")).unwrap_err().contains("private is the convention nowhere"));
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

    /// The interrupt frame was a constant in llrm-mir beside the description; a convention states it now. A slot that is
    /// not `[register, bytes]` was nobody's error before and is refused.
    #[test]
    fn test_an_interrupt_frame_is_read_from_the_description() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../llrm-x86-m16/src/machines/calling.toml")).unwrap();
        let calling = Calling::parse(&text).unwrap();
        let frame = &calling.interrupt().expect("m16 has an interrupt handler").interrupt_frame;
        assert_eq!(frame.len(), 15);
        assert_eq!(frame[0], ("gs".to_owned(), 2));
        assert_eq!(frame.iter().map(|slot| slot.1).sum::<i64>(), 46);
        let bad = text.replace("[\"gs\", 2]", "[\"gs\"]");
        assert!(Calling::parse(&bad).is_err());
    }
}
