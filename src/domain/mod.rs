#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
}

impl SessionSummary {
    pub fn new(name: impl Into<String>, window_count: usize, attached: bool) -> Self {
        Self {
            name: name.into(),
            window_count,
            attached,
        }
    }
}
