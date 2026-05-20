#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
}
