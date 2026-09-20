use std::env;
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
}

impl Invocation {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, &'static str> {
        let mut input = None;
        let mut output = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => {
                    let path = arguments.next().ok_or("-o requires a path")?;
                    if output.replace(PathBuf::from(path)).is_some() {
                        return Err("-o may only be specified once");
                    }
                }
                _ if argument.starts_with('-') => return Err(usage()),
                _ if input.is_none() => input = Some(PathBuf::from(argument)),
                _ => return Err(usage()),
            }
        }
        Ok(Self {
            input: input.ok_or(usage())?,
            output,
        })
    }

    fn run(self) -> ExitCode {
        let source = match fs::read_to_string(&self.input) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let output = match canonicalize(&source) {
            Ok(output) => output,
            Err(error) => {
                return failure(format!(
                    "{}:{}:{}: {}",
                    self.input.display(),
                    error.line,
                    error.column,
                    error.message
                ));
            }
        };
        write_output(self.output.as_deref(), output.as_bytes())
    }
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
    "usage: llrm-opt [-o FILE] INPUT.qir"
}

#[cfg(test)]
mod tests {
    use super::canonicalize;

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
}
