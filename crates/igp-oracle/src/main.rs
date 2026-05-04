use clap::Parser;
use igp_oracle::{run_reconcile, Cli, Command, IgpOracleError};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Reconcile(args) => run_reconcile(args).await,
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("igp-oracle error: {err}");
            std::process::exit(exit_code_for_error(&err));
        }
    }
}

fn exit_code_for_error(err: &IgpOracleError) -> i32 {
    err.exit_code()
}
