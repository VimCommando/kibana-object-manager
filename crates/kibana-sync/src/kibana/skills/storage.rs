//! Markdown-first Skills filesystem representation.

use crate::{Error, Result, ResultContext};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

const SKILL_FILE: &str = "SKILL.md";
const REFERENCED_CONTENT_METADATA_FILE: &str = ".referenced_content.yml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillFrontmatter {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub experimental: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReferencedContent {
    pub name: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillDirectory {
    pub frontmatter: SkillFrontmatter,
    pub content: String,
    pub referenced_content: Vec<ReferencedContent>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ReferencedContentMetadata {
    #[serde(default)]
    entries: Vec<ReferencedContentMetadataEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReferencedContentMetadataEntry {
    path: String,
    name: String,
    #[serde(rename = "relativePath")]
    relative_path: String,
}

impl SkillDirectory {
    pub fn to_value(&self, include_id: bool) -> Value {
        let mut object = Map::new();

        if include_id {
            object.insert("id".to_string(), Value::String(self.frontmatter.id.clone()));
        }

        if let Some(name) = &self.frontmatter.name {
            object.insert("name".to_string(), Value::String(name.clone()));
        }

        if let Some(description) = &self.frontmatter.description {
            object.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }

        object.insert("content".to_string(), Value::String(self.content.clone()));
        object.insert(
            "tool_ids".to_string(),
            Value::Array(
                self.frontmatter
                    .tool_ids
                    .iter()
                    .map(|id| Value::String(id.clone()))
                    .collect(),
            ),
        );
        object.insert(
            "referenced_content".to_string(),
            Value::Array(
                self.referenced_content
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "name": entry.name.clone(),
                            "relativePath": entry.relative_path.clone(),
                            "content": entry.content.clone(),
                        })
                    })
                    .collect(),
            ),
        );

        if let Some(experimental) = self.frontmatter.experimental {
            object.insert("experimental".to_string(), Value::Bool(experimental));
        }

        Value::Object(object)
    }
}

pub fn skill_directory_name(skill: &Value) -> Result<String> {
    let id = required_str(skill, "id")?;
    Ok(sanitize_path_component(id))
}

pub fn skill_to_directory(root: &Path, skill: &Value) -> Result<PathBuf> {
    let directory = root.join(skill_directory_name(skill)?);
    if directory.exists() {
        let metadata = std::fs::symlink_metadata(&directory).with_context(|| {
            format!(
                "Failed to inspect existing skill directory: {}",
                directory.display()
            )
        })?;
        if metadata.is_dir() {
            std::fs::remove_dir_all(&directory).with_context(|| {
                format!(
                    "Failed to remove existing skill directory: {}",
                    directory.display()
                )
            })?;
        } else {
            std::fs::remove_file(&directory).with_context(|| {
                format!(
                    "Failed to remove existing skill path: {}",
                    directory.display()
                )
            })?;
        }
    }
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create skill directory: {}", directory.display()))?;

    let document = skill_value_to_directory(skill)?;
    write_skill_file(&directory.join(SKILL_FILE), &document)?;
    write_referenced_content(&directory, &document.referenced_content)?;

    Ok(directory)
}

pub fn read_skill_directory(directory: &Path) -> Result<SkillDirectory> {
    let canonical_directory = directory
        .canonicalize()
        .with_context(|| format!("Failed to resolve skill directory: {}", directory.display()))?;
    let skill_file = directory.join(SKILL_FILE);
    let canonical_skill_file = skill_file
        .canonicalize()
        .with_context(|| format!("Failed to resolve skill file: {}", skill_file.display()))?;
    if !canonical_skill_file.starts_with(&canonical_directory) {
        return Err(Error::message(format!(
            "skill file escapes skill directory: {}",
            skill_file.display()
        )));
    }

    let content = std::fs::read_to_string(&skill_file)
        .with_context(|| format!("Failed to read skill file: {}", skill_file.display()))?;
    let (frontmatter, body) = parse_skill_markdown(&content)?;
    let referenced_content = read_referenced_content(directory)?;

    Ok(SkillDirectory {
        frontmatter,
        content: body.to_string(),
        referenced_content,
    })
}

