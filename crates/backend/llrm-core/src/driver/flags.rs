//! gcc and clang's optimization and code generation options, which every
//! frontend accepts: parsed and mapped here, once.

use std::path::PathBuf;

use llrm_transforms::inline::Threshold;
use llrm_transforms::pipeline;

use crate::abi::machine::Machine;
use crate::model::passes;

/// The options' usage line, for a frontend's own.
pub const USAGE: &str = "[-O0|-O1|-O2|-O3|-Os|-Oz|-Og] [-f[no-]PASS] [-march=CPU] [-mtune=CPU] [-m[no-]stack-is-data] [--cpu CPU] [--machine MACHINE] [-o OUTPUT] [-S]";

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

    pub fn name(self) -> &'static str {
        match self {
            Self::O0 => "O0",
            Self::O1 => "O1",
            Self::O2 => "O2",
            Self::O3 => "O3",
            Self::Os => "Os",
            Self::Oz => "Oz",
            Self::Og => "Og",
        }
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
    ("inline-functions", |options, on| options.inline = if !on { Threshold(0) } else if options.inline.0 == 0 { Threshold::default() } else { options.inline }),
    ("strength-reduce", |options, on| options.strength = on),
    ("unswitch-loops", |options, on| options.unswitch = on),
    ("gcse", |options, on| (options.forward, options.drop_loads) = (on, on)),
    ("tree-dse", |options, on| options.drop_stores = on),
    ("tree-dce", |options, on| options.dead = on),
    ("tree-sra", |options, on| options.promote = on),
    ("move-loop-invariants", |options, on| options.hoist = on),
    ("tree-loop-distribute-patterns", |options, on| options.fill = on),
];

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
    pub output: Option<PathBuf>,
    /// `-S`: assembly rather than an object.
    pub assembly: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self { level: Level::O2, passes: Vec::new(), cpu: None, machine: None, stack_is_data: None, output: None, assembly: false }
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
            "-mstack-is-data" => self.stack_is_data = Some(true),
            "-mno-stack-is-data" => self.stack_is_data = Some(false),
            "--cpu" => self.cpu = Some(value("--cpu")?),
            "--machine" => self.machine = Some(PathBuf::from(value("--machine")?)),
            _ if flag.starts_with("-O") => self.level = Level::parse(&flag[2..])?,
            _ if flag.starts_with("-march=") || flag.starts_with("-mtune=") => {
                let (option, name) = flag.split_once('=').expect("an =");
                let cpu = CPUS.iter().find(|(gcc, _)| *gcc == name).ok_or_else(|| format!("unknown {option}={name}; choose i386, i486 or pentium"))?;
                self.cpu = Some(cpu.1.to_owned());
            }
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

    /// The old MIR's options, for `--legacy`.
    pub fn legacy(&self) -> passes::Options {
        let options = self.pipeline();
        let on = |pass: bool| options.optimize && pass;
        passes::Options {
            level: self.level.name().to_owned(),
            max_unroll_iterations: options.limits.max_unroll_iterations,
            max_unrolled_operations: options.limits.max_unrolled_operations,
            grows: options.limits.grows,
            lcssa: on(options.lcssa),
            floatloop: on(options.floatloop),
            fold: on(options.fold),
            decide: on(options.decide),
            dead: on(options.dead),
            hoist: on(options.hoist),
            forward: on(options.forward),
            drop_loads: on(options.drop_loads),
            drop_stores: on(options.drop_stores),
            promote: on(options.promote),
            strength: on(options.strength),
            unroll: on(options.unroll),
            peel: on(options.peel),
            fill: on(options.fill),
            unswitch: on(options.unswitch),
        }
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
        Ok(machine)
    }

    /// The driver's options for `machine`.
    pub fn driver(&self, machine: Machine) -> super::Options {
        super::Options { pipeline: self.pipeline(), ..super::Options::of(machine) }
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;
