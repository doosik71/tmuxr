#[derive(Debug, Clone)]
pub struct AppConfig {
    pub mouse_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
        }
    }
}
