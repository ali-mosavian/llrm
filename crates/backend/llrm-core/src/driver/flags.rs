//! gcc and clang's optimization and code generation options, which every
//! frontend accepts: parsed and mapped here, once.

use std::path::PathBuf;

use llrm_target::object::Format;
use llrm_transforms::inline::Threshold;
use llrm_transforms::pipeline;

use crate::abi::machine::Machine;

/// The options' usage line, for a frontend's own.
pub const USAGE: &str = "[-O0|-O1|-O2|-O3|-Omax|-Os|-Oz|-Og] [-f[no-]PASS] [-f[no-]sanitize=CHECKS] [-f[no-]trapv] [-f[no-]wrapv] [-m16|-m32|-m64] [-march=CPU] [-mtune=CPU] [-mabi=ABI] [-m[no-]stack-is-data] [-m[no-]far-bss] [--clocks-per-byte N] [--machine MACHINE] [-fstack-usage] [-Wstack-usage=N] [-fobject-format=omf|elf|macho|coff] [-g] [-o OUTPUT] [-S]";

/// An `-O` level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    /// Every pass on, every budget as the compiler has them: what -O3 was
    /// before it meant gcc's.
    Omax,
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
            "max" => Self::Omax,
            "s" => Self::Os,
            "z" => Self::Oz,
            "g" => Self::Og,
            _ => {
                return Err(format!(
                    "unknown optimization level -O{text}; choose -O0, -O1, -O2, -O3, -Omax, -Os, -Oz or -Og"
                ));
            }
        })
    }

    pub fn options(self) -> pipeline::Options {
        match self {
            Self::O0 => pipeline::Options::none(),
            Self::O1 | Self::Og => pipeline::Options::basic(),
            Self::O2 => pipeline::Options::standard(),
            Self::O3 => pipeline::Options::speed(),
            Self::Omax => pipeline::Options::aggressive(),
            Self::Os => pipeline::Options::size(),
            Self::Oz => pipeline::Options::min_size(),
        }
    }
}

/// gcc's `-f` pass names, each with the options it sets.
const PASSES: [(&str, fn(&mut pipeline::Options, bool)); 20] = [
    ("allocation-search", |options, on| options.search = on),
    ("allocation-routes", |options, on| options.routes = on),
    ("allocation-search-all", |options, on| options.exhaustive = on),
    ("unroll-loops", |options, on| options.unroll = on),
    ("peel-loops", |options, on| options.peel = on),
    ("inline-functions-called-once", |options, on| options.inline.last = on),
    ("inline-functions", |options, on| {
        options.inline = if !on {
            Threshold { limit: 0, ..options.inline }
        } else if options.inline.limit == 0 {
            Threshold { limit: Threshold::default().limit, ..options.inline }
        } else {
            options.inline
        }
    }),
    ("strength-reduce", |options, on| options.strength = on),
    ("unswitch-loops", |options, on| options.unswitch = on),
    ("tree-ch", |options, on| options.copy_headers = on),
    ("ipa-cp-clone", |options, on| options.inline.cp_clone = on),
    ("ipa-vrp", |options, on| options.ipa_ranges = on),
    ("gcse", |options, on| (options.forward, options.drop_loads) = (on, on)),
    ("gvn-dataflow", |options, on| options.gvn_dataflow = on),
    ("tree-dse", |options, on| options.drop_stores = on),
    ("tree-dce", |options, on| options.dead = on),
    ("tree-sra", |options, on| options.promote = on),
    ("move-loop-invariants", |options, on| options.hoist = on),
    ("tree-loop-distribute-patterns", |options, on| options.fill = on),
    ("optimize-sibling-calls", |options, on| options.sibcalls = on),
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
    /// `stack`: each procedure's entry compares SP with the runtime's limit.
    /// Not part of `undefined`, as gcc's `-fstack-check` is not.
    pub stack: bool,
}

