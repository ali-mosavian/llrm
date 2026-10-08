//! How a target's operations are priced from the instruction forms that make
//! them: `opcosts.txt`. A line `operation = sum` says an operation's cost as
//! a sum of form prices, `[bytes]` the code bytes of each form and `[size]`
//! what differs when the same sums are over bytes. The prices of a CPU are
//! `timings.rs`'s.

use llrm_mir::target::{OperationCosts, carry_cost, step_cost};

/// One cost: a sum of terms, or one of MIR's named combinations.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Cost {
    /// `[n *] form` and `n`, added or subtracted.
    Sum(Vec<(i64, Option<String>)>),
    /// The CPU's operand-size prefix cost.
    Prefix,
    /// `llrm_mir::target::carry_cost(extend, add, shift, move)`.
    Carry([String; 4]),
    /// `llrm_mir::target::step_cost(add)`.
    Step(String),
}

/// A target's operations, as sums over a price table.
#[derive(Clone, Debug)]
pub struct Description {
    operations: Vec<(String, Cost)>,
    bytes: Vec<(String, i64)>,
    sizes: Vec<(String, Cost)>,
}

fn cost(text: &str, line: usize) -> Result<Cost, String> {
    let bad = |what: &str| format!("opcosts.txt:{line}: {what}");
    let text = text.trim();
    if text == "prefix" {
        return Ok(Cost::Prefix);
    }
    let call = |name: &str| text.strip_prefix(name).and_then(|rest| rest.strip_prefix('(')).and_then(|rest| rest.strip_suffix(')'));
    if let Some(arguments) = call("carry") {
        let names: Vec<String> = arguments.split(',').map(|one| one.trim().to_owned()).collect();
        let names: [String; 4] = names.try_into().map_err(|_| bad("carry takes four forms"))?;
        return Ok(Cost::Carry(names));
    }
    if let Some(argument) = call("step") {
        return Ok(Cost::Step(argument.trim().to_owned()));
    }
    let mut terms = Vec::new();
    let mut sign = 1;
    for token in text.split_whitespace() {
        match token {
            "+" => sign = 1,
            "-" => sign = -1,
            term => {
                let (count, form) = match term.split_once('*') {
                    Some((count, form)) => (count.trim().parse().map_err(|_| bad(&format!("`{count}` is no count")))?, Some(form.trim().to_owned())),
                    None => match term.parse::<i64>() {
                        Ok(number) => (number, None),
                        Err(_) => (1, Some(term.to_owned())),
                    },
                };
                terms.push((sign * count, form));
                sign = 1;
            }
        }
    }
    if terms.is_empty() {
        return Err(bad("no cost"));
    }
    Ok(Cost::Sum(terms))
}

const OPERATIONS: [&str; 32] = [
    "add", "multiply", "divide", "shift", "address", "carry", "carry_step", "load", "store", "memory_update", "branch", "prefix", "move", "call", "return", "argument", "pop", "adjust",
    "return_pops", "float_add", "float_multiply", "float_divide", "float_load", "float_store", "float_release", "extend", "fill", "fill_cell", "copy", "copy_cell", "direction", "unroll_budget",
];

