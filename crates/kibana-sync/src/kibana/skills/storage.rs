//! Markdown-first Skills filesystem representation.

use crate::{Error, Result, ResultContext};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

pub(crate) const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillFrontmatter {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawSkillFrontmatter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tool_ids: Vec<String>,
    #[serde(default)]
    experimental: Option<bool>,
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
    validate_skill_id(id)?;
    Ok(id.to_string())
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
    let directory_metadata = std::fs::symlink_metadata(directory)
        .with_context(|| format!("Failed to inspect skill directory: {}", directory.display()))?;
    if directory_metadata.file_type().is_symlink() {
        return Err(Error::message(format!(
            "skill directory cannot be a symlink: {}",
            directory.display()
        )));
    }
    let canonical_directory = directory
        .canonicalize()
        .with_context(|| format!("Failed to resolve skill directory: {}", directory.display()))?;
    let skill_file = directory.join(SKILL_FILE);
    if std::fs::symlink_metadata(&skill_file)
        .with_context(|| format!("Failed to inspect skill file: {}", skill_file.display()))?
        .file_type()
        .is_symlink()
    {
        return Err(Error::message(format!(
            "skill file cannot be a symlink: {}",
            skill_file.display()
        )));
    }
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
    let mut files = Vec::new();
    collect_reference_files(&canonical_directory, directory, &mut files)?;
    files.sort();
    let referenced_files = files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(directory)
                .map_err(|_| {
                    Error::message(format!(
                        "referenced content escaped skill directory: {}",
                        path.display()
                    ))
                })?
                .to_path_buf();
            let content = std::fs::read_to_string(&path).with_context(|| {
                format!("Failed to read referenced content: {}", path.display())
            })?;
            Ok((relative, content))
        })
        .collect::<Result<Vec<_>>>()?;
    skill_files_to_directory(&content, referenced_files)
}

pub fn skill_to_value(directory: &Path, include_id: bool) -> Result<Value> {
    Ok(read_skill_directory(directory)?.to_value(include_id))
}

pub(crate) fn skill_files_to_value(
    skill_markdown: &str,
    referenced_files: impl IntoIterator<Item = (PathBuf, String)>,
    include_id: bool,
) -> Result<Value> {
    Ok(skill_files_to_directory(skill_markdown, referenced_files)?.to_value(include_id))
}

