//! Which `x86.instr` form a pin machine opcode is.
//!
//! # How it works
//! The pin names an opcode `<MNEMONIC><width><operand letters>`: `ADD32rr`,
//! `ADD32mi8`, `MOVZX32rr8`. [`OpcodeMap::form`] reads the mnemonic, the
//! width and the letters (`r` register, `m` memory, `i` immediate; a width or
//! `CL`/`1` after them is a size or a fixed operand) and finds the form of
//! that name whose widths hold the width and whose destination and untied
//! sources admit the letters. A form's tied source (`^0`) has no letter: the
//! pin names it once, as the destination. Zero matches or more than one is
//! an [`Unmapped`] opcode, never a guess.
//!
//! ```text
//! ADD32mr  ->  add  rm/^0,rmi   dest m, untied source r
//! IMUL32rri ->  imul r/rm,i      (r/^0,rmi has one untied source, not two)
//! ```
//!
//! # Example
//! `ADD32rr` is the form `add` (`rm/^0,rmi`, 8/16/32); `ADD32ri8` is the same
//! form; `CMOV32rr` is unmapped, [`Unmapped::Condition`]: the pin has one
//! opcode where `x86.instr` expands `{cc}` into one form per condition.
//!
//! # Used by
//! The emitter (D1b), which builds LIR `Semantics` from the form.
//!
//! # See also
//! `llrm_target::instr::Form`; the pin's `X86GenInstrInfo.inc` for the names.

use llrm_target::instr::Form;

/// Why an opcode has no single form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unmapped {
    /// A target-independent opcode the emitter handles itself (`COPY`,
    /// `IMPLICIT_DEF`, `EXTRACT_SUBREG`, ...).
    Generic,
    /// An x87 pseudo (`ADD_Fp32m`, `LD_Fp64m`, ...): SD6, until `fpstack.rs`.
    X87,
    /// A vector or other opcode this port leaves out (SD13).
    Vector,
    /// A condition-coded opcode (`CMOV`, `SETCC`, `JCC`): one pin opcode, one
    /// form per condition.
    Condition,
    /// A 64-bit width on a target whose forms have none.
    Wide64,
    /// The name has no `<MNEMONIC><width><letters>` shape.
    Name,
    /// No form of that name admits the width and operand letters.
    NoForm,
    /// More than one form does.
    Ambiguous(Vec<usize>),
}

/// `x86.instr`'s forms, by mnemonic.
pub struct OpcodeMap {
    forms: Vec<Form>,
    /// [`Self::scalar_sse`], asked per operand width: found once.
    scalar_sse: bool,
    /// The answer for each name asked: every machine node of a function asks,
    /// and a name has one answer.
    asked: std::sync::Mutex<llrm_support::hash::HashMap<String, Result<usize, Unmapped>>>,
    /// [`memory_bytes`] for each name asked, parsed once.
    sized: std::sync::Mutex<llrm_support::hash::HashMap<String, Option<u32>>>,
}

impl OpcodeMap {
    pub fn new(forms: Vec<Form>) -> OpcodeMap {
        let scalar_sse = forms.iter().any(|form| form.name == "addsd");
        OpcodeMap { forms, scalar_sse, asked: Default::default(), sized: Default::default() }
    }

    /// Whether this target does scalar floating point in xmm registers (it has
    /// `addsd`): a floating value is then 4 or 8 bytes, not the x87's 10.
    pub fn scalar_sse(&self) -> bool {
        self.scalar_sse
    }

    /// [`memory_bytes`] of `name`, memoized: every machine node asks.
    pub fn memory_bytes(
        &self,
        name: &str,
    ) -> Option<u32> {
        let mut sized = self.sized.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(&answer) = sized.get(name) {
            return answer;
        }
        let answer = memory_bytes(name);
        sized.insert(name.to_owned(), answer);
        answer
    }

    pub fn forms(&self) -> &[Form] {
        &self.forms
    }

