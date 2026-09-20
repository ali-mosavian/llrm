use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use llrm::driver;

fn main() -> ExitCode {
    match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation.run(),
        Err(message) => {
            eprintln!("llrm-omf: {message}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
}

impl Invocation {
    fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut input = None;
        let mut output = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => {
                    output = Some(PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "-o requires a value".to_owned())?,
                    ));
                }
                "-h" | "--help" => return Err(usage().to_owned()),
                _ if argument.starts_with('-') => {
                    return Err(format!("unknown option {argument:?}\n{}", usage()));
                }
                _ if input.is_none() => input = Some(PathBuf::from(argument)),
                _ => return Err("expected exactly one input path".to_owned()),
            }
        }
        let input = input.ok_or_else(|| usage().to_owned())?;
        if !input
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("obj") || extension.eq_ignore_ascii_case("lib")
            })
        {
            return Err("OMF input must have an .obj or .lib extension".to_owned());
        }
        Ok(Self { input, output })
    }

    fn run(self) -> ExitCode {
        let bytes = match fs::read(&self.input) {
            Ok(bytes) => bytes,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let file = match driver::parse_omf(&bytes) {
            Ok(file) => file,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        write_output(self.output.as_deref(), &file.to_bytes())
    }
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
    eprintln!("llrm-omf: {message}");
    ExitCode::FAILURE
}

fn usage() -> &'static str {
    "usage: llrm-omf [-o FILE] INPUT.obj|INPUT.lib"
}

#[cfg(test)]
mod tests {
    use super::Invocation;

    #[test]
    fn accepts_object_and_library_inputs() {
        assert!(Invocation::parse(["module.obj".to_owned()].into_iter()).is_ok());
        assert!(Invocation::parse(["library.lib".to_owned()].into_iter()).is_ok());
        assert!(Invocation::parse(["program.bas".to_owned()].into_iter()).is_err());
    }
}
