//! gcc and clang's optimization and code generation options, which every
//! frontend accepts: parsed and mapped here, once.

use std::path::PathBuf;

use llrm_transforms::inline::Threshold;
use llrm_transforms::pipeline;

use crate::abi::machine::Machine;

/// The options' usage line, for a frontend's own.
pub const USAGE: &str = "[-O0|-O1|-O2|-O3|-Os|-Oz|-Og] [-f[no-]PASS] [-f[no-]sanitize=CHECKS] [-f[no-]trapv] [-march=CPU] [-mtune=CPU] [-m[no-]stack-is-data] [-m[no-]far-bss] [--cpu CPU] [--machine MACHINE] [-fstack-usage] [-Wstack-usage=N] [-g] [-o OUTPUT] [-S]";

/// An `-O` level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    Os,
    Oz,
    /// gcc's -Og, which is -O1 here: no pass removes what a debugger reads.
    Og,
}

impl Level {
    /// The level `-O<text>` names; a bare `-O` is -O1, as in gcc.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "0" => Self::O0,
            "" | "1" => Self::O1,
            "2" => Self::O2,
            "3" => Self::O3,
            "s" => Self::Os,
            "z" => Self::Oz,
            "g" => Self::Og,
            _ => return Err(format!("unknown optimization level -O{text}; choose -O0, -O1, -O2, -O3, -Os, -Oz or -Og")),
        })
    }

    pub fn options(self) -> pipeline::Options {
        match self {
            Self::O0 => pipeline::Options::none(),
            Self::O1 | Self::Og => pipeline::Options::basic(),
            Self::O2 => pipeline::Options::default(),
            Self::O3 => pipeline::Options::aggressive(),
            Self::Os => pipeline::Options::size(),
            Self::Oz => pipeline::Options::min_size(),
        }
    }
}

/// gcc's `-f` pass names, each with the options it sets.
const PASSES: [(&str, fn(&mut pipeline::Options, bool)); 11] = [
    ("unroll-loops", |options, on| options.unroll = on),
    ("peel-loops", |options, on| options.peel = on),
    ("inline-functions", |options, on| options.inline = if !on { Threshold::new(0) } else if options.inline.limit == 0 { Threshold { limit: Threshold::default().limit, ..options.inline } } else { options.inline }),
    ("strength-reduce", |options, on| options.strength = on),
    ("unswitch-loops", |options, on| options.unswitch = on),
    ("gcse", |options, on| (options.forward, options.drop_loads) = (on, on)),
    ("tree-dse", |options, on| options.drop_stores = on),
    ("tree-dce", |options, on| options.dead = on),
    ("tree-sra", |options, on| options.promote = on),
    ("move-loop-invariants", |options, on| options.hoist = on),
    ("tree-loop-distribute-patterns", |options, on| options.fill = on),
];

/// The run-time checks `-fsanitize` names, gcc's: what BC's /D checks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sanitize {
    /// `bounds`: array subscripts.
    pub bounds: bool,
    /// `integer-divide-by-zero`: integer division, where the processor
    /// would trap.
    pub integer_divide_by_zero: bool,
    /// `signed-integer-overflow`, or `-ftrapv`: integer arithmetic and
    /// narrowing.
    pub signed_integer_overflow: bool,
}

impl Sanitize {
    /// Each check `list` names, `undefined` all of them, set to `on`.
    fn set(&mut self, list: &str, on: bool) -> Result<(), String> {
        for name in list.split(',') {
            match name {
                "bounds" => self.bounds = on,
                "integer-divide-by-zero" => self.integer_divide_by_zero = on,
                "signed-integer-overflow" => self.signed_integer_overflow = on,
                "undefined" => *self = Self { bounds: on, integer_divide_by_zero: on, signed_integer_overflow: on },
                _ => return Err(format!("unknown sanitizer {name}; choose bounds, integer-divide-by-zero, signed-integer-overflow or undefined")),
            }
        }
        Ok(())
    }
}

/// gcc's `-march`/`-mtune` names for the CPUs priced.
const CPUS: [(&str, &str); 3] = [("i386", "386"), ("i486", "486"), ("pentium", "P5")];

/// The options, as given.
#[derive(Clone, Debug)]
pub struct Flags {
    pub level: Level,
    /// Each `-f` as (index into `PASSES`, on), in order. gcc applies them
    /// over the level wherever they stand.
    passes: Vec<(usize, bool)>,
    cpu: Option<String>,
    machine: Option<PathBuf>,
    /// `-m[no-]stack-is-data`: whether the stack lives in the data group.
    stack_is_data: Option<bool>,
    far_bss: Option<bool>,
    pub output: Option<PathBuf>,
    /// `-S`: assembly rather than an object.
    pub assembly: bool,
    pub sanitize: Sanitize,
    /// `-g`: CodeView debug information.
    pub debug: bool,
    /// `-fstack-usage`.
    pub stack_usage: bool,
    /// `-Wstack-usage=N`.
    pub stack_limit: Option<i64>,
}