    /// The index of the one form `name` is, or why it is none.
    pub fn form(
        &self,
        name: &str,
    ) -> Result<usize, Unmapped> {
        if let Some(answer) = self.asked.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(name) {
            return answer.clone();
        }
        let answer = self.searched(name);
        self.asked.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(name.to_owned(), answer.clone());
        answer
    }

    fn searched(
        &self,
        name: &str,
    ) -> Result<usize, Unmapped> {
        let name = name.split('|').next().unwrap_or(name);
        // `ADD32ri_DB`: an add whose bits are disjoint, which two-address
        // conversion may make an `lea`; as an instruction it is an `or`
        // (`X86MCInstLower`)
        let disjoint;
        let name = match name.strip_suffix("_DB").and_then(|add| add.strip_prefix("ADD")) {
            Some(rest) => {
                disjoint = format!("OR{rest}");
                &disjoint
            }
            None => name,
        };
        if matches!(
            name,
            "COPY"
                | "IMPLICIT_DEF"
                | "EXTRACT_SUBREG"
                | "INSERT_SUBREG"
                | "SUBREG_TO_REG"
                | "COPY_TO_REGCLASS"
                | "REG_SEQUENCE"
        ) {
            return Err(Unmapped::Generic);
        }
        // `MOVSDrm_alt`: the load that may be rematerialised is the same form.
        let name = name.strip_suffix("_alt").unwrap_or(name);
        // A target with no SSE forms (the 32-bit one) keeps these as vector
        // opcodes below.
        // A move between a general and an SSE register: the pin's names for
        // the direction, the form is the one with that encoding.
        if let Some(iced) = gpr_xmm_move(name) {
            return self.forms.iter().position(|f| f.iced.as_deref() == Some(iced)).ok_or(Unmapped::NoForm);
        }
        if let Some((mnemonic, width, result, letters)) =
            sse(name).filter(|(m, ..)| self.forms.iter().any(|f| &f.name == m))
        {
            return self.single(&mnemonic, width, result, &letters);
        }
        if name.contains("_Fp") || name.starts_with('F') && name[1..].starts_with(|c: char| c.is_ascii_uppercase()) {
            return Err(Unmapped::X87);
        }
        if name.starts_with("V_")
            || name.starts_with('V') && name.contains("rr")
            || name.contains("MMX")
            || name.contains("PS")
            || name.contains("PD")
        {
            return Err(Unmapped::Vector);
        }
        let Some((mnemonic, width, letters)) = split(name) else { return Err(Unmapped::Name) };
        if matches!(mnemonic.as_str(), "cmov" | "set" | "j") || name.contains("CC") {
            return Err(Unmapped::Condition);
        }
        if width == Some(64) && !self.forms.iter().any(|form| form.widths.contains(&64)) {
            return Err(Unmapped::Wide64);
        }
        self.single(&mnemonic, width, None, &letters)
    }

    /// The one form of `mnemonic` at `width` that admits `letters`, and whose
    /// result is `result` bits where the name says (`CVTTSD2SI64rr`: both
    /// widths of a conversion state the source's).
    fn single(
        &self,
        mnemonic: &str,
        width: Option<u32>,
        result: Option<u32>,
        letters: &[char],
    ) -> Result<usize, Unmapped> {
        let found: Vec<usize> = self
            .forms
            .iter()
            .enumerate()
            .filter(|(_, form)| {
                form.name == mnemonic
                    && width.is_none_or(|w| form.widths.contains(&w))
                    && result.is_none_or(|bits| {
                        form.iced.as_deref().is_some_and(|iced| iced.contains(&format!("_r{bits}_")))
                    })
                    && admits(form, letters)
            })
            .map(|(at, _)| at)
            .collect();
        match found[..] {
            [] => Err(Unmapped::NoForm),
            [one] => Ok(one),
            _ => Err(Unmapped::Ambiguous(found)),
        }
    }
}

