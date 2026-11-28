//! The clocks of a `mos_rt.s` routine on given arguments, by running its text.
//!
//! The static estimate of [`crate::timing`] counts a loop as ten trips and both
//! arms of a branch each trip, which is no price for a 16-step divide whose
//! arms depend on the quotient. The figures the selection prices a routine at
//! (`hotdivision`) were measured once on sim65 and went stale with the next
//! edit of the runtime. Here the routine's own text runs on a model of the
//! registers, the flags, the zero page and the stack, each instruction costing
//! its [`crate::timing::instruction`] clocks and a taken branch one more, so a
//! figure is read from the runtime it prices.
//!
//! Page crossings are not modelled: sim65 charges a clock for a crossing, which
//! moves with where the code lies (`tests/runtime.rs`).
//!
//! The model has the instructions the divide routines use; any other stops the
//! run with an error naming it.

use llrm_support::hash::HashMap;

use crate::dagisel::hooks::Cpu;

const RUNTIME: &str = include_str!("../runtime/mos_rt.s");

/// One line of code: a mnemonic and its operand text.
struct Line {
    mnemonic: String,
    operand: String,
    /// The last global label before it: what its local labels belong to.
    scope: String,
}

/// The runtime as lines of code and the index each label stands at.
struct Program {
    lines: Vec<Line>,
    labels: HashMap<String, usize>,
}

impl Program {
    fn parse() -> Program {
        let expanded = crate::timing::expand_macros(RUNTIME);
        let mut lines = Vec::new();
        let mut labels = HashMap::default();
        let mut scope = String::new();
        let mut in_macro = false;
        for raw in expanded.lines() {
            let mut code = raw.split(';').next().unwrap_or("").trim();
            if code.starts_with(".macro") {
                in_macro = true;
            }
            if in_macro {
                in_macro = !code.starts_with(".endmacro");
                continue;
            }
            if let Some((label, rest)) = code.split_once(':') {
                if !label.contains(char::is_whitespace) {
                    let key = if label.starts_with('@') { format!("{scope}{label}") } else { label.to_owned() };
                    if !label.starts_with('@') {
                        scope = label.to_owned();
                    }
                    labels.insert(key, lines.len());
                    code = rest.trim();
                }
            }
            if code.is_empty() || code.starts_with('.') || code.contains('=') {
                continue;
            }
            let (mnemonic, operand) = code.split_once(char::is_whitespace).unwrap_or((code, ""));
            lines.push(Line {
                mnemonic: mnemonic.to_ascii_lowercase(),
                operand: operand.trim().to_owned(),
                scope: scope.clone(),
            });
        }
        Program { lines, labels }
    }
}

struct Machine {
    a: u8,
    x: u8,
    y: u8,
    carry: bool,
    zero: bool,
    negative: bool,
    zp: [u8; 256],
    stack: Vec<u8>,
}

impl Machine {
    fn set_nz(
        &mut self,
        value: u8,
    ) -> u8 {
        self.zero = value == 0;
        self.negative = value & 0x80 != 0;
        value
    }

    fn flags(&self) -> u8 {
        u8::from(self.carry) | u8::from(self.zero) << 1 | u8::from(self.negative) << 7
    }

    fn set_flags(
        &mut self,
        flags: u8,
    ) {
        self.carry = flags & 1 != 0;
        self.zero = flags & 2 != 0;
        self.negative = flags & 0x80 != 0;
    }

    /// The zero-page index of `rcN` or `rcN,x` / `rcN,y`.
    fn cell(
        &self,
        operand: &str,
    ) -> Result<usize, String> {
        let (name, index) = match operand.split_once(',') {
            Some((name, "x")) => (name, self.x),
            Some((name, "y")) => (name, self.y),
            _ => (operand, 0),
        };
        let n: usize = name
            .strip_prefix("rc")
            .and_then(|digits| digits.parse().ok())
            .ok_or_else(|| format!("operand `{operand}` is not a cell"))?;
        Ok((n + usize::from(index)) & 0xFF)
    }

    /// The value an instruction reads: an immediate or a cell.
    fn read(
        &self,
        operand: &str,
    ) -> Result<u8, String> {
        match operand.strip_prefix('#') {
            Some(number) => match number.strip_prefix('$') {
                Some(hex) => u8::from_str_radix(hex, 16).map_err(|e| e.to_string()),
                None => number.parse().map_err(|_| format!("immediate `{operand}` is not a number")),
            },
            None => Ok(self.zp[self.cell(operand)?]),
        }
    }
}

/// The clocks `routine` takes from its first instruction to the return that
/// leaves it, `cells` (`rcN`, value) set before, the cells it leaves, and the
/// registers (A, X) it returns.
pub struct Run {
    pub clocks: u64,
    pub a: u8,
    pub x: u8,
    pub cells: [u8; 256],
}

