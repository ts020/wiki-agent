pub mod frontmatter;
pub mod headings;
pub mod ingest;

pub use frontmatter::Frontmatter;
pub use headings::Heading;
pub use ingest::{NoteData, ingest_notes, should_ingest};
