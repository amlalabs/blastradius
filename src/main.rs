use clap::Parser;

use blastradius::cli::Cli;

fn main() {
    let cli = Cli::parse();
    let code = blastradius::run(cli);
    std::process::exit(code);
}
