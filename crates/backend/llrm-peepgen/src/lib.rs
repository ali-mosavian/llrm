//! Compiles llrm's peephole rules (`.peep`) against the instruction
//! description (`x86.instr`) into Rust: one decision automaton per rule
//! group and the rules at its accepting states. llrm-core's build script
//! runs it.

pub mod automaton;
pub mod emit;
pub mod resolve;
pub mod syntax;
pub mod table;

pub struct Generated {
    pub rules: String,
    /// (group, rules, states), in file order.
    pub groups: Vec<(String, usize, usize)>,
}

/// Every group's automaton, for a table and rules already read.
pub fn compile(table: &table::Table, rules: &str, file: &str) -> Result<(resolve::Program, Vec<automaton::Automaton>), String> {
    let parsed = syntax::parse(rules, file)?;
    let program = resolve::resolve(&parsed, table, file)?;
    let automata = program.groups.iter().map(automaton::build).collect();
    Ok((program, automata))
}

pub fn generate(table_source: &str, table_file: &str, rules: &str, rules_file: &str) -> Result<Generated, String> {
    let table = table::load(table_source, table_file)?;
    let (program, automata) = compile(&table, rules, rules_file)?;
    let groups = program
        .groups
        .iter()
        .zip(&automata)
        .map(|(group, automaton)| (group.name.clone(), group.rules.len(), automaton.states()))
        .collect();
    Ok(Generated {
        rules: emit::rules(&program, &table, &automata, rules_file)?,
        groups,
    })
}

#[cfg(test)]
mod tests;