impl Sanitize {
    /// Each check `list` names, `undefined` all of them, set to `on`.
    fn set(
        &mut self,
        list: &str,
        on: bool,
    ) -> Result<(), String> {
        for name in list.split(',') {
            match name {
                "bounds" => self.bounds = on,
                "integer-divide-by-zero" => self.integer_divide_by_zero = on,
                "signed-integer-overflow" => self.signed_integer_overflow = on,
                "stack" => self.stack = on,
                "undefined" => {
                    *self =
                        Self { bounds: on, integer_divide_by_zero: on, signed_integer_overflow: on, stack: self.stack }
                }
                _ => {
                    return Err(format!(
                        "unknown sanitizer {name}; choose bounds, integer-divide-by-zero, signed-integer-overflow, stack or undefined"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The options, as given.
#[derive(Clone, Debug)]
pub struct Flags {
    pub level: Level,
    /// Each `-f` as (index into `PASSES`, on), in order. gcc applies them
    /// over the level wherever they stand.
    passes: Vec<(usize, bool)>,
    /// `-march=`: the CPU, by the name the target's `timings.times` gives
    /// gcc's.
    march: Option<String>,
    /// `-mtune=`: the CPU the code is priced for, where that is not the
    /// `-march` one.
    mtune: Option<String>,
    machine: Option<PathBuf>,
    /// `-m16`, `-m32`, `-m64`: the target to build for, by the number its
    /// description gives.
    mode: Option<u32>,
    /// `-m[no-]stack-is-data`: whether the stack lives in the data group.
    stack_is_data: Option<bool>,
    far_bss: Option<bool>,
    /// `--clocks-per-byte N`: the clocks an inline must save for each byte of
    /// code it adds (default 16; 0 allows no growth).
    milliclocks_per_byte: Option<i64>,
    pub output: Option<PathBuf>,
    /// `-S`: assembly rather than an object.
    pub assembly: bool,
    pub sanitize: Sanitize,
    /// `-fwrapv`: signed arithmetic wraps, so a front end states no no-overflow
    /// promise of it.
    pub wrapv: bool,
    /// `-g`: debug information, in the object format's own format unless
    /// `-gcodeview`, `-gdwarf[-N]` or `-gtd` says which.
    pub debug: bool,
    pub debug_format: llrm_object::debug::Format,
    /// `-fobject-format=`: the object format to write, where the target has
    /// more than its default.
    pub object_format: Option<Format>,
    /// `-mabi=`: the ABI an unmarked function has, by the family name the
    /// target's `calling.toml` gives; its default without.
    pub abi: Option<String>,
    /// `-fstack-usage`.
    pub stack_usage: bool,
    /// `-Wstack-usage=N`.
    pub stack_limit: Option<i64>,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            wrapv: false,
            level: Level::O2,
            passes: Vec::new(),
            march: None,
            mtune: None,
            machine: None,
            mode: None,
            stack_is_data: None,
            far_bss: None,
            milliclocks_per_byte: None,
            output: None,
            assembly: false,
            sanitize: Sanitize::default(),
            debug: false,
            abi: None,
            debug_format: Default::default(),
            object_format: None,
            stack_usage: false,
            stack_limit: None,
        }
    }
}

impl Flags {
    /// Takes `argv[*at]`, and its value, when it is one of these options,
    /// leaving `*at` on the last argument taken. False leaves it to the
    /// frontend.
    pub fn take(
        &mut self,
        argv: &[String],
        at: &mut usize,
    ) -> Result<bool, String> {
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
            "-g" => (self.debug, self.debug_format) = (true, Default::default()),
            "-g0" => self.debug = false,
            "-gcodeview" => (self.debug, self.debug_format) = (true, llrm_object::debug::Format::CodeView),
            "-gdwarf" | "-gdwarf-5" => {
                (self.debug, self.debug_format) = (true, llrm_object::debug::Format::Dwarf { version: 5 })
            }
            "-gdwarf-4" => (self.debug, self.debug_format) = (true, llrm_object::debug::Format::Dwarf { version: 4 }),
            "-gtd" => (self.debug, self.debug_format) = (true, llrm_object::debug::Format::TurboDebugger),
            "-mstack-is-data" => self.stack_is_data = Some(true),
            "-mno-stack-is-data" => self.stack_is_data = Some(false),
            "-mfar-bss" => self.far_bss = Some(true),
            "-mno-far-bss" => self.far_bss = Some(false),
            "--clocks-per-byte" => {
                let text = value("--clocks-per-byte")?;
                let clocks = text.parse::<f64>().ok().filter(|clocks| clocks.is_finite() && *clocks >= 0.0);
                self.milliclocks_per_byte = Some(
                    (clocks.ok_or_else(|| format!("--clocks-per-byte {text}: expected a number of clocks"))? * 1000.0)
                        .round() as i64,
                );
            }
            "--machine" => self.machine = Some(PathBuf::from(value("--machine")?)),
            _ if flag.starts_with("-O") => self.level = Level::parse(&flag[2..])?,
            _ if Self::mode_flag(flag).is_some() => self.mode = Self::mode_flag(flag),
            _ if flag.starts_with("-march=") => self.march = Some(flag["-march=".len()..].to_owned()),
            _ if flag.starts_with("-mtune=") => self.mtune = Some(flag["-mtune=".len()..].to_owned()),
            "-fstack-usage" => self.stack_usage = true,
            _ if flag.starts_with("-Wstack-usage=") => {
                let limit = &flag["-Wstack-usage=".len()..];
                self.stack_limit =
                    Some(limit.parse().map_err(|_| format!("-Wstack-usage={limit}: expected a number of bytes"))?);
            }
            _ if flag.starts_with("-mabi=") => self.abi = Some(flag["-mabi=".len()..].to_owned()),
            _ if flag.starts_with("-fobject-format=") => {
                self.object_format =
                    Some(Format::parse(&flag["-fobject-format=".len()..]).map_err(|error| format!("{flag}: {error}"))?)
            }
            "-fwrapv" | "-fno-wrapv" => self.wrapv = flag == "-fwrapv",
            "-ftrapv" | "-fno-trapv" => self.sanitize.signed_integer_overflow = flag == "-ftrapv",
            _ if flag.starts_with("-fsanitize=") => self.sanitize.set(&flag["-fsanitize=".len()..], true)?,
            _ if flag.starts_with("-fno-sanitize=") => self.sanitize.set(&flag["-fno-sanitize=".len()..], false)?,
            _ if flag.starts_with("-f") => {
                let (name, on) = match flag[2..].strip_prefix("no-") {
                    Some(name) => (name, false),
                    None => (&flag[2..], true),
                };
                let known = PASSES
                    .iter()
                    .position(|(one, _)| *one == name)
                    .ok_or_else(
                        || format!(
                            "unknown option {flag}; the passes are {}",
                            PASSES.map(|(one, _)| format!("-f{one}")).join(", ")
                        ),
                    )?;
                self.passes.push((known, on));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The convention an unmarked function has on `target`: the `-mabi=`
    /// family's, else the target's default.
    pub fn convention(
        &self,
        target: &dyn llrm_target::Target,
    ) -> Result<&'static llrm_target::calling::Convention, String> {
        target
            .calling()
            .chosen(self.abi.as_deref())
            .map_err(|error| format!("-mabi={}: {error}", self.abi.as_deref().unwrap_or_default()))
    }

    /// The object format to write for `target`: `-fobject-format=`, else the
    /// target's default.
    pub fn format(
        &self,
        target: &dyn llrm_target::Target,
    ) -> Result<Format, String> {
        target.object().choose(target.name(), self.object_format)
    }

    /// The number `-m16`, `-m32`, `-m64` give: the one parser of the flag.
    pub fn mode_flag(argument: &str) -> Option<u32> {
        let digits = argument.strip_prefix("-m")?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }

    /// The `-m` number given, if any: the target whose description names it.
    pub fn mode(&self) -> Option<u32> {
        self.mode
    }

    /// The pipeline's options: the level's, then each `-f`.
    pub fn pipeline(&self) -> pipeline::Options {
        let mut options = self.level.options();
        for &(pass, on) in &self.passes {
            PASSES[pass].1(&mut options, on);
        }
        if let Some(clocks) = self.milliclocks_per_byte {
            options.limits.milliclocks_per_byte = clocks;
        }
        options
    }

    /// The CPU `-mtune`, else `-march`, names for `target`, if either is given.
    fn cpu(
        &self,
        target: &dyn llrm_target::Target,
    ) -> Result<Option<String>, String> {
        let (option, name) = match (&self.mtune, &self.march) {
            (Some(name), _) => ("-mtune", name),
            (None, Some(name)) => ("-march", name),
            _ => return Ok(None),
        };
        match target.march(name) {
            Some(cpu) => Ok(Some(cpu.to_owned())),
            None => Err(format!("unknown {option}={name}; {} has {}", target.name(), target.marches().join(", "))),
        }
    }

    /// `default`, or the `--machine` description, on the CPU `-march` or
    /// `-mtune` names for `target`.
    pub fn machine(
        &self,
        target: &dyn llrm_target::Target,
        default: Machine,
    ) -> Result<Machine, String> {
        let mut machine = match &self.machine {
            Some(path) => Machine { layout: default.layout.clone(), ..Machine::load(path, &default.cpu)? },
            None => default,
        };
        if let Some(cpu) = self.cpu(target)? {
            machine.cpu = cpu;
        }
        if let Some(stack_is_data) = self.stack_is_data {
            machine.segments.as_mut().ok_or("-mstack-is-data needs a segmented machine")?.stack_is_data = stack_is_data;
        }
        if let Some(far_bss) = self.far_bss {
            machine.far_bss = far_bss;
        }
        Ok(machine)
    }

    /// The driver's options for `machine`.
    pub fn driver(
        &self,
        machine: Machine,
        arch: std::rc::Rc<dyn llrm_target::Target>,
        selection: &'static crate::backend::isel::Compiled,
    ) -> super::Options {
        super::Options {
            debug_format: self.debug_format,
            pipeline: self.pipeline(),
            stack_usage: self.stack_usage,
            stack_limit: self.stack_limit,
            abi: self.abi.clone(),
            ..super::Options::new(machine, arch, selection)
        }
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;
