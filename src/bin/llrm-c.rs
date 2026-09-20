use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation.run(),
        Err(message) => {
            eprintln!("llrm-c: {message}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    input: PathBuf,
}

impl Invocation {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let arguments: Vec<_> = arguments.collect();
        if arguments.len() != 1 {
            return Err(usage().to_owned());
        }
        let input = PathBuf::from(&arguments[0]);
        if !input
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cgs"))
        {
            return Err("WCC capture input must have a .cgs extension".to_owned());
        }
        Ok(Self { input })
    }

    fn run(self) -> ExitCode {
        eprintln!(
            "llrm-c: {}: the WCC capture frontend has not been ported yet",
            self.input.display()
        );
        ExitCode::FAILURE
    }
}

fn usage() -> &'static str {
    "usage: llrm-c INPUT.cgs"
}

#[cfg(test)]
mod tests {
    use super::Invocation;

    #[test]
    fn accepts_only_wcc_capture_input() {
        assert!(Invocation::parse(["program.cgs".to_owned()].into_iter()).is_ok());
        assert!(Invocation::parse(["program.bas".to_owned()].into_iter()).is_err());
    }
}
