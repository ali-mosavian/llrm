use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut tokens_only = false;
    let mut syntax_only = false;
    let mut input = None;
    for argument in env::args().skip(1) {
        if argument == "--tokens" {
            tokens_only = true;
        } else if argument == "--syntax" {
            syntax_only = true;
        } else if argument.starts_with('-') {
            eprintln!("llrm-modern: unknown option {argument:?}");
            return ExitCode::from(2);
        } else {
            if input.is_some() {
                eprintln!("llrm-modern: expected one input file");
                return ExitCode::from(2);
            }
            input = Some(argument);
        }
    }
    if tokens_only && syntax_only {
        eprintln!("llrm-modern: --tokens and --syntax are mutually exclusive");
        return ExitCode::from(2);
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-modern [--tokens|--syntax] FILE");
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("llrm-modern: {input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let tokens = match llrm::frontend::modern::lex(&source) {
        Ok(tokens) => tokens,
        Err(error) => return report(&input, error),
    };
    if tokens_only {
        for token in tokens {
            println!("{}:{} {:?}", token.span.line, token.span.column, token.kind);
        }
        return ExitCode::SUCCESS;
    }
    let module = match llrm::frontend::modern::parse(tokens) {
        Ok(module) => module,
        Err(error) => return report(&input, error),
    };
    if syntax_only {
        println!("{module:#?}");
        return ExitCode::SUCCESS;
    }
    let module_name = Path::new(&input)
        .file_stem()
        .and_then(|one| one.to_str())
        .unwrap_or("module");
    match llrm::frontend::modern::semantic::compile(&module, module_name) {
        Ok(hir) => {
            print!("{hir}");
            ExitCode::SUCCESS
        }
        Err(error) => report(&input, error),
    }
}

fn report(path: &str, error: llrm::frontend::modern::Diagnostic) -> ExitCode {
    eprintln!(
        "{path}:{}:{}: {}",
        error.span.line, error.span.column, error.message
    );
    ExitCode::FAILURE
}
