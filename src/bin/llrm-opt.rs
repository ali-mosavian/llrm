use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let invocation = match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("llrm-opt: {message}");
            return ExitCode::from(2);
        }
    };
    invocation.run()
}

struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
    passes: Vec<PassName>,
    verify_each: bool,
}

impl Invocation {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut input = None;
        let mut output = None;
        let mut passes = None;
        let mut verify_each = false;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => {
                    let path = arguments.next().ok_or("-o requires a path")?;
                    if output.replace(PathBuf::from(path)).is_some() {
                        return Err("-o may only be specified once".into());
                    }
                }
                "--passes" => {
                    let value = arguments.next().ok_or("--passes requires a value")?;
                    if passes.replace(parse_passes(&value)?).is_some() {
                        return Err("--passes may only be specified once".into());
                    }
                }
                "--verify-each" => verify_each = true,
                _ if argument.starts_with('-') => return Err(usage().into()),
                _ if input.is_none() => input = Some(PathBuf::from(argument)),
                _ => return Err(usage().into()),
            }
        }
        Ok(Self {
            input: input.ok_or_else(|| usage().to_owned())?,
            output,
            passes: passes.unwrap_or_default(),
            verify_each,
        })
    }

    fn run(self) -> ExitCode {
        let source = match fs::read_to_string(&self.input) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let output = match run_pipeline(&source, &self.passes, self.verify_each) {
            Ok(output) => output,
            Err(PipelineError::Text(error)) => {
                return failure(format!(
                    "{}:{}:{}: {}",
                    self.input.display(),
                    error.line,
                    error.column,
                    error.message
                ));
            }
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        write_output(self.output.as_deref(), output.as_bytes())
    }
}

#[derive(Clone, Copy)]
enum PassName {
    AlgebraicSimplify,
    CommonSubexpressionElimination,
    ConstantFold,
    DeadCodeElimination,
    DeadStoreElimination,
    SimplifyBranches,
    UnreachableBlockElimination,
}

fn parse_passes(value: &str) -> Result<Vec<PassName>, String> {
    if value.is_empty() {
        return Err("--passes requires a nonempty comma-separated pipeline".into());
    }
    value
        .split(',')
        .map(|name| match name {
            "algebraic-simplify" => Ok(PassName::AlgebraicSimplify),
            "common-subexpression-elimination" => {
                Ok(PassName::CommonSubexpressionElimination)
            }
            "constant-fold" => Ok(PassName::ConstantFold),
            "dead-code-elimination" => Ok(PassName::DeadCodeElimination),
            "dead-store-elimination" => Ok(PassName::DeadStoreElimination),
            "simplify-branches" => Ok(PassName::SimplifyBranches),
            "unreachable-block-elimination" => Ok(PassName::UnreachableBlockElimination),
            _ => Err(format!("unknown pass `{name}`")),
        })
        .collect()
}

#[derive(Debug)]
enum PipelineError {
    Algebraic(llrm::transforms::AlgebraicError),
    Text(llrm::ir::TextError),
    BranchSimplify(llrm::transforms::BranchSimplifyError),
    Fold(llrm::transforms::FoldError),
    Pass(llrm::transforms::PassError),
    Verification(Vec<llrm::support::diagnostic::Diagnostic>),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Algebraic(error) => error.fmt(formatter),
            Self::Text(error) => error.fmt(formatter),
            Self::BranchSimplify(error) => error.fmt(formatter),
            Self::Fold(error) => error.fmt(formatter),
            Self::Pass(error) => error.fmt(formatter),
            Self::Verification(diagnostics) => write!(
                formatter,
                "pass pipeline produced invalid IR with {} diagnostic(s)",
                diagnostics.len()
            ),
        }
    }
}

fn run_pipeline(
    source: &str,
    passes: &[PassName],
    verify_each: bool,
) -> Result<String, PipelineError> {
    let mut module = llrm::ir::parse_text(source).map_err(PipelineError::Text)?;
    let mut manager = llrm::transforms::FunctionPassManager::new();
    manager.set_verify_each(verify_each);
    for pass in passes {
        match pass {
            PassName::AlgebraicSimplify => manager.add_pass(
                llrm::transforms::AlgebraicSimplify::new(&module)
                    .map_err(PipelineError::Algebraic)?,
            ),
            PassName::CommonSubexpressionElimination => manager.add_pass(
                llrm::transforms::CommonSubexpressionElimination::new(),
            ),
            PassName::ConstantFold => manager.add_pass(
                llrm::transforms::ConstantFold::new(&module).map_err(PipelineError::Fold)?,
            ),
            PassName::DeadCodeElimination => {
                manager.add_pass(llrm::transforms::DeadCodeElimination::new())
            }
            PassName::DeadStoreElimination => {
                manager.add_pass(llrm::transforms::DeadStoreElimination::new())
            }
            PassName::SimplifyBranches => manager.add_pass(
                llrm::transforms::SimplifyBranches::new(&module)
                    .map_err(PipelineError::BranchSimplify)?,
            ),
            PassName::UnreachableBlockElimination => {
                manager.add_pass(llrm::transforms::UnreachableBlockElimination::new())
            }
        }
    }
    manager.run(&mut module).map_err(PipelineError::Pass)?;
    module.verify().map_err(PipelineError::Verification)?;
    Ok(llrm::ir::write_text(&module))
}