pub fn skill_to_value(directory: &Path, include_id: bool) -> Result<Value> {
    Ok(read_skill_directory(directory)?.to_value(include_id))
}

pub fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '&' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect::<String>()
        .trim()
        .to_string();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "unnamed".to_string()
    } else {
        sanitized
    }
}

fn skill_value_to_directory(skill: &Value) -> Result<SkillDirectory> {
    let id = required_str(skill, "id")?.to_string();
    let frontmatter = SkillFrontmatter {
        id,
        name: optional_str(skill, "name").map(ToOwned::to_owned),
        description: optional_str(skill, "description").map(ToOwned::to_owned),
        tool_ids: string_array(skill.get("tool_ids")).unwrap_or_default(),
        experimental: skill.get("experimental").and_then(|value| value.as_bool()),
    };
    let content = optional_str(skill, "content")
        .unwrap_or_default()
        .to_string();
    let referenced_content = referenced_content_from_value(skill)?;

    Ok(SkillDirectory {
        frontmatter,
        content,
        referenced_content,
    })
}

fn write_skill_file(path: &Path, document: &SkillDirectory) -> Result<()> {
    let yaml = yaml_serde::to_string(&document.frontmatter)
        .context("Failed to serialize skill frontmatter")?;
    let markdown = format!("---\n{}---\n{}", yaml, document.content);
    std::fs::write(path, markdown)
        .with_context(|| format!("Failed to write skill file: {}", path.display()))
}

fn parse_skill_markdown(markdown: &str) -> Result<(SkillFrontmatter, &str)> {
    let (rest, delimiter) = if let Some(rest) = markdown.strip_prefix("---\n") {
        (rest, "\n---\n")
    } else if let Some(rest) = markdown.strip_prefix("---\r\n") {
        (rest, "\r\n---\r\n")
    } else {
        return Err(Error::message("skill file is missing YAML frontmatter"));
    };

    let Some((yaml, body)) = rest.split_once(delimiter) else {
        return Err(Error::message(
            "skill file has unterminated YAML frontmatter",
        ));
    };
    let frontmatter: SkillFrontmatter =
        yaml_serde::from_str(yaml).context("Failed to parse skill frontmatter")?;
    if frontmatter.id.is_empty() {
        return Err(Error::MissingResourceId { resource: "skill" });
    }
    Ok((frontmatter, body))
}