/// `MOVSDto64rr`: `movq r64, xmm`; the iced name of the form each pin name
/// of a move between a general and an SSE register selects.
fn gpr_xmm_move(name: &str) -> Option<&'static str> {
    Some(match name {
        "MOVSDto64rr" => "Movq_rm64_xmm",
        "MOV64toSDrr" => "Movq_xmm_rm64",
        "MOVSS2DIrr" => "Movd_rm32_xmm",
        "MOVDI2SSrr" => "Movd_xmm_rm32",
        _ => return None,
    })
}

/// The scalar SSE instructions the pin names `ADDSDrr`, `MOVSSrm`, `XORPSrr`,
/// `CVTSI642SDrr`: (the form's mnemonic, its width if the name fixes it, the
/// operand letters). `MOVSD` is the string copy in our table; the scalar move
/// is `movsd_x`. `SQRTSDr` names its tied source too. A pin name that is none
/// of these is `None`.
fn sse(name: &str) -> Option<(String, Option<u32>, Option<u32>, Vec<char>)> {
    const ARITHMETIC: [&str; 22] = [
        "ADDSS", "ADDSD", "SUBSS", "SUBSD", "MULSS", "MULSD", "DIVSS", "DIVSD", "MINSS", "MINSD", "MAXSS", "MAXSD",
        "SQRTSS", "SQRTSD", "MOVSS", "MOVSD", "UCOMISS", "UCOMISD", "XORPS", "ANDPS", "ANDNPS", "ORPS",
    ];
    const MOVES: [&str; 1] = ["MOVAPS"];
    const CONVERTS: [(&str, &str, Option<u32>); 8] = [
        ("CVTSS2SD", "cvtss2sd", None),
        ("CVTSD2SS", "cvtsd2ss", None),
        ("CVTSI2SS", "cvtsi2ss", Some(32)),
        ("CVTSI642SS", "cvtsi2ss", Some(64)),
        ("CVTSI2SD", "cvtsi2sd", Some(32)),
        ("CVTSI642SD", "cvtsi2sd", Some(64)),
        ("CVTSD2SS", "cvtsd2ss", None),
        ("CVTSS2SD", "cvtss2sd", None),
    ];
    // The result's width is in the name: `CVTTSD2SIrr` 32 bits, `CVTTSD2SI64rr`
    // 64.
    const TRUNCATES: [(&str, &str, u32); 4] = [
        ("CVTTSD2SI64", "cvttsd2si", 64),
        ("CVTTSD2SI", "cvttsd2si", 32),
        ("CVTTSS2SI64", "cvttss2si", 64),
        ("CVTTSS2SI", "cvttss2si", 32),
    ];
    let letters_of = |tail: &str| -> Option<Vec<char>> {
        (!tail.is_empty() && tail.chars().all(|c| c == 'r' || c == 'm')).then(|| tail.chars().collect())
    };
    let mut found = None;
    for prefix in ARITHMETIC.iter().chain(&MOVES) {
        if let Some(letters) = name.strip_prefix(prefix).and_then(letters_of) {
            let mnemonic = if *prefix == "MOVSD" { "movsd_x".to_owned() } else { prefix.to_ascii_lowercase() };
            // The longest prefix wins (`ANDNPS` is not `ANDPS`).
            if found
                .as_ref()
                .is_none_or(|(old, ..): &(String, Option<u32>, Option<u32>, Vec<char>)| mnemonic.len() > old.len())
            {
                found = Some((mnemonic, None, None, letters));
            }
        }
    }
    for (prefix, mnemonic, width) in CONVERTS {
        if let Some(letters) = name.strip_prefix(prefix).and_then(letters_of) {
            found = Some((mnemonic.to_owned(), width, None, letters));
        }
    }
    for (prefix, mnemonic, result) in TRUNCATES {
        if let Some(letters) = name.strip_prefix(prefix).and_then(letters_of) {
            found = Some((mnemonic.to_owned(), None, Some(result), letters));
        }
    }
    found.map(|(mnemonic, width, result, letters)| {
        let letters = if mnemonic.starts_with("sqrt") && letters.len() == 1 { vec!['r', 'r'] } else { letters };
        (mnemonic, width, result, letters)
    })
}

