use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct BearNote {
    pub id: String,
    pub title: String,
    pub text: String,
    pub tags: Vec<String>,
    #[allow(dead_code)]
    pub created: OffsetDateTime,
    pub modified: OffsetDateTime,
    pub is_trashed: bool,
    pub is_archived: bool,
    pub is_pinned: bool,
}