impl Description {
    pub fn parse(text: &str) -> Result<Self, String> {
        let (mut operations, mut bytes, mut sizes) = (Vec::new(), Vec::new(), Vec::new());
        let mut section = "operations";
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                if !["operations", "bytes", "size"].contains(&name) {
                    return Err(format!("opcosts.txt:{}: no section [{name}]", index + 1));
                }
                section = if name == "operations" { "operations" } else if name == "bytes" { "bytes" } else { "size" };
                continue;
            }
            let (name, value) = line.split_once('=').ok_or_else(|| format!("opcosts.txt:{}: `{line}` has no `=`", index + 1))?;
            let name = name.trim().to_owned();
            if section == "bytes" {
                bytes.push((name, value.trim().parse().map_err(|_| format!("opcosts.txt:{}: `{value}` is no byte count", index + 1))?));
                continue;
            }
            if !OPERATIONS.contains(&name.as_str()) {
                return Err(format!("opcosts.txt:{}: no operation `{name}`", index + 1));
            }
            let cost = cost(value, index + 1)?;
            if section == "operations" { operations.push((name, cost)) } else { sizes.push((name, cost)) }
        }
        for name in OPERATIONS {
            if !operations.iter().any(|(one, _)| one == name) {
                return Err(format!("opcosts.txt: operation `{name}` has no cost"));
            }
        }
        Ok(Self { operations, bytes, sizes })
    }

    /// The operations priced by `price` (a form's cost), `prefix` the CPU's
    /// operand-size prefix cost.
    pub fn operations(&self, price: &dyn Fn(&str) -> i64, prefix: i64) -> OperationCosts {
        Self::evaluated(&self.operations, &[], price, prefix)
    }

    /// The operations in code bytes: the same sums over the forms' bytes, but
    /// where `[size]` says otherwise.
    pub fn size_costs(&self) -> OperationCosts {
        let price = |form: &str| self.bytes.iter().find(|(one, _)| one == form).unwrap_or_else(|| panic!("opcosts.txt has no bytes for {form}")).1;
        Self::evaluated(&self.operations, &self.sizes, &price, 0)
    }

    /// The code bytes of a form.
    pub fn bytes(&self, form: &str) -> Option<i64> {
        self.bytes.iter().find(|(one, _)| one == form).map(|(_, bytes)| *bytes)
    }

    fn evaluated(operations: &[(String, Cost)], replaced: &[(String, Cost)], price: &dyn Fn(&str) -> i64, prefix: i64) -> OperationCosts {
        let value = |cost: &Cost| match cost {
            Cost::Prefix => prefix,
            Cost::Sum(terms) => terms.iter().map(|(count, form)| count * form.as_deref().map_or(1, price)).sum(),
            Cost::Carry([extend, add, shift, r#move]) => carry_cost(price(extend), price(add), price(shift), price(r#move)),
            Cost::Step(add) => step_cost(price(add)),
        };
        let mut out = OperationCosts::default();
        for (name, cost) in operations {
            let cost = replaced.iter().find(|(one, _)| one == name).map_or(cost, |(_, replacement)| replacement);
            let value = value(cost);
            match name.as_str() {
                "add" => out.add = value,
                "multiply" => out.multiply = value,
                "divide" => out.divide = value,
                "shift" => out.shift = value,
                "address" => out.address = value,
                "carry" => out.carry = value,
                "carry_step" => out.carry_step = value,
                "load" => out.load = value,
                "store" => out.store = value,
                "memory_update" => out.memory_update = value,
                "branch" => out.branch = value,
                "prefix" => out.prefix = value,
                "move" => out.r#move = value,
                "call" => out.call = value,
                "return" => out.return_ = value,
                "argument" => out.argument = value,
                "pop" => out.pop = value,
                "adjust" => out.adjust = value,
                "return_pops" => out.return_pops = value,
                "float_add" => out.float_add = value,
                "float_multiply" => out.float_multiply = value,
                "float_divide" => out.float_divide = value,
                "float_load" => out.float_load = value,
                "float_store" => out.float_store = value,
                "float_release" => out.float_release = value,
                "extend" => out.extend = value,
                "fill" => out.fill = value,
                "fill_cell" => out.fill_cell = value,
                "copy" => out.copy = value,
                "copy_cell" => out.copy_cell = value,
                "direction" => out.direction = value,
                "unroll_budget" => out.unroll_budget = value,
                other => unreachable!("`{other}` was checked"),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &str = "add = a\nmultiply = m\ndivide = d\nshift = s\naddress = l\ncarry = carry(z, a, s, r)\ncarry_step = step(a)\nload = ld\nstore = st\nmemory_update = u\nbranch = j\nprefix = prefix\nmove = r\ncall = c\nreturn = t\nargument = st + ld\npop = p\nadjust = i\nreturn_pops = rp - t\nfloat_add = fa\nfloat_multiply = fm\nfloat_divide = fd\nfloat_load = fl\nfloat_store = fs\nfloat_release = fr\nextend = z\nfill = f + 2*p + 3\nfill_cell = fc\ncopy = cp\ncopy_cell = cc\ndirection = 2*a\nunroll_budget = 7\n";

    #[test]
    fn a_sum_is_terms_with_counts_and_signs() {
        let description = Description::parse(ALL).unwrap();
        let prices = |form: &str| match form { "st" => 3, "ld" => 4, "rp" => 10, "t" => 6, "f" => 5, "p" => 7, "a" => 1, other => other.len() as i64 };
        let costs = description.operations(&prices, 9);
        assert_eq!((costs.argument, costs.return_pops, costs.fill, costs.prefix, costs.direction), (7, 4, 5 + 14 + 3, 9, 2));
    }

    #[test]
    fn size_costs_replace_what_size_says_and_price_by_bytes() {
        let text = format!("{ALL}[bytes]\n{}\n[size]\nprefix = 1\nargument = 2\n", ["a", "m", "d", "s", "l", "z", "r", "ld", "st", "u", "j", "c", "t", "p", "i", "rp", "fa", "fm", "fd", "fl", "fs", "fr", "f", "fc", "cp", "cc"].iter().map(|one| format!("{one} = 4")).collect::<Vec<_>>().join("\n"));
        let costs = Description::parse(&text).unwrap().size_costs();
        assert_eq!((costs.prefix, costs.argument, costs.add, costs.fill), (1, 2, 4, 4 + 8 + 3));
    }

    #[test]
    fn an_operation_missing_or_unknown_is_refused() {
        assert_eq!(Description::parse("add = a\n").unwrap_err(), "opcosts.txt: operation `multiply` has no cost");
        assert_eq!(Description::parse(&format!("{ALL}bogus = a\n")).unwrap_err(), format!("opcosts.txt:{}: no operation `bogus`", ALL.lines().count() + 1));
    }
}
