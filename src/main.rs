use std::{
    collections::VecDeque,
    error::Error,
    io::{stdin, BufRead},
};

use clap::Parser;
use console::Term;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    /// Output NUM lines to the terminal
    #[arg(short = 'n', long, value_name = "NUM", default_value = "10")]
    lines: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let term = Term::buffered_stdout();
    let mut history = VecDeque::with_capacity(cli.lines);

    let mut lines = stdin().lock().lines();
    while let Some(line) = lines.next().transpose()? {
        term.clear_last_lines(history.len())?;

        if history.len() == cli.lines {
            history.pop_front();
        }
        history.push_back(line);

        let width = term.size().1 as usize;
        for line in &history {
            term.write_line(&line[..width.min(line.len())])?;
        }
        term.flush()?;
    }

    Ok(())
}