/// Runs `routine` of `mos_rt.s` on `cpu`.
pub fn run(
    cpu: Cpu,
    routine: &str,
    cells: &[(u8, u8)],
) -> Result<Run, String> {
    static PROGRAM: std::sync::OnceLock<Program> = std::sync::OnceLock::new();
    let program = PROGRAM.get_or_init(Program::parse);
    let mut machine =
        Machine { a: 0, x: 0, y: 0, carry: false, zero: false, negative: false, zp: [0; 256], stack: Vec::new() };
    for &(n, value) in cells {
        machine.zp[usize::from(n)] = value;
    }
    let mut at = *program.labels.get(routine).ok_or_else(|| format!("no `{routine}` in mos_rt.s"))?;
    let mut clocks = 0u64;
    let mut calls: Vec<usize> = Vec::new();
    for _ in 0..200_000 {
        let Some(line) = program.lines.get(at) else { return Err(format!("ran off the end of the runtime at {at}")) };
        let (mnemonic, operand) = (line.mnemonic.as_str(), line.operand.as_str());
        let cost = crate::timing::instruction(cpu, mnemonic, operand)
            .ok_or_else(|| format!("`{mnemonic} {operand}` is not priced"))?;
        clocks += u64::from(cost.cycles);
        let target = |name: &str| -> Result<usize, String> {
            let local = format!("{}{name}", line.scope);
            program.labels.get(&local).or_else(|| program.labels.get(name)).copied().ok_or(format!("no label `{name}`"))
        };
        let mut next = at + 1;
        let accumulator = operand.eq_ignore_ascii_case("a");
        match mnemonic {
            "lda" => {
                let v = machine.read(operand)?;
                machine.a = machine.set_nz(v);
            }
            "ldx" => {
                let v = machine.read(operand)?;
                machine.x = machine.set_nz(v);
            }
            "ldy" => {
                let v = machine.read(operand)?;
                machine.y = machine.set_nz(v);
            }
            "sta" | "stx" | "sty" => {
                let value = match mnemonic {
                    "sta" => machine.a,
                    "stx" => machine.x,
                    _ => machine.y,
                };
                let cell = machine.cell(operand)?;
                machine.zp[cell] = value;
            }
            "cmp" | "cpx" | "cpy" => {
                let register = match mnemonic {
                    "cmp" => machine.a,
                    "cpx" => machine.x,
                    _ => machine.y,
                };
                let value = machine.read(operand)?;
                machine.carry = register >= value;
                machine.set_nz(register.wrapping_sub(value));
            }
            "adc" | "sbc" => {
                let value = machine.read(operand)?;
                let value = if mnemonic == "sbc" { !value } else { value };
                let sum = u16::from(machine.a) + u16::from(value) + u16::from(machine.carry);
                machine.carry = sum > 0xFF;
                machine.a = machine.set_nz(sum as u8);
            }
            "and" | "ora" | "eor" => {
                let value = machine.read(operand)?;
                let result = match mnemonic {
                    "and" => machine.a & value,
                    "ora" => machine.a | value,
                    _ => machine.a ^ value,
                };
                machine.a = machine.set_nz(result);
            }
            "asl" | "lsr" | "rol" | "ror" => {
                let cell = if accumulator { 0 } else { machine.cell(operand)? };
                let value = if accumulator { machine.a } else { machine.zp[cell] };
                let carry_in = u8::from(machine.carry);
                let (result, carry) = match mnemonic {
                    "asl" => (value << 1, value & 0x80 != 0),
                    "lsr" => (value >> 1, value & 1 != 0),
                    "rol" => (value << 1 | carry_in, value & 0x80 != 0),
                    _ => (value >> 1 | carry_in << 7, value & 1 != 0),
                };
                machine.carry = carry;
                machine.set_nz(result);
                if accumulator {
                    machine.a = result;
                } else {
                    machine.zp[cell] = result;
                }
            }
            "inc" | "dec" => {
                let cell = machine.cell(operand)?;
                let result =
                    if mnemonic == "inc" { machine.zp[cell].wrapping_add(1) } else { machine.zp[cell].wrapping_sub(1) };
                machine.zp[cell] = machine.set_nz(result);
            }
            "inx" => machine.x = machine.set_nz(machine.x.wrapping_add(1)),
            "iny" => machine.y = machine.set_nz(machine.y.wrapping_add(1)),
            "dex" => machine.x = machine.set_nz(machine.x.wrapping_sub(1)),
            "dey" => machine.y = machine.set_nz(machine.y.wrapping_sub(1)),
            "tax" => machine.x = machine.set_nz(machine.a),
            "tay" => machine.y = machine.set_nz(machine.a),
            "txa" => machine.a = machine.set_nz(machine.x),
            "tya" => machine.a = machine.set_nz(machine.y),
            "sec" => machine.carry = true,
            "clc" => machine.carry = false,
            "pha" => machine.stack.push(machine.a),
            "pla" => {
                let v = machine.stack.pop().ok_or("pla on an empty stack")?;
                machine.a = machine.set_nz(v);
            }
            "php" => machine.stack.push(machine.flags()),
            "plp" => {
                let v = machine.stack.pop().ok_or("plp on an empty stack")?;
                machine.set_flags(v);
            }
            "bne" | "beq" | "bcc" | "bcs" | "bpl" | "bmi" => {
                let taken = match mnemonic {
                    "bne" => !machine.zero,
                    "beq" => machine.zero,
                    "bcc" => !machine.carry,
                    "bcs" => machine.carry,
                    "bpl" => !machine.negative,
                    _ => machine.negative,
                };
                if taken {
                    clocks += 1;
                    next = target(operand)?;
                }
            }
            "jmp" => next = target(operand)?,
            "jsr" => {
                calls.push(at + 1);
                next = target(operand)?;
            }
            "rts" => match calls.pop() {
                Some(back) => next = back,
                None => return Ok(Run { clocks, a: machine.a, x: machine.x, cells: machine.zp }),
            },
            other => return Err(format!("`{other} {operand}` is not modelled")),
        }
        at = next;
    }
    Err(format!("`{routine}` ran 200000 instructions"))
}
