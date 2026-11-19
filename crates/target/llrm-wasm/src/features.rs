//! `WebAssemblySubtarget`'s feature bits, from a function's `"target-cpu"` and
//! `"target-features"` attributes
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssembly.td)).
//! Each function has its own, as `getSubtargetImpl(const Function &)` gives it
//! one.

use llrm_mir::Attribute;
use llrm_mir::module::Function;

/// The features of `WebAssembly.td`, sorted by name; the second is what
/// enabling one enables with it (`SubtargetFeature`'s implied list).
const FEATURES: [(&str, &[&str]); 19] = [
    ("atomics", &[]),
    ("bulk-memory", &["bulk-memory-opt"]),
    ("bulk-memory-opt", &[]),
    ("call-indirect-overlong", &[]),
    ("exception-handling", &[]),
    ("extended-const", &[]),
    ("fp16", &[]),
    ("gc", &[]),
    ("multimemory", &[]),
    ("multivalue", &[]),
    ("mutable-globals", &[]),
    ("nontrapping-fptoint", &[]),
    ("reference-types", &["call-indirect-overlong"]),
    ("relaxed-atomics", &[]),
    ("relaxed-simd", &[]),
    ("sign-ext", &[]),
    ("simd128", &[]),
    ("tail-call", &[]),
    ("wide-arithmetic", &[]),
];

/// The `ProcessorModel`s.
const CPUS: [(&str, &[&str]); 4] = [
    ("mvp", &[]),
    (
        "generic",
        &[
            "bulk-memory",
            "bulk-memory-opt",
            "call-indirect-overlong",
            "multivalue",
            "mutable-globals",
            "nontrapping-fptoint",
            "reference-types",
            "sign-ext",
        ],
    ),
    (
        "lime1",
        &[
            "bulk-memory-opt",
            "call-indirect-overlong",
            "extended-const",
            "multivalue",
            "mutable-globals",
            "nontrapping-fptoint",
            "sign-ext",
        ],
    ),
    (
        "bleeding-edge",
        &[
            "atomics",
            "bulk-memory",
            "bulk-memory-opt",
            "call-indirect-overlong",
            "exception-handling",
            "extended-const",
            "fp16",
            "gc",
            "multimemory",
            "multivalue",
            "mutable-globals",
            "nontrapping-fptoint",
            "relaxed-simd",
            "reference-types",
            "simd128",
            "sign-ext",
            "tail-call",
        ],
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features(u32);

impl Default for Features {
    /// `generic`, the CPU `llc` takes with none named.
    fn default() -> Features {
        Features::of_cpu("generic").expect("the generic CPU")
    }
}

fn bit(name: &str) -> Option<u32> {
    FEATURES.iter().position(|(feature, _)| *feature == name).map(|at| 1 << at)
}

impl Features {
    fn of_cpu(name: &str) -> Result<Features, String> {
        let (_, features) = CPUS.iter().find(|(cpu, _)| *cpu == name).ok_or_else(|| format!("unknown CPU {name:?}"))?;
        Ok(features.iter().fold(Features(0), |set, feature| set.with(feature, true).expect("a feature")))
    }

    /// `FeatureBitset::set` and `ClearImpliedBits`: enabling a feature
    /// enables what it implies, disabling one disables what implies it.
    pub fn with(
        self,
        name: &str,
        on: bool,
    ) -> Result<Features, String> {
        let this = bit(name).ok_or_else(|| format!("{name:?} is not a recognized feature"))?;
        let mut set = self.0;
        if on {
            set |= this;
            for implied in FEATURES[this.trailing_zeros() as usize].1 {
                set = Features(set).with(implied, true)?.0;
            }
        } else {
            set &= !this;
            for (feature, implies) in FEATURES {
                if implies.contains(&name) {
                    set = Features(set).with(feature, false)?.0;
                }
            }
        }
        Ok(Features(set))
    }

    /// The subtarget of a function: its CPU (`generic` if it names none), then
    /// its feature list in order.
    pub fn of(function: &Function) -> Result<Features, String> {
        let value = |key: &str| {
            function
                .attrs
                .iter()
                .find_map(
                    |attr| match attr {
                        Attribute::Str(name, Some(value)) if name == key => Some(value.as_str()),
                        _ => None,
                    },
                )
        };
        let mut set = Features::of_cpu(value("target-cpu").filter(|cpu| !cpu.is_empty()).unwrap_or("generic"))?;
        for feature in value("target-features").unwrap_or("").split(',').filter(|feature| !feature.is_empty()) {
            let (on, name) = match feature.split_at(1) {
                ("+", name) => (true, name),
                ("-", name) => (false, name),
                _ => return Err(format!("target feature {feature:?} has no + or -")),
            };
            set = set.with(name, on)?;
        }
        Ok(set)
    }

    pub fn has(
        self,
        name: &str,
    ) -> bool {
        bit(name).is_some_and(|bit| self.0 & bit != 0)
    }

    /// The enabled features, sorted by name.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        FEATURES.iter().filter(move |(name, _)| self.has(name)).map(|(name, _)| *name)
    }
}
