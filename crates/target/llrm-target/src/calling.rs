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
    pub pointer: String,
    pub pointer_returned: String,
    pub pointer_popped_by: String,
}

/// One calling convention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Convention {
    pub name: String,
    /// What a byte argument takes on the stack, and the least a cell holds.
    pub slot_bytes: i64,
    pub order: Order,
    pub cleanup: Cleanup,
    pub argument_registers: Vec<String>,
    pub return_address_bytes: i64,
    /// Where the first argument lies from the frame register, past the saved frame register and
    /// the return address.
    pub first_argument_offset: i64,
    /// The same when the call is far, where the convention has far calls.
    pub first_argument_offset_far: Option<i64>,
    /// The register a frame's cells are addressed through, and the stack pointer.
    pub frame: String,
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
}

/// A target's conventions, in file order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Calling {
    pub conventions: Vec<Convention>,
}

const KEYS: [&str; 22] = [
    "slot_bytes",
    "order",
    "cleanup",
    "argument_registers",
    "return_address_bytes",
    "first_argument_offset",
    "first_argument_offset_far",
    "frame",
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
];

impl Calling {
    /// `text`, a `calling.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let mut conventions = Vec::new();
        let mut stated: Vec<(&String, toml::Table)> = Vec::new();
        for (name, value) in &table {
            let mut one = value.as_table().ok_or_else(|| format!("calling.toml: {name} is not a table"))?.clone();
            // `like = "other"`: everything `other` states that this does not.
            if let Some(like) = one.remove("like") {
                let like = like.as_str().ok_or_else(|| format!("calling.toml: {name}.like is not a name"))?;
                let base = stated.iter().find(|(one, _)| one.as_str() == like).ok_or_else(|| format!("calling.toml: {name} is like {like}, which is not given before it"))?;
                for (key, value) in &base.1 {
                    one.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            conventions.push(Convention::parse(name, &one)?);
            stated.push((name, one));
        }
        if conventions.is_empty() {
            return Err("calling.toml: no convention".to_owned());
        }
        Ok(Self { conventions })
    }

    /// The convention a language's own functions use: the file's first.
    pub fn native(&self) -> &Convention {
        &self.conventions[0]
    }

    /// The names, in file order.
    pub fn names(&self) -> Vec<&str> {
        self.conventions.iter().map(|one| one.name.as_str()).collect()
    }

    pub fn named(&self, name: &str) -> Option<&Convention> {
        self.conventions.iter().find(|one| one.name == name)
    }
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
                pointer_returned: text("aggregate_pointer_returned")?,
                pointer_popped_by: text("aggregate_pointer_popped_by")?,
            }),
        };
        Ok(Self {
            name: name.to_owned(),
            slot_bytes: integer("slot_bytes")?,
            order,
            cleanup,
            argument_registers: names(table.get("argument_registers"), "argument_registers")?,
            return_address_bytes: integer("return_address_bytes")?,
            first_argument_offset: integer("first_argument_offset")?,
            first_argument_offset_far: table.contains_key("first_argument_offset_far").then(|| integer("first_argument_offset_far")).transpose()?,
            frame: text("frame")?,
            stack: text("stack")?,
            preserved,
            clobbered: names(table.get("clobbered"), "clobbered")?,
            entry_state: names(table.get("entry_state"), "entry_state")?,
            results,
            aggregate,
            promotion: text("promotion")?,
            wide_slots: integer("wide_slots")?,
            variadic_float: text("variadic_float")?,
        })
    }

    /// The registers a result `width` bytes wide leaves in, low part first; the widest entry that
    /// holds it where none is that width.
    pub fn result_registers(&self, width: i64) -> Option<&[String]> {
        let by_width = self.results.iter().filter_map(|(class, registers)| Some((class.parse::<i64>().ok()?, registers)));
        by_width.filter(|(bytes, _)| *bytes >= width).min_by_key(|(bytes, _)| *bytes).map(|(_, registers)| registers.as_slice())
    }

    /// The registers kept for the caller that a value may be held in: all but the frame register.
    pub fn callee_saved(&self) -> Vec<&Kept> {
        self.preserved.iter().filter(|one| one.full != self.frame && one.pushed != self.frame).collect()
    }
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

    const ONE: &str = "[c]\nslot_bytes = 2\norder = \"left-to-right\"\ncleanup = \"callee\"\nargument_registers = []\nreturn_address_bytes = 2\nfirst_argument_offset = 4\nfirst_argument_offset_far = 6\nframe = \"bp\"\nstack = \"sp\"\npreserved = [\"bp\", [\"esi\", \"si\"]]\nclobbered = [\"ax\"]\nentry_state = []\npromotion = \"slot\"\nwide_slots = 2\nvariadic_float = \"double\"\n[c.result]\n1 = [\"eax\"]\n4 = [\"eax\", \"edx\"]\n";

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
}
