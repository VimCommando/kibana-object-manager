//! Filesystem writer for the stable Kibana bundle layout.

use crate::kibana::agents::{AgentEntry, AgentsManifest};
use crate::kibana::saved_objects::{SavedObject, SavedObjectsManifest};
use crate::kibana::skills::{SkillEntry, SkillsManifest, skill_to_directory};
use crate::kibana::spaces::{SpaceEntry, SpacesManifest};
use crate::kibana::tools::ToolsManifest;
use crate::kibana::workflows::{WorkflowEntry, WorkflowsManifest};
use crate::sync::{SpaceBundle, SyncBundle};
use crate::{Error, Result, ResultContext};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct FilesystemWriter {
    root: PathBuf,
}

impl FilesystemWriter {
    pub(crate) fn create(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create bundle root: {}", root.display()))?;
        Ok(Self { root })
    }

    pub(crate) fn write(&self, bundle: &SyncBundle) -> Result<()> {
        std::fs::create_dir_all(&self.root)
            .with_context(|| format!("Failed to create bundle root: {}", self.root.display()))?;

        if !bundle.spaces.is_empty() {
            self.write_spaces(&bundle.spaces)?;
        }
        for (space_id, space_bundle) in &bundle.by_space {
            self.write_space_bundle(space_id, space_bundle)?;
        }
        Ok(())
    }

    fn write_space_bundle(&self, space_id: &str, bundle: &SpaceBundle) -> Result<()> {
        let manifest_dir = self.manifest_dir(space_id);

        if !bundle.saved_objects.is_empty() {
            saved_objects_manifest(&bundle.saved_objects)?
                .write(manifest_dir.join("saved_objects.json"))?;
            write_json_values(
                &self.resource_dir(space_id, "objects"),
                &bundle.saved_objects,
                saved_object_relative_path,
            )?;
        }
        if !bundle.workflows.is_empty() {
            workflows_manifest(&bundle.workflows)?.write(manifest_dir.join("workflows.yml"))?;
            write_json_values(
                &self.resource_dir(space_id, "workflows"),
                &bundle.workflows,
                workflow_resource_file_name,
            )?;
        }
        if !bundle.agents.is_empty() {
            agents_manifest(&bundle.agents)?.write(manifest_dir.join("agents.yml"))?;
            write_json_values(
                &self.resource_dir(space_id, "agents"),
                &bundle.agents,
                named_resource_file_name,
            )?;
        }
        if !bundle.tools.is_empty() {
            tools_manifest(&bundle.tools)?.write(manifest_dir.join("tools.yml"))?;
            write_json_values(
                &self.resource_dir(space_id, "tools"),
                &bundle.tools,
                named_resource_file_name,
            )?;
        }
        if !bundle.skills.is_empty() {
            skills_manifest(&bundle.skills)?.write(manifest_dir.join("skills.yml"))?;
            write_skill_directories(&self.resource_dir(space_id, "skills"), &bundle.skills)?;
        }
        Ok(())
    }

    fn write_spaces(&self, spaces: &[Value]) -> Result<()> {
        let mut entries = Vec::with_capacity(spaces.len());
        for space in spaces {
            if !space.is_object() {
                return Err(Error::message("space must be a JSON object"));
            }
            let id = required_str(space, "id", "space")?;
            let name = optional_str(space, "name").unwrap_or(id);
            entries.push(SpaceEntry::new(id.to_string(), name.to_string()));
        }
        SpacesManifest::with_spaces(entries).write(self.root.join("spaces.yml"))?;
        for space in spaces {
            let id = required_str(space, "id", "space")?;
            let mut definition = space.clone();
            let object = definition
                .as_object_mut()
                .ok_or(Error::MissingResourceId { resource: "space" })?;
            object.insert("id".to_string(), Value::String(id.to_string()));
            if object.get("name").and_then(Value::as_str).is_none() {
                object.insert("name".to_string(), Value::String(id.to_string()));
            }
            write_json_file(&self.root.join(id).join("space.json"), &definition)?;
        }
        Ok(())
    }

    fn manifest_dir(&self, space_id: &str) -> PathBuf {
        self.root.join(space_id).join("manifest")
    }

