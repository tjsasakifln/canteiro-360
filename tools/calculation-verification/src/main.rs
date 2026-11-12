use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!("Uso: canteiro-calculation-verification <reference-vectors.json>");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("Informe somente o caminho do documento de referência.");
        return ExitCode::from(2);
    }
    match fs::read_to_string(path) {
        Ok(input) => {
            let output = canteiro_calculation_verification::verify_document(&input);
            println!("{output}");
            if output.starts_with('{') {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("Não foi possível ler o documento de referência: {error}");
            ExitCode::from(2)
        }
    }
}
