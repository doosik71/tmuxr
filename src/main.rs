use anyhow::Result;
use tmuxr::app::App;

fn main() -> Result<()> {
    let mut app = App::new();
    app.run()
}
