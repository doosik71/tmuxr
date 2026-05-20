use anyhow::Result;

use crate::tmux::TmuxClient;

pub struct App;

impl App {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) -> Result<()> {
        let client = TmuxClient::new();
        let context = client.detect();

        if !context.binary_available {
            println!("tmux is not installed or not available in PATH.");
            return Ok(());
        }

        let sessions = client.list_sessions()?;

        println!("tmuxr core integration bootstrap");
        println!("inside tmux client: {}", context.inside_client);
        println!("available sessions: {}", sessions.len());

        for session in sessions {
            println!(
                "- {} (windows: {}, attached: {})",
                session.name, session.window_count, session.attached
            );
        }

        Ok(())
    }
}
