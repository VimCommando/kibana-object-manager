//! Kibana Skills API
//!
//! Support for managing Kibana skills via /api/agent_builder/skills endpoints.

mod extractor;
mod loader;
mod manifest;
mod storage;

pub use extractor::{SkillsExtractor, parse_skills_response};
pub use loader::SkillsLoader;
pub use manifest::{SkillEntry, SkillsManifest};
pub(crate) use storage::skill_files_to_value;
pub use storage::{
    ReferencedContent, SkillFrontmatter, read_skill_directory, sanitize_path_component,
    skill_directory_name, skill_to_directory, skill_to_value,
};
