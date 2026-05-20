use std::process::Command;

pub fn command(program: &str) -> Command {
    Command::new(program)
}
