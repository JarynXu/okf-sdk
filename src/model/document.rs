#[derive(Debug, Clone, Default)]
pub struct Document {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
}