impl Default for Flags {
    fn default() -> Self {
        Self { level: Level::O2, passes: Vec::new(), cpu: None, machine: None, stack_is_data: None, far_bss: None, output: None, assembly: false, sanitize: Sanitize::default(), debug: false, stack_usage: false, stack_limit: None }
    }
}

impl Flags {
    /// Takes `argv[*at]`, and its value, when it is one of these options,
    /// leaving `*at` on the last argument taken. False leaves it to the
    /// frontend.
    pub fn take(&mut self, argv: &[String], at: &mut usize) -> Result<bool, String> {
        let argument = argv[*at].as_str();
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value)),
            _ => (argument, None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            if let Some(value) = inline {
                return Ok(value.to_owned());
            }
            *at += 1;
            argv.get(*at).cloned().ok_or_else(|| format!("argument {name}: expected one argument"))
        };
        match flag {
            "-o" | "--output" => self.output = Some(PathBuf::from(value("-o/--output")?)),
            "-S" => self.assembly = true,
            "-g" => self.debug = true,
            "-g0" => self.debug = false,
            "-mstack-is-data" => self.stack_is_data = Some(true),
            "-mno-stack-is-data" => self.stack_is_data = Some(false),
            "-mfar-bss" => self.far_bss = Some(true),
            "-mno-far-bss" => self.far_bss = Some(false),
            "--cpu" => self.cpu = Some(value("--cpu")?),
            "--machine" => self.machine = Some(PathBuf::from(value("--machine")?)),
            _ if flag.starts_with("-O") => self.level = Level::parse(&flag[2..])?,
            _ if flag.starts_with("-march=") || flag.starts_with("-mtune=") => {
                let (option, name) = flag.split_once('=').expect("an =");
                let cpu = CPUS.iter().find(|(gcc, _)| *gcc == name).ok_or_else(|| format!("unknown {option}={name}; choose i386, i486 or pentium"))?;
                self.cpu = Some(cpu.1.to_owned());
            }
            "-fstack-usage" => self.stack_usage = true,
            _ if flag.starts_with("-Wstack-usage=") => {
                let limit = &flag["-Wstack-usage=".len()..];
                self.stack_limit = Some(limit.parse().map_err(|_| format!("-Wstack-usage={limit}: expected a number of bytes"))?);
            }
            "-ftrapv" | "-fno-trapv" => self.sanitize.signed_integer_overflow = flag == "-ftrapv",
            _ if flag.starts_with("-fsanitize=") => self.sanitize.set(&flag["-fsanitize=".len()..], true)?,
            _ if flag.starts_with("-fno-sanitize=") => self.sanitize.set(&flag["-fno-sanitize=".len()..], false)?,
            _ if flag.starts_with("-f") => {
                let (name, on) = match flag[2..].strip_prefix("no-") {
                    Some(name) => (name, false),
                    None => (&flag[2..], true),
                };
                let known = PASSES.iter().position(|(one, _)| *one == name).ok_or_else(|| {
                    format!("unknown option {flag}; the passes are {}", PASSES.map(|(one, _)| format!("-f{one}")).join(", "))
                })?;
                self.passes.push((known, on));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The pipeline's options: the level's, then each `-f`.
    pub fn pipeline(&self) -> pipeline::Options {
        let mut options = self.level.options();
        for &(pass, on) in &self.passes {
            PASSES[pass].1(&mut options, on);
        }
        options
    }

    /// `default`, or the `--machine` description, on the CPU named.
    pub fn machine(&self, default: Machine) -> Result<Machine, String> {
        let mut machine = match &self.machine {
            Some(path) => Machine::load(path)?,
            None => default,
        };
        if let Some(cpu) = &self.cpu {
            machine.cpu = cpu.clone();
        }
        if let Some(stack_is_data) = self.stack_is_data {
            machine.segments.stack_is_data = stack_is_data;
        }
        if let Some(far_bss) = self.far_bss {
            machine.far_bss = far_bss;
        }
        Ok(machine)
    }

    /// The driver's options for `machine`.
    pub fn driver(&self, machine: Machine) -> super::Options {
        super::Options { pipeline: self.pipeline(), stack_usage: self.stack_usage, stack_limit: self.stack_limit, ..super::Options::of(machine) }
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;