/// The bytes a memory operand of `name` reads or writes: the width in the
/// name (`MOV16mr`: 2, `ADD32mi8`: 4), or for an extension (`MOVZX32rm8`,
/// `MOVSX32rm16`) the size after the letters, the source's. `None` if the
/// name has no width.
pub fn memory_bytes(name: &str) -> Option<u32> {
    // `MOV8mr_NOREX` stores `AH`: the byte store it stands for.
    let name = name.strip_suffix("_NOREX").unwrap_or(name);
    // A scalar SSE access is the scalar's size: `MOVSDrm` 8, `MOVSSmr` 4.
    if sse(name.strip_suffix("_alt").unwrap_or(name)).is_some() {
        // A conversion reads its source's size: the float named first, or the
        // integer.
        for (prefix, bytes) in
            [("CVTTSD2SI", 8), ("CVTTSS2SI", 4), ("CVTSD2SS", 8), ("CVTSS2SD", 4), ("CVTSI642", 8), ("CVTSI2", 4)]
        {
            if name.starts_with(prefix) {
                return Some(bytes);
            }
        }
        let upper: String = name.chars().take_while(char::is_ascii_uppercase).collect();
        return if upper.ends_with("SD") {
            Some(8)
        } else if upper.ends_with("SS") {
            Some(4)
        } else {
            // `ANDPSrm`: the whole register.
            Some(16)
        };
    }
    let (mnemonic, width, _) = split(name)?;
    let bits = if matches!(mnemonic.as_str(), "movzx" | "movsx") {
        let rest = name.trim_start_matches(|c: char| c.is_ascii_uppercase());
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        let size: String =
            rest.chars().skip_while(|c| c.is_ascii_lowercase()).take_while(|c| c.is_ascii_digit()).collect();
        size.parse().unwrap_or(8)
    } else {
        width?
    };
    Some(bits.div_ceil(8))
}

/// `ADD32ri8` -> (`add`, 32, `ri`); `SHL32rCL` -> (`shl`, 32, `rr`);
/// `MOVZX32rr8` -> (`movzx`, 32, `rr`).
fn split(name: &str) -> Option<(String, Option<u32>, Vec<char>)> {
    let upper = name.find(|c: char| !c.is_ascii_uppercase())?;
    let mnemonic = name[..upper].to_ascii_lowercase();
    let rest = &name[upper..];
    let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let width = (digits > 0).then(|| rest[..digits].parse().ok()).flatten();
    let mut letters = Vec::new();
    let mut tail = &rest[digits..];
    while let Some(c) = tail.chars().next() {
        match c {
            'r' | 'm' | 'i' => letters.push(c),
            'C' if tail[1..].starts_with('L') => {
                letters.push('r');
                tail = &tail[1..];
            }
            // A lone `1` is the shift-by-one immediate (`SHL32r1`); a digit run
            // (`rr16`, `ri8`) is a size.
            '1' if !tail[1..].starts_with(|d: char| d.is_ascii_digit()) => letters.push('i'),
            '0'..='9' => {}
            _ => return None,
        }
        tail = &tail[c.len_utf8()..];
    }
    (!letters.is_empty() || width.is_none()).then_some((mnemonic, width, letters))
}

/// Whether `form`'s destination and untied sources admit `letters` (the
/// destination first).
fn admits(
    form: &Form,
    letters: &[char],
) -> bool {
    let untied: Vec<_> = form.sources.iter().filter(|s| s.tied.is_none()).collect();
    if letters.len() != form.dests.len() + untied.len() {
        return false;
    }
    let (dests, sources) = letters.split_at(form.dests.len());
    // `r` also names an xmm register in the pin's names (`ADDSDrr`).
    let allows = |l: char, o: &llrm_target::instr::Operand| o.allows(l) || l == 'r' && o.allows('x');
    dests.iter().zip(&form.dests).all(|(&l, d)| allows(l, d)) && sources.iter().zip(&untied).all(|(&l, s)| allows(l, s))
}
