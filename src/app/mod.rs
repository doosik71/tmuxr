use anyhow::Result;

pub struct App;

impl App {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) -> Result<()> {
        println!("tmuxr bootstrap is ready. Session-centric TUI features come next.");
        Ok(())
    }
}