    fn resource_dir(&self, space_id: &str, resource: &str) -> PathBuf {
        self.root.join(space_id).join(resource)
    }
}

fn write_json_values(
    root: &Path,
    values: &[Value],
    relative_path: fn(&Value) -> Result<PathBuf>,
) -> Result<()> {
    for value in values {
        write_json_file(&root.join(relative_path(value)?), value)?;
    }
    Ok(())
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(value)
        .with_context(|| format!("Failed to serialize JSON resource: {}", path.display()))?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write JSON resource: {}", path.display()))
}

fn write_skill_directories(root: &Path, values: &[Value]) -> Result<()> {
    for value in values {
        skill_to_directory(root, value)?;
    }
    Ok(())
}

fn saved_object_relative_path(value: &Value) -> Result<PathBuf> {
    let object_type = required_str(value, "type", "saved object")?;
    let id = required_str(value, "id", "saved object")?;
    Ok(PathBuf::from(sanitize_path_component(object_type))
        .join(format!("{}.json", sanitize_path_component(id))))
}

fn named_resource_file_name(value: &Value) -> Result<PathBuf> {
    let id = required_str(value, "id", "resource")?;
    let stem = optional_str(value, "name")
        .filter(|name| *name != id)
        .map(|name| {
            format!(
                "{}--{}",
                sanitize_path_component(name),
                sanitize_path_component(id)
            )
        })
        .unwrap_or_else(|| sanitize_path_component(id));
    Ok(PathBuf::from(format!("{stem}.json")))
}

fn workflow_resource_file_name(value: &Value) -> Result<PathBuf> {
    let id = required_str(value, "id", "workflow")?;
    let stem = optional_str(value, "name")
        .filter(|name| *name != id)
        .map(|name| {
            format!(
                "{}--{}",
                sanitize_workflow_file_stem(name),
                sanitize_workflow_file_stem(id)
            )
        })
        .unwrap_or_else(|| sanitize_workflow_file_stem(id));
    Ok(PathBuf::from(format!("{stem}.json")))
}

fn saved_objects_manifest(values: &[Value]) -> Result<SavedObjectsManifest> {
    let mut manifest = SavedObjectsManifest::new();
    for value in values {
        manifest.add_object(SavedObject::new(
            required_str(value, "type", "saved object")?,
            required_str(value, "id", "saved object")?,
        ));
    }
    manifest.sort();
    Ok(manifest)
}

fn workflows_manifest(values: &[Value]) -> Result<WorkflowsManifest> {
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let id = required_str(value, "id", "workflow")?;
        entries.push(WorkflowEntry::new(
            id,
            optional_str(value, "name").unwrap_or(id),
        ));
    }
    Ok(WorkflowsManifest::with_workflows(entries))
}

fn agents_manifest(values: &[Value]) -> Result<AgentsManifest> {
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let id = required_str(value, "id", "agent")?;
        entries.push(AgentEntry::new(
            id,
            optional_str(value, "name").unwrap_or(id),
        ));
    }
    Ok(AgentsManifest::with_agents(entries))
}

fn tools_manifest(values: &[Value]) -> Result<ToolsManifest> {
    let tools = values
        .iter()
        .map(|value| required_str(value, "id", "tool").map(ToOwned::to_owned))
        .collect::<Result<Vec<_>>>()?;
    Ok(ToolsManifest::with_tools(tools))
}

fn skills_manifest(values: &[Value]) -> Result<SkillsManifest> {
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let id = required_str(value, "id", "skill")?;
        entries.push(SkillEntry::new(
            id,
            optional_str(value, "name").unwrap_or(id),
        ));
    }
    Ok(SkillsManifest::with_skills(entries))
}

fn required_str<'a>(
    value: &'a Value,
    field: &'static str,
    resource: &'static str,
) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(if field == "id" {
            Error::MissingResourceId { resource }
        } else {
            Error::MissingField { field }
        })
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn sanitize_path_component(value: &str) -> String {
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
    if sanitized.is_empty() {
        "unnamed".to_string()
    } else {
        sanitized
    }
}

fn sanitize_workflow_file_stem(value: &str) -> String {
    let stem = sanitize_path_component(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase();
    if stem.is_empty() {
        "unnamed".to_string()
    } else {
        stem
    }
}