fn skill_files_to_directory(
    skill_markdown: &str,
    referenced_files: impl IntoIterator<Item = (PathBuf, String)>,
) -> Result<SkillDirectory> {
    let (frontmatter, body) = parse_skill_markdown(skill_markdown)?;
    let mut referenced_content = referenced_files
        .into_iter()
        .map(|(relative, content)| {
            let parent = relative.parent().unwrap_or_else(|| Path::new(""));
            Ok(ReferencedContent {
                name: relative
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .ok_or_else(|| Error::message("referenced content filename is not UTF-8"))?
                    .to_string(),
                relative_path: path_to_api_relative_path(parent),
                content,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    referenced_content.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(SkillDirectory {
        frontmatter,
        content: body.to_string(),
        referenced_content,
    })
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

fn validate_skill_id(id: &str) -> Result<()> {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return Err(invalid_skill_id());
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(invalid_skill_id());
    }

    let mut last = first;
    for character in chars {
        if !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && character != '-'
            && character != '_'
        {
            return Err(invalid_skill_id());
        }
        last = character;
    }

    if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
        return Err(invalid_skill_id());
    }

    Ok(())
}

fn invalid_skill_id() -> Error {
    Error::message(
        "ID must start and end with a letter or number, and contain only lowercase letters, numbers, hyphens, and underscores",
    )
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
    let raw_frontmatter: RawSkillFrontmatter =
        yaml_serde::from_str(yaml).context("Failed to parse skill frontmatter")?;
    let id = raw_frontmatter
        .id
        .filter(|id| !id.trim().is_empty())
        .ok_or(Error::MissingResourceId { resource: "skill" })?;
    validate_skill_id(&id)?;
    let frontmatter = SkillFrontmatter {
        id,
        name: raw_frontmatter.name,
        description: raw_frontmatter.description,
        tool_ids: raw_frontmatter.tool_ids,
        experimental: raw_frontmatter.experimental,
    };
    Ok((frontmatter, body))
}

fn write_referenced_content(root: &Path, entries: &[ReferencedContent]) -> Result<()> {
    let mut seen_paths = BTreeSet::new();

    for entry in entries {
        let relative_dir = safe_relative_dir(&entry.relative_path)?;
        let sanitized_name = sanitize_path_component(&entry.name);
        let file_name = format!("{sanitized_name}.md");
        let relative_file = relative_dir.join(file_name);
        let comparable_metadata_path = relative_file.to_string_lossy().to_lowercase();
        if !seen_paths.insert(comparable_metadata_path) {
            return Err(Error::message(format!(
                "duplicate referenced content path after sanitization: {}",
                relative_file.display()
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
    }
    Ok(())
}

fn collect_reference_files(
    canonical_root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to inspect path: {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(Error::message(format!(
                "path uses symlink traversal inside skill directory: {}",
                path.display()
            )));
        }

        let canonical = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;
        if !canonical.starts_with(canonical_root) {
            return Err(Error::message(format!(
                "path escapes skill directory: {}",
                path.display()
            )));
        }

        if metadata.is_dir() {
            collect_reference_files(canonical_root, &path, files)?;
        } else if path.file_name().and_then(|name| name.to_str()) != Some(SKILL_FILE) {
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
            Some(skill_directory_name(&skill).unwrap().as_str())
        );
        assert!(dir.join("SKILL.md").exists());
        assert!(dir.join("overview.md").exists());
        assert!(dir.join("examples/query.md").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("overview.md")).unwrap(),
            "Root ref\n"
        );

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
        let markdown = std::fs::read_to_string(dir.join(SKILL_FILE)).unwrap();

        assert!(projected.get("id").is_none());
        assert_eq!(projected["tool_ids"], json!([]));
        assert_eq!(projected["referenced_content"], json!([]));
        assert!(!markdown.contains(": null"));
    }

    #[test]
    fn rejects_invalid_skill_ids() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "..",
            "name": "Dot Dot",
            "content": "Body\n"
        });

        let err = skill_to_directory(temp.path(), &skill).unwrap_err();

        assert!(err.to_string().contains("ID must start and end"));
        assert!(!temp.path().join(SKILL_FILE).exists());
    }

    #[test]
    fn skill_directory_name_uses_valid_skill_id_directly() {
        let skill = json!({"id": "skill_prod-1"});

        let name = skill_directory_name(&skill).unwrap();

        assert_eq!(name, "skill_prod-1");
    }

    #[test]
    fn rejects_uppercase_skill_ids() {
        let skill = json!({"id": "Skill-A"});

        let err = skill_directory_name(&skill).unwrap_err();

        assert!(err.to_string().contains("only lowercase letters"));
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
    fn missing_frontmatter_id_is_missing_resource_id() {
        let markdown = "---\nname: Missing ID\n---\nBody\n";

        let err = parse_skill_markdown(markdown).unwrap_err();

        assert!(matches!(
            err,
            crate::Error::MissingResourceId { resource: "skill" }
        ));
    }

    #[test]
    fn whitespace_frontmatter_id_is_missing_resource_id() {
        let markdown = "---\nid: \"  \"\nname: Blank ID\n---\nBody\n";

        let err = parse_skill_markdown(markdown).unwrap_err();

        assert!(matches!(
            err,
            crate::Error::MissingResourceId { resource: "skill" }
        ));
    }

    #[test]
    fn derives_referenced_content_values_from_sanitized_paths() {
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
        assert_eq!(
            std::fs::read_to_string(dir.join("examples_prod/query_prod.md")).unwrap(),
            "from logs\n"
        );

        let projected = skill_to_value(&dir, true).unwrap();
        assert_eq!(projected["referenced_content"][0]["name"], "query_prod");
        assert_eq!(
            projected["referenced_content"][0]["relativePath"],
            "./examples_prod"
        );
        assert_eq!(projected["referenced_content"][0]["content"], "from logs\n");
    }

    #[test]
    fn collects_non_markdown_referenced_content() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skill");
        std::fs::create_dir(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join(SKILL_FILE),
            "---\nid: skill\nname: Skill\n---\nInstructions\n",
        )
        .unwrap();
        std::fs::create_dir(skill_dir.join("examples")).unwrap();
        std::fs::write(skill_dir.join("examples/query.json"), r#"{"query":"*"}"#).unwrap();
        std::fs::write(skill_dir.join("notes.txt"), "Notes\n").unwrap();

        let projected = skill_to_value(&skill_dir, true).unwrap();

        assert_eq!(projected["referenced_content"].as_array().unwrap().len(), 2);
        assert_eq!(projected["referenced_content"][0]["name"], "notes");
        assert_eq!(projected["referenced_content"][0]["relativePath"], "");
        assert_eq!(projected["referenced_content"][1]["name"], "query");
        assert_eq!(
            projected["referenced_content"][1]["relativePath"],
            "./examples"
        );
        assert_eq!(
            projected["referenced_content"][1]["content"],
            r#"{"query":"*"}"#
        );
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

        assert!(err.to_string().contains("skill file cannot be a symlink"));
    }

    #[test]
    fn rejects_skill_directory_symlink() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join(SKILL_FILE), "---\nid: skill\n---\nBody\n").unwrap();
        let link = temp.path().join("link");
        symlink_dir(&target, &link).unwrap();

        let err = read_skill_directory(&link).unwrap_err();

        assert!(
            err.to_string()
                .contains("skill directory cannot be a symlink")
        );
    }

    #[test]
    fn rejects_referenced_content_symlink_directory() {
        let temp = TempDir::new().unwrap();
        let skill = json!({
            "id": "symlink-skill",
            "name": "Symlink Skill",
            "content": "Body\n"
        });
        let dir = skill_to_directory(temp.path(), &skill).unwrap();
        symlink_dir(&dir, &dir.join("loop")).unwrap();

        let err = skill_to_value(&dir, true).unwrap_err();

        assert!(err.to_string().contains("symlink traversal"));
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

    #[cfg(unix)]
    fn symlink_dir(source: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn symlink_dir(source: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(source, link)
    }
}