fn canonicalize(source: &str) -> Result<String, llrm::ir::TextError> {
    llrm::ir::parse_text(source).map(|module| llrm::ir::write_text(&module))
}

fn write_output(path: Option<&Path>, bytes: &[u8]) -> ExitCode {
    match path {
        Some(path) => match fs::write(path, bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failure(format!("{}: {error}", path.display())),
        },
        None => match io::stdout().write_all(bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failure(format!("stdout: {error}")),
        },
    }
}

fn failure(message: String) -> ExitCode {
    eprintln!("llrm-opt: {message}");
    ExitCode::FAILURE
}

const fn usage() -> &'static str {
    concat!(
        "usage: llrm-opt [-o FILE] [--passes PIPELINE] [--verify-each] INPUT.qir\n\n",
        "passes: algebraic-simplify, common-subexpression-elimination, constant-fold, ",
        "dead-code-elimination, dead-store-elimination, simplify-branches, ",
        "unreachable-block-elimination"
    )
}

#[cfg(test)]
mod tests {
    use super::{PassName, canonicalize, run_pipeline};

    #[test]
    fn verifies_and_canonicalizes_qir() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 void\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
            "block 0\n",
            "term return none\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        assert_eq!(canonicalize(source).as_deref(), Ok(source));
    }

    #[test]
    fn runs_the_constant_fold_pipeline() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 integer 8\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
            "block 0\n",
            "inst 0 results [0:0] binary add const type 0 integer 1 const type 0 integer 2\n",
            "term return some value 0\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        let output = run_pipeline(source, &[PassName::ConstantFold], true).unwrap();

        assert!(!output.contains("inst 0"));
        assert!(output.contains("term return some const type 0 integer 3"));
    }

    #[test]
    fn composes_constant_folding_with_branch_simplification() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 integer 8\n",
            "type 1 integer 1\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
            "block 0\n",
            "inst 0 results [0:1] binary add const type 1 integer 0 const type 1 integer 1\n",
            "term branch value 0 1 2\n",
            "endblock\n",
            "block 1\n",
            "term return some const type 0 integer 1\n",
            "endblock\n",
            "block 2\n",
            "term return some const type 0 integer 0\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        let output = run_pipeline(
            source,
            &[
                PassName::ConstantFold,
                PassName::AlgebraicSimplify,
                PassName::SimplifyBranches,
                PassName::DeadCodeElimination,
            ],
            true,
        )
        .unwrap();

        assert!(!output.contains("inst 0"));
        assert!(output.contains("term jump 1"));
    }

    #[test]
    fn runs_common_subexpression_elimination() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 integer 16\n",
            "function 0 \"main\" linkage internal result 0 parameters [0,0] variadic false cc far_pascal attributes []\n",
            "param 0 0\n",
            "param 1 0\n",
            "block 0\n",
            "inst 0 results [2:0] binary add value 0 value 1\n",
            "inst 1 results [3:0] binary add value 0 value 1\n",
            "term return some value 3\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        let output = run_pipeline(
            source,
            &[PassName::CommonSubexpressionElimination],
            true,
        )
        .unwrap();

        assert!(output.contains("inst 0 results [2:0]"));
        assert!(!output.contains("inst 1 results [3:0]"));
        assert!(output.contains("term return some value 2"));
    }

    #[test]
    fn removes_unreachable_blocks_deterministically() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 void\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
            "block 0\n",
            "term jump 1\n",
            "endblock\n",
            "block 1\n",
            "term return none\n",
            "endblock\n",
            "block 2\n",
            "term return none\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        let output = run_pipeline(source, &[PassName::UnreachableBlockElimination], true).unwrap();

        assert!(!output.contains("block 2\n"));
        assert_eq!(canonicalize(&output).as_deref(), Ok(output.as_str()));
        let repeated =
            run_pipeline(&output, &[PassName::UnreachableBlockElimination], true).unwrap();
        assert_eq!(repeated, output);
    }

    #[test]
    fn runs_dead_store_elimination_on_direct_global_stores() {
        let source = concat!(
            "qir 3\n",
            "module \"m\"\n",
            "type 0 void\n",
            "type 1 integer 8\n",
            "type 2 pointer neardata\n",
            "global 0 \"g\" 1 internal false some integer 0\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc far_pascal attributes []\n",
            "block 0\n",
            "inst 0 results [] store 1 false const type 2 globaladdr 0 0 const type 1 integer 1\n",
            "inst 1 results [] store 1 false const type 2 globaladdr 0 0 const type 1 integer 2\n",
            "term return none\n",
            "endblock\n",
            "endfunction\n",
            "end\n",
        );

        let output = run_pipeline(source, &[PassName::DeadStoreElimination], true).unwrap();

        assert!(!output.contains("inst 0 results [] store"));
        assert!(output.contains("inst 1 results [] store"));
        assert_eq!(canonicalize(&output).as_deref(), Ok(output.as_str()));
        let repeated = run_pipeline(&output, &[PassName::DeadStoreElimination], true).unwrap();
        assert_eq!(repeated, output);
    }
}
