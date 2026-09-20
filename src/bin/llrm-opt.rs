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
    ConstantFold,
    DeadCodeElimination,
    SimplifyBranches,
}

fn parse_passes(value: &str) -> Result<Vec<PassName>, String> {
    if value.is_empty() {
        return Err("--passes requires a nonempty comma-separated pipeline".into());
    }
    value
        .split(',')
        .map(|name| match name {
            "algebraic-simplify" => Ok(PassName::AlgebraicSimplify),
            "constant-fold" => Ok(PassName::ConstantFold),
            "dead-code-elimination" => Ok(PassName::DeadCodeElimination),
            "simplify-branches" => Ok(PassName::SimplifyBranches),
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
            PassName::ConstantFold => manager.add_pass(
                llrm::transforms::ConstantFold::new(&module).map_err(PipelineError::Fold)?,
            ),
            PassName::DeadCodeElimination => {
                manager.add_pass(llrm::transforms::DeadCodeElimination::new())
            }
            PassName::SimplifyBranches => manager.add_pass(
                llrm::transforms::SimplifyBranches::new(&module)
                    .map_err(PipelineError::BranchSimplify)?,
            ),
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
    "usage: llrm-opt [-o FILE] [--passes PIPELINE] [--verify-each] INPUT.qir"
}

#[cfg(test)]
mod tests {
    use super::{PassName, canonicalize, run_pipeline};

    #[test]
    fn verifies_and_canonicalizes_qir() {
        let source = concat!(
            "qir 1\n",
            "module \"m\"\n",
            "type 0 void\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc basic attributes []\n",
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
            "qir 1\n",
            "module \"m\"\n",
            "type 0 integer 8\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc basic attributes []\n",
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
            "qir 1\n",
            "module \"m\"\n",
            "type 0 integer 8\n",
            "type 1 integer 1\n",
            "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc basic attributes []\n",
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
}
