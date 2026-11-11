//! Ponto de entrada do futuro consumidor de tarefas persistentes.
//! Não deve anunciar processamento funcional antes de existir fila e autorização.
use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args().skip(1).collect::<Vec<_>>() == ["--check"] {
        println!("Canteiro 360: executável disponível; processamento ainda não habilitado.");
        return ExitCode::SUCCESS;
    }
    eprintln!("Processamento ainda não habilitado. Use --check para verificar o executável.");
    ExitCode::from(64)
}