fn write_referenced_content(root: &Path, entries: &[ReferencedContent]) -> Result<()> {
    let mut metadata = ReferencedContentMetadata::default();
    let mut seen_paths = BTreeSet::new();

    for entry in entries {
        let relative_dir = safe_relative_dir(&entry.relative_path)?;
        let sanitized_name = sanitize_path_component(&entry.name);
        let file_name = format!("{sanitized_name}.md");
        let relative_file = relative_dir.join(file_name);
        let metadata_path = metadata_path_for(&relative_file)?;
        let comparable_metadata_path = metadata_path.to_lowercase();
        if !seen_paths.insert(comparable_metadata_path) {
            return Err(Error::message(format!(
                "duplicate referenced content path after sanitization: {metadata_path}"
            )));
        }

        let path = root.join(&relative_file);
        if path.file_name().and_then(|name| name.to_str()) == Some(SKILL_FILE) {
            return Err(Error::message(
                "referenced content cannot be written as SKILL.md",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(&path, &entry.content)
            .with_context(|| format!("Failed to write referenced content: {}", path.display()))?;

        let normalized_relative_path = normalize_api_relative_path(&entry.relative_path)?;
        let derived_relative_path = path_to_api_relative_path(&relative_dir);
        if sanitized_name != entry.name || derived_relative_path != normalized_relative_path {
            metadata.entries.push(ReferencedContentMetadataEntry {
                path: metadata_path,
                name: entry.name.clone(),
                relative_path: normalized_relative_path,
            });
        }
    }

    write_referenced_content_metadata(root, &metadata)?;
    Ok(())
}

fn read_referenced_content(root: &Path) -> Result<Vec<ReferencedContent>> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("Failed to resolve skill directory: {}", root.display()))?;
    let mut files = Vec::new();
    collect_markdown_files(&canonical_root, root, &mut files)?;
    files.sort();
    let metadata = read_referenced_content_metadata(root)?;

    let mut entries = files
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(root).map_err(|_| {
                Error::message(format!(
                    "referenced content escaped skill directory: {}",
                    path.display()
                ))
            })?;
            let parent = relative.parent().unwrap_or_else(|| Path::new(""));
            let metadata_entry = metadata_entry_for(&metadata, relative)?;
            let relative_path = metadata_entry
                .map(|entry| entry.relative_path.clone())
                .unwrap_or_else(|| path_to_api_relative_path(parent));
            let name = if let Some(entry) = metadata_entry {
                entry.name.clone()
            } else {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .ok_or_else(|| Error::message("referenced content filename is not UTF-8"))?
                    .to_string()
            };
            let content = std::fs::read_to_string(&path).with_context(|| {
                format!("Failed to read referenced content: {}", path.display())
            })?;
            Ok(ReferencedContent {
                name,
                relative_path,
                content,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(entries)
}

fn read_referenced_content_metadata(root: &Path) -> Result<ReferencedContentMetadata> {
    let path = root.join(REFERENCED_CONTENT_METADATA_FILE);
    if !path.exists() {
        return Ok(ReferencedContentMetadata::default());
    }

    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read referenced content metadata: {}",
            path.display()
        )
    })?;
    yaml_serde::from_str(&content).context("Failed to parse referenced content metadata")
}

fn write_referenced_content_metadata(
    root: &Path,
    metadata: &ReferencedContentMetadata,
) -> Result<()> {
    if metadata.entries.is_empty() {
        return Ok(());
    }

    let path = root.join(REFERENCED_CONTENT_METADATA_FILE);
    let yaml = yaml_serde::to_string(metadata)
        .context("Failed to serialize referenced content metadata")?;
    std::fs::write(&path, yaml).with_context(|| {
        format!(
            "Failed to write referenced content metadata: {}",
            path.display()
        )
    })
}

fn metadata_entry_for<'a>(
    metadata: &'a ReferencedContentMetadata,
    relative: &Path,
) -> Result<Option<&'a ReferencedContentMetadataEntry>> {
    let path = metadata_path_for(relative)?;
    Ok(metadata.entries.iter().find(|entry| entry.path == path))
}

fn collect_markdown_files(
    canonical_root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let canonical = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;
        if !canonical.starts_with(canonical_root) {
            return Err(Error::message(format!(
                "path escapes skill directory: {}",
                path.display()
            )));
        }

        if path.is_dir() {
            collect_markdown_files(canonical_root, &path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md")
            && path.file_name().and_then(|name| name.to_str()) != Some(SKILL_FILE)
        {
            files.push(path);
        }
    }

    Ok(())
}

fn referenced_content_from_value(skill: &Value) -> Result<Vec<ReferencedContent>> {
    let mut entries = Vec::new();
    let Some(array) = skill
        .get("referenced_content")
        .and_then(|value| value.as_array())
    else {
        return Ok(entries);
    };

    for value in array {
        entries.push(ReferencedContent {
            name: required_nested_str(value, "name")?.to_string(),
            relative_path: required_nested_str(value, "relativePath")?.to_string(),
            content: required_nested_str(value, "content")?.to_string(),
        });
    }
    entries.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(entries)
}

fn safe_relative_dir(relative_path: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();
    if relative_path.is_empty() {
        return Ok(path);
    }

    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| Error::message("relativePath contains non-UTF-8 data"))?;
                path.push(sanitize_path_component(value));
            }
            Component::CurDir => {}
            _ => {
                return Err(Error::message(format!(
                    "unsafe referenced content relativePath: {relative_path}"
                )));
            }
        }
    }

    Ok(path)
}

