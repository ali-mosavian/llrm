use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let invocation = match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("llrm-llc: {message}");
            return ExitCode::from(2);
        }
    };
    invocation.run()
}

#[derive(Debug)]
struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
}

impl Invocation {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut input = None;
        let mut output = None;
        let mut emit_seen = false;

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => {
                    let path = arguments.next().ok_or("-o requires a path")?;
                    if output.replace(PathBuf::from(path)).is_some() {
                        return Err("-o may only be specified once".into());
                    }
                }
                "--emit" => {
                    let emit = arguments.next().ok_or("--emit requires a value")?;
                    if emit_seen {
                        return Err("--emit may only be specified once".into());
                    }
                    if emit != "qmir" {
                        return Err(format!(
                            "unsupported output kind {emit:?}; only qmir is available"
                        ));
                    }
                    emit_seen = true;
                }
                "-h" | "--help" => return Err(usage().to_owned()),
                _ if argument.starts_with('-') => return Err(usage().to_owned()),
                _ if input.is_none() => input = Some(PathBuf::from(argument)),
                _ => return Err(usage().to_owned()),
            }
        }

        Ok(Self {
            input: input.ok_or_else(|| usage().to_owned())?,
            output,
        })
    }

    fn run(self) -> ExitCode {
        let source = match fs::read_to_string(&self.input) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let output = match compile(&source) {
            Ok(output) => output,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        write_output(self.output.as_deref(), output.as_bytes())
    }
}

#[derive(Debug)]
enum CompileError {
    Text(llrm::ir::TextError),
    Driver(llrm::driver::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(error) => write!(
                formatter,
                "{}:{}: {}",
                error.line, error.column, error.message
            ),
            Self::Driver(error) => error.fmt(formatter),
        }
    }
}

impl Error for CompileError {}

/// Parses verified portable IR, selects the initial x86 subset, and writes qmir.
fn compile(source: &str) -> Result<String, CompileError> {
    let module = llrm::ir::parse_text(source).map_err(CompileError::Text)?;
    let machine = llrm::driver::lower_ir_to_machine(&module).map_err(CompileError::Driver)?;
    Ok(llrm::codegen::machine::write_text(&machine))
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
    eprintln!("llrm-llc: {message}");
    ExitCode::FAILURE
}

const fn usage() -> &'static str {
    "usage: llrm-llc [--emit qmir] [-o FILE] INPUT.qir"
}

#[cfg(test)]
mod tests {
    use super::{CompileError, Invocation, compile};
    use llrm::target::x86::X86Opcode;

    const MINIMAL_INTEGER_IR: &str = concat!(
        "qir 1\n",
        "module \"m\"\n",
        "type 0 void\n",
        "type 1 integer 16\n",
        "function 0 \"main\" linkage internal result 0 parameters [] variadic false cc basic attributes []\n",
        "block 0\n",
        "inst 0 results [0:1] binary add const type 1 integer 1 const type 1 integer 2\n",
        "term return none\n",
        "endblock\n",
        "endfunction\n",
        "end\n",
    );

    #[test]
    fn selects_integer_ir_to_verified_qmir() {
        let qmir = compile(MINIMAL_INTEGER_IR).expect("the selected integer subset must compile");
        let machine = llrm::codegen::machine::parse_text(&qmir)
            .expect("the qmir writer must emit parseable text");
        machine.verify().expect("selected qmir must verify");
        let instructions = machine.functions[0]
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .collect::<Vec<_>>();

        assert!(
            instructions.iter().any(
                |instruction| instruction.opcode.get() == X86Opcode::Mov.machine_opcode().get()
            )
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.opcode.get()
                    == X86Opcode::Copy.machine_opcode().get())
        );
        assert!(
            instructions.iter().any(
                |instruction| instruction.opcode.get() == X86Opcode::Add.machine_opcode().get()
            )
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.opcode.get()
                    == X86Opcode::ReturnNear.machine_opcode().get())
        );
    }

    #[test]
    fn reports_text_failures_without_selecting() {
        assert!(matches!(compile("qir 1\n"), Err(CompileError::Text(_))));
    }

    #[test]
    fn rejects_non_qmir_emit_values_as_invalid_invocation() {
        let error = Invocation::parse(
            ["--emit", "asm", "program.qir"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("assembly output is not implemented in the initial llc slice");
        assert!(error.contains("only qmir"));
    }
}
