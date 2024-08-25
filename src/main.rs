use std::{
    cell::RefCell,
    collections::VecDeque,
    error::Error,
    io::{stdin, BufRead},
    iter::once,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use clap::Parser;
use console::Term;
use itertools::join;

static THREAD_NAME: &str = "Console Write";

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    /// Output NUM lines to the terminal
    #[arg(short = 'n', long, value_name = "NUM", default_value = "10")]
    lines: usize,

    /// Retain NUM lines after the input stream is closed
    #[arg(long, value_name = "NUM")]
    retain: Option<usize>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let (tx, rx) = mpsc::channel();
    let thread = thread::Builder::new()
        .name(THREAD_NAME.into())
        .spawn(move || {
            console_writer(
                EmitterOpts {
                    lines: cli.lines,
                    retain: cli.retain.unwrap_or(cli.lines),
                },
                rx,
            )
        })?;

    let result = pipe_reader(stdin().lock(), tx);
    thread.join().unwrap();

    result
}

enum Cmd {
    Fragment(String),
    Line(String),
}

fn pipe_reader<R>(mut rd: R, tx: Sender<Cmd>) -> Result<(), Box<dyn Error>>
where
    R: BufRead,
{
    loop {
        let mut buffer = String::new();
        if rd.read_line(&mut buffer)? == 0 {
            break;
        }

        let cmd = match buffer.trim_end_matches('\n').len() {
            len if len == buffer.len() => Cmd::Fragment(buffer),
            len => {
                buffer.truncate(len);
                Cmd::Line(buffer)
            }
        };

        tx.send(cmd)?;
    }

    Ok(())
}

fn console_writer(emitter_opts: EmitterOpts, rx: Receiver<Cmd>) {
    let mut emitter = Emitter::new(emitter_opts);

    loop {
        match rx.recv() {
            Err(_) => break,
            Ok(Cmd::Fragment(new_fragment)) => emitter.add_fragment(new_fragment),
            Ok(Cmd::Line(new_line)) => emitter.add_line(new_line).unwrap(),
        };
    }
}

struct EmitterOpts {
    lines: usize,
    retain: usize,
}

struct Emitter {
    lines: VecDeque<String>,
    retain: usize,
    fragments: RefCell<Vec<String>>,
    term: Term,
}

impl Drop for Emitter {
    fn drop(&mut self) {
        let _ = self
            .flush_fragments()
            .and_then(|_| self.truncate(self.retain));
    }
}

impl Emitter {
    fn new(opts: EmitterOpts) -> Self {
        Self {
            lines: VecDeque::with_capacity(opts.lines),
            retain: opts.retain,
            fragments: RefCell::new(Vec::default()),
            term: Term::stdout(),
        }
    }

    fn add_line(&mut self, new_line: String) -> Result<(), Box<dyn Error>> {
        let fragments = self.fragments.take();
        let line = if fragments.is_empty() {
            new_line
        } else {
            join(fragments.iter().chain(once(&new_line)), "")
        };
        self.write_line(line)?;
        Ok(())
    }

    fn add_fragment(&mut self, new_fragment: String) {
        self.fragments.get_mut().push(new_fragment);
    }

    fn flush_fragments(&mut self) -> Result<(), Box<dyn Error>> {
        let fragments = self.fragments.take();
        if !fragments.is_empty() {
            let line = fragments.join("");
            self.write_line(line)?;
        }
        Ok(())
    }

    fn write_line(&mut self, line: String) -> Result<(), Box<dyn Error>> {
        let width = self.term.size().1 as usize;
        if self.lines.len() < self.lines.capacity() {
            self.term.write_line(&line[..width.min(line.len())])?;
            self.lines.push_back(line);
        } else {
            self.lines.pop_front();
            self.lines.push_back(line);
            self.term.clear_last_lines(self.lines.capacity())?;
            for line in &self.lines {
                self.term.write_line(&line[..width.min(line.len())])?;
            }
        }
        Ok(())
    }

    fn truncate(&mut self, retain: usize) -> Result<(), Box<dyn Error>> {
        if self.lines.len() > retain {
            self.term.clear_last_lines(self.lines.len())?;
            while self.lines.len() > retain {
                self.lines.pop_front();
            }
            for line in &self.lines {
                self.term.write_line(line)?;
            }
        }
        Ok(())
    }
}