fn normalize_api_relative_path(relative_path: &str) -> Result<String> {
    let mut parts = Vec::new();
    if relative_path.is_empty() {
        return Ok(String::new());
    }

    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| Error::message("relativePath contains non-UTF-8 data"))?;
                parts.push(value);
            }
            Component::CurDir => {}
            _ => {
                return Err(Error::message(format!(
                    "unsafe referenced content relativePath: {relative_path}"
                )));
            }
        }
    }

    if parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("./{}", parts.join("/")))
    }
}

fn metadata_path_for(relative: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| Error::message("referenced content path is not UTF-8"))?;
                parts.push(value);
            }
            _ => {
                return Err(Error::message(format!(
                    "unsafe referenced content path: {}",
                    relative.display()
                )));
            }
        }
    }

    Ok(parts.join("/"))
}

fn path_to_api_relative_path(path: &Path) -> String {
    let relative = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    if relative.is_empty() {
        relative
    } else {
        format!("./{relative}")
    }
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(|value| {
        value.as_array().map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
    })
}

fn required_str<'a>(value: &'a Value, field: &'static str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(|field| field.as_str())
        .ok_or(if field == "id" {
            Error::MissingResourceId { resource: "skill" }
        } else {
            Error::MissingField { field }
        })
}

fn required_nested_str<'a>(value: &'a Value, field: &'static str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(|field| field.as_str())
        .ok_or(Error::MissingField { field })
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(|field| field.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn writes_and_reads_skill_directory() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "threat-hunting-copy",
            "name": "Threat Hunting Copy",
            "description": "A copied skill",
            "content": "Main body\n",
            "tool_ids": ["tool-a"],
            "experimental": true,
            "referenced_content": [
                {"name": "overview", "relativePath": "", "content": "Root ref\n"},
                {"name": "query", "relativePath": "examples", "content": "from logs\n"}
            ],
            "readonly": true
        });

        let dir = skill_to_directory(temp.path(), &skill).unwrap();

        assert_eq!(
            dir.file_name().and_then(|name| name.to_str()),
            Some("threat-hunting-copy")
        );
        assert!(dir.join("SKILL.md").exists());
        assert!(dir.join("overview.md").exists());
        assert!(dir.join("examples/query.md").exists());
        assert!(!dir.join(REFERENCED_CONTENT_METADATA_FILE).exists());

        let read = read_skill_directory(&dir).unwrap();
        assert_eq!(read.frontmatter.id, "threat-hunting-copy");
        assert_eq!(read.content, "Main body\n");
        assert_eq!(read.referenced_content.len(), 2);

        let projected = read.to_value(true);
        assert_eq!(projected["id"], "threat-hunting-copy");
        assert_eq!(projected["referenced_content"][0]["relativePath"], "");
        assert_eq!(
            projected["referenced_content"][1]["relativePath"],
            "./examples"
        );
        assert!(projected.get("readonly").is_none());
    }

    #[test]
    fn update_projection_omits_id_and_uses_stable_empty_arrays() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "empty-skill",
            "name": "Empty Skill",
            "content": ""
        });

        let dir = skill_to_directory(temp.path(), &skill).unwrap();
        let projected = skill_to_value(&dir, false).unwrap();

        assert!(projected.get("id").is_none());
        assert_eq!(projected["tool_ids"], json!([]));
        assert_eq!(projected["referenced_content"], json!([]));
    }

    #[test]
    fn sanitizes_dot_path_skill_ids() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "..",
            "name": "Dot Dot",
            "content": "Body\n"
        });

        let dir = skill_to_directory(temp.path(), &skill).unwrap();

        assert_eq!(dir, temp.path().join("unnamed"));
        assert!(dir.join(SKILL_FILE).exists());
        assert!(!temp.path().join(SKILL_FILE).exists());
    }

    #[test]
    fn parses_crlf_frontmatter_without_changing_body() {
        let markdown = "---\r\nid: crlf-skill\r\nname: CRLF Skill\r\n---\r\nBody\r\n";

        let (frontmatter, body) = parse_skill_markdown(markdown).unwrap();

        assert_eq!(frontmatter.id, "crlf-skill");
        assert_eq!(frontmatter.name.as_deref(), Some("CRLF Skill"));
        assert_eq!(body, "Body\r\n");
    }

    #[test]
    fn preserves_referenced_content_values_when_paths_are_sanitized() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "sanitize-skill",
            "name": "Sanitize Skill",
            "content": "Body\n",
            "referenced_content": [
                {
                    "name": "query:prod",
                    "relativePath": "./examples:prod",
                    "content": "from logs\n"
                }
            ]
        });

        let dir = skill_to_directory(temp.path(), &skill).unwrap();

        assert!(dir.join("examples_prod/query_prod.md").exists());
        assert!(dir.join(REFERENCED_CONTENT_METADATA_FILE).exists());

        let projected = skill_to_value(&dir, true).unwrap();
        assert_eq!(projected["referenced_content"][0]["name"], "query:prod");
        assert_eq!(
            projected["referenced_content"][0]["relativePath"],
            "./examples:prod"
        );
        assert_eq!(projected["referenced_content"][0]["content"], "from logs\n");
    }

    #[test]
    fn rewrites_skill_directory_without_stale_referenced_content() {
        let temp = TempDir::new().unwrap();
        let first = json!({
            "id": "rewrite-skill",
            "name": "Rewrite Skill",
            "content": "Body\n",
            "referenced_content": [
                {"name": "old", "relativePath": "", "content": "old\n"}
            ]
        });
        let second = json!({
            "id": "rewrite-skill",
            "name": "Rewrite Skill",
            "content": "Body\n",
            "referenced_content": [
                {"name": "new", "relativePath": "", "content": "new\n"}
            ]
        });

        let dir = skill_to_directory(temp.path(), &first).unwrap();
        assert!(dir.join("old.md").exists());

        let dir = skill_to_directory(temp.path(), &second).unwrap();
        let projected = skill_to_value(&dir, true).unwrap();

        assert!(!dir.join("old.md").exists());
        assert!(dir.join("new.md").exists());
        assert_eq!(projected["referenced_content"].as_array().unwrap().len(), 1);
        assert_eq!(projected["referenced_content"][0]["name"], "new");
    }

    #[test]
    fn rejects_case_insensitive_referenced_content_path_collisions() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "case-skill",
            "name": "Case Skill",
            "content": "Body\n",
            "referenced_content": [
                {"name": "Query", "relativePath": "", "content": "upper\n"},
                {"name": "query", "relativePath": "", "content": "lower\n"}
            ]
        });

        let err = skill_to_directory(temp.path(), &skill).unwrap_err();

        assert!(
            err.to_string()
                .contains("duplicate referenced content path")
        );
    }

    #[test]
    fn rejects_skill_file_symlink_escape() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skill");
        std::fs::create_dir(&skill_dir).unwrap();
        let outside = temp.path().join("outside.md");
        std::fs::write(&outside, "---\nid: escaped\n---\nBody\n").unwrap();
        symlink_file(&outside, &skill_dir.join(SKILL_FILE)).unwrap();

        let err = read_skill_directory(&skill_dir).unwrap_err();

        assert!(
            err.to_string()
                .contains("skill file escapes skill directory")
        );
    }

    #[test]
    fn rejects_parent_relative_path() {
        let skill = json!({
            "id": "bad-skill",
            "referenced_content": [
                {"name": "secret", "relativePath": "../outside", "content": "no"}
            ]
        });
        let temp = TempDir::new().unwrap();

        let err = skill_to_directory(temp.path(), &skill).unwrap_err();

        assert!(err.to_string().contains("unsafe referenced content"));
    }

    #[cfg(unix)]
    fn symlink_file(source: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn symlink_file(source: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(source, link)
    }
}
