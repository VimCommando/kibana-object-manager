//! Explicit filesystem bundle support for Kibana sync resources.
//!
//! These helpers operate only on caller-provided paths. They do not discover
//! project roots, read environment variables, initialize logging, or apply CLI
//! migration policy.

use crate::kibana::agents::{AgentEntry, AgentsManifest};
use crate::kibana::saved_objects::{SavedObject, SavedObjectsManifest};
use crate::kibana::skills::{SkillEntry, SkillsManifest, skill_to_directory, skill_to_value};
use crate::kibana::spaces::{SpaceEntry, SpacesManifest};
use crate::kibana::tools::ToolsManifest;
use crate::kibana::workflows::{WorkflowEntry, WorkflowsManifest};
use crate::sync::{SpaceBundle, SyncBundle, SyncSelection};
use crate::{Error, Result, ResultContext};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::kibana::skills::skill_directory_name;

/// Path-explicit reader and writer for version-controlled Kibana asset bundles.
///
/// The stable layout is:
///
/// ```text
/// <root>/
///   spaces.yml
///   <space>/
///     manifest/
///       saved_objects.json
///       workflows.yml
///       agents.yml
///       tools.yml
///     objects/
///     workflows/
///     agents/
///     tools/
///     skills/
/// ```
#[derive(Clone, Debug)]
pub struct KibanaFsBundle {
    root: PathBuf,
}

impl KibanaFsBundle {
    /// Reference a filesystem bundle rooted at a caller-provided path.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if !root.exists() {
            return Err(Error::message(format!(
                "filesystem bundle root does not exist: {}",
                root.display()
            )));
        }

        Ok(Self { root })
    }

    /// Create or reference a filesystem bundle rooted at a caller-provided path.
    pub fn create(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create bundle root: {}", root.display()))?;
        Ok(Self { root })
    }

    /// Return the caller-provided bundle root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read all resource families discoverable in the bundle.
    pub fn read_all(&self) -> Result<SyncBundle> {
        let spaces = self.discover_space_ids()?;
        let selection = SyncSelection {
            spaces,
            saved_objects: Some(SavedObjectsManifest::new()),
            include_spaces: self.spaces_manifest_path().exists(),
            include_workflows: true,
            include_agents: true,
            include_tools: true,
            include_skills: true,
        };

        self.read(&selection)
    }

    /// Read selected resource families into a storage-neutral sync bundle.
    pub fn read(&self, selection: &SyncSelection) -> Result<SyncBundle> {
        let mut bundle = SyncBundle::default();

        if selection.include_spaces {
            bundle.spaces = self.read_spaces()?;
        }

        for space_id in &selection.spaces {
            let mut space_bundle = SpaceBundle::default();

            if let Some(selection_manifest) = &selection.saved_objects {
                space_bundle.saved_objects =
                    self.read_saved_objects(space_id, selection_manifest)?;
            }

            if selection.include_workflows {
                space_bundle.workflows = self.read_workflows(space_id)?;
            }

            if selection.include_agents {
                space_bundle.agents = self.read_agents(space_id)?;
            }

            if selection.include_tools {
                space_bundle.tools = self.read_tools(space_id)?;
            }

            if selection.include_skills {
                space_bundle.skills = self.read_skills(space_id)?;
            }

            bundle.by_space.insert(space_id.clone(), space_bundle);
        }

        Ok(bundle)
    }

    /// Write a storage-neutral sync bundle to the stable filesystem layout.
    pub fn write(&self, bundle: &SyncBundle) -> Result<()> {
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
            let manifest = saved_objects_manifest(&bundle.saved_objects)?;
            manifest.write(manifest_dir.join("saved_objects.json"))?;
            write_json_values(
                &self.objects_dir(space_id),
                &bundle.saved_objects,
                saved_object_relative_path,
            )?;
        }

        if !bundle.workflows.is_empty() {
            let manifest = workflows_manifest(&bundle.workflows)?;
            manifest.write(manifest_dir.join("workflows.yml"))?;
            write_json_values(
                &self.workflows_dir(space_id),
                &bundle.workflows,
                workflow_resource_file_name,
            )?;
        }

        if !bundle.agents.is_empty() {
            let manifest = agents_manifest(&bundle.agents)?;
            manifest.write(manifest_dir.join("agents.yml"))?;
            write_json_values(
                &self.agents_dir(space_id),
                &bundle.agents,
                named_resource_file_name,
            )?;
        }

        if !bundle.tools.is_empty() {
            let manifest = tools_manifest(&bundle.tools)?;
            manifest.write(manifest_dir.join("tools.yml"))?;
            write_json_values(
                &self.tools_dir(space_id),
                &bundle.tools,
                named_resource_file_name,
            )?;
        }

        if !bundle.skills.is_empty() {
            let manifest = skills_manifest(&bundle.skills)?;
            manifest.write(manifest_dir.join("skills.yml"))?;
            write_skill_directories(&self.skills_dir(space_id), &bundle.skills)?;
        }

        Ok(())
    }

    fn read_saved_objects(
        &self,
        space_id: &str,
        selection_manifest: &SavedObjectsManifest,
    ) -> Result<Vec<Value>> {
        let values = read_json_dir(&self.objects_dir(space_id))?;
        let manifest_path = self.saved_objects_manifest_path(space_id);

        let manifest = if manifest_path.exists() {
            Some(SavedObjectsManifest::read(manifest_path)?)
        } else if selection_manifest.objects.is_empty() {
            None
        } else {
            Some(selection_manifest.clone())
        };

        let Some(manifest) = manifest else {
            return Ok(values);
        };

        manifest
            .objects
            .iter()
            .map(|object| {
                values
                    .iter()
                    .find(|value| saved_object_matches(value, &object.object_type, &object.id))
                    .cloned()
                    .ok_or_else(|| {
                        missing_manifest_resource(
                            "saved object",
                            format!("{}/{}", object.object_type, object.id),
                            &self.objects_dir(space_id),
                        )
                    })
            })
            .collect()
    }

    fn read_workflows(&self, space_id: &str) -> Result<Vec<Value>> {
        let values = read_json_dir(&self.workflows_dir(space_id))?;
        let manifest_path = self.workflows_manifest_path(space_id);
        if !manifest_path.exists() {
            return Ok(values);
        }

        let manifest = WorkflowsManifest::read(manifest_path)?;
        manifest
            .workflows
            .iter()
            .map(|entry| {
                find_named_resource(&values, &entry.id, Some(&entry.name)).ok_or_else(|| {
                    missing_manifest_resource("workflow", &entry.id, &self.workflows_dir(space_id))
                })
            })
            .collect()
    }

    fn read_agents(&self, space_id: &str) -> Result<Vec<Value>> {
        let values = read_json_dir(&self.agents_dir(space_id))?;
        let manifest_path = self.agents_manifest_path(space_id);
        if !manifest_path.exists() {
            return Ok(values);
        }

        let manifest = AgentsManifest::read(manifest_path)?;
        manifest
            .agents
            .iter()
            .map(|entry| {
                find_named_resource(&values, &entry.id, Some(&entry.name)).ok_or_else(|| {
                    missing_manifest_resource("agent", &entry.id, &self.agents_dir(space_id))
                })
            })
            .collect()
    }

    fn read_tools(&self, space_id: &str) -> Result<Vec<Value>> {
        let values = read_json_dir(&self.tools_dir(space_id))?;
        let manifest_path = self.tools_manifest_path(space_id);
        if !manifest_path.exists() {
            return Ok(values);
        }

        let manifest = ToolsManifest::read(manifest_path)?;
        manifest
            .tools
            .iter()
            .map(|tool_id| {
                find_named_resource(&values, tool_id, None).ok_or_else(|| {
                    missing_manifest_resource("tool", tool_id, &self.tools_dir(space_id))
                })
            })
            .collect()
    }

    fn read_skills(&self, space_id: &str) -> Result<Vec<Value>> {
        let root = self.skills_dir(space_id);
        let manifest_path = self.skills_manifest_path(space_id);
        let manifest = if manifest_path.exists() {
            Some(SkillsManifest::read(manifest_path)?)
        } else {
            None
        };

        if !root.exists() {
            if let Some(manifest) = manifest
                && let Some(entry) = manifest.skills.first()
            {
                return Err(missing_skill_manifest_resource(&entry.id, &root));
            }
            return Ok(Vec::new());
        }

        let mut directories = Vec::new();
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::message(format!(
                    "skill directory cannot be a symlink: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() && path.join("SKILL.md").exists() {
                directories.push(path);
            }
        }
        directories.sort();

        let values = directories
            .into_iter()
            .map(|directory| skill_to_value(&directory, true))
            .collect::<Result<Vec<_>>>()?;

        let Some(manifest) = manifest else {
            return Ok(values);
        };
        manifest
            .skills
            .iter()
            .map(|entry| {
                values
                    .iter()
                    .find(|value| value.get("id").and_then(|id| id.as_str()) == Some(&entry.id))
                    .cloned()
                    .ok_or_else(|| missing_skill_manifest_resource(&entry.id, &root))
            })
            .collect()
    }

    fn read_spaces(&self) -> Result<Vec<Value>> {
        let path = self.spaces_manifest_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let manifest = SpacesManifest::read(path)?;
        Ok(manifest
            .spaces
            .into_iter()
            .map(|space| serde_json::json!({"id": space.id, "name": space.name}))
            .collect())
    }

    fn write_spaces(&self, spaces: &[Value]) -> Result<()> {
        let mut entries = Vec::with_capacity(spaces.len());

        for space in spaces {
            let id = required_str(space, "id", "space")?;
            let name = optional_str(space, "name").unwrap_or(id);
            entries.push(SpaceEntry::new(id.to_string(), name.to_string()));
        }

        SpacesManifest::with_spaces(entries).write(self.spaces_manifest_path())
    }

    fn discover_space_ids(&self) -> Result<Vec<String>> {
        let mut ids = BTreeSet::new();

        let spaces_path = self.spaces_manifest_path();
        if spaces_path.exists() {
            let manifest = SpacesManifest::read(spaces_path)?;
            ids.extend(manifest.spaces.into_iter().map(|space| space.id));
        }

        if self.root.exists() {
            for entry in std::fs::read_dir(&self.root)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };

                if self.has_space_resources(name) {
                    ids.insert(name.to_string());
                }
            }
        }

        Ok(ids.into_iter().collect())
    }

    fn has_space_resources(&self, space_id: &str) -> bool {
        self.objects_dir(space_id).exists()
            || self.workflows_dir(space_id).exists()
            || self.agents_dir(space_id).exists()
            || self.tools_dir(space_id).exists()
            || self.skills_dir(space_id).exists()
            || self.manifest_dir(space_id).exists()
    }

    fn spaces_manifest_path(&self) -> PathBuf {
        self.root.join("spaces.yml")
    }

    fn space_dir(&self, space_id: &str) -> PathBuf {
        self.root.join(space_id)
    }

    fn manifest_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("manifest")
    }

    fn saved_objects_manifest_path(&self, space_id: &str) -> PathBuf {
        self.manifest_dir(space_id).join("saved_objects.json")
    }

    fn workflows_manifest_path(&self, space_id: &str) -> PathBuf {
        self.manifest_dir(space_id).join("workflows.yml")
    }

    fn agents_manifest_path(&self, space_id: &str) -> PathBuf {
        self.manifest_dir(space_id).join("agents.yml")
    }

    fn tools_manifest_path(&self, space_id: &str) -> PathBuf {
        self.manifest_dir(space_id).join("tools.yml")
    }

    fn skills_manifest_path(&self, space_id: &str) -> PathBuf {
        self.manifest_dir(space_id).join("skills.yml")
    }

    fn objects_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("objects")
    }

    fn workflows_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("workflows")
    }

    fn agents_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("agents")
    }

    fn tools_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("tools")
    }

    fn skills_dir(&self, space_id: &str) -> PathBuf {
        self.space_dir(space_id).join("skills")
    }
}

fn read_json_dir(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_json_files(path, &mut files)?;
    files.sort();

    files
        .into_iter()
        .map(|file| {
            let content = std::fs::read_to_string(&file)
                .with_context(|| format!("Failed to read JSON resource: {}", file.display()))?;
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON resource: {}", file.display()))
        })
        .collect()
}

fn collect_json_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            files.push(path);
        }
    }

    Ok(())
}

fn saved_object_matches(value: &Value, object_type: &str, id: &str) -> bool {
    value.get("type").and_then(|field| field.as_str()) == Some(object_type)
        && value.get("id").and_then(|field| field.as_str()) == Some(id)
}

fn find_named_resource(values: &[Value], id: &str, name: Option<&str>) -> Option<Value> {
    values
        .iter()
        .find(|value| value.get("id").and_then(|field| field.as_str()) == Some(id))
        .or_else(|| {
            name.and_then(|name| {
                values
                    .iter()
                    .find(|value| value.get("name").and_then(|field| field.as_str()) == Some(name))
            })
        })
        .cloned()
}

fn missing_manifest_resource(
    resource: &str,
    id: impl std::fmt::Display,
    directory: &Path,
) -> Error {
    Error::message(format!(
        "{resource} '{id}' is listed in the manifest but no matching JSON resource was found under {}",
        directory.display()
    ))
}

fn missing_skill_manifest_resource(id: impl std::fmt::Display, directory: &Path) -> Error {
    Error::message(format!(
        "skill '{id}' is listed in the manifest but no matching skills/<skill-directory>/SKILL.md resource was found under {}",
        directory.display()
    ))
}

fn write_json_values(
    root: &Path,
    values: &[Value],
    relative_path: fn(&Value) -> Result<PathBuf>,
) -> Result<()> {
    for value in values {
        let path = root.join(relative_path(value)?);
        write_json_file(&path, value)?;
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
        let name = optional_str(value, "name").unwrap_or(id);
        entries.push(WorkflowEntry::new(id, name));
    }
    Ok(WorkflowsManifest::with_workflows(entries))
}

fn agents_manifest(values: &[Value]) -> Result<AgentsManifest> {
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let id = required_str(value, "id", "agent")?;
        let name = optional_str(value, "name").unwrap_or(id);
        entries.push(AgentEntry::new(id, name));
    }
    Ok(AgentsManifest::with_agents(entries))
}

fn tools_manifest(values: &[Value]) -> Result<ToolsManifest> {
    let mut tools = Vec::with_capacity(values.len());
    for value in values {
        tools.push(required_str(value, "id", "tool")?.to_string());
    }
    Ok(ToolsManifest::with_tools(tools))
}

fn skills_manifest(values: &[Value]) -> Result<SkillsManifest> {
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        let id = required_str(value, "id", "skill")?;
        let name = optional_str(value, "name").unwrap_or(id);
        entries.push(SkillEntry::new(id, name));
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
        .and_then(|field| field.as_str())
        .ok_or(if field == "id" {
            Error::MissingResourceId { resource }
        } else {
            Error::MissingField { field }
        })
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(|field| field.as_str())
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
    let sanitized = sanitize_path_component(value);
    let stem = sanitized
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{SpaceBundle, SyncBundle, SyncSelection};
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn round_trips_bundle_through_explicit_path() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let mut bundle = SyncBundle {
            spaces: vec![json!({"id": "default", "name": "Default"})],
            ..SyncBundle::default()
        };
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                saved_objects: vec![json!({
                    "type": "dashboard",
                    "id": "dashboard-1",
                    "attributes": {"title": "Sales Dashboard"}
                })],
                workflows: vec![json!({"id": "workflow-1", "name": "Daily Workflow"})],
                agents: vec![json!({"id": "agent-1", "name": "Support Agent"})],
                tools: vec![json!({"id": "tool-1", "name": "Search Tool"})],
                skills: vec![json!({
                    "id": "skill-1",
                    "name": "Skill One",
                    "description": "Skill description",
                    "content": "Main skill body\n",
                    "tool_ids": ["tool-1"],
                    "referenced_content": [
                        {"name": "query", "relativePath": "./examples", "content": "from logs\n"}
                    ]
                })],
            },
        );

        let writer = KibanaFsBundle::create(&bundle_path).unwrap();
        writer.write(&bundle).unwrap();

        assert!(bundle_path.join("spaces.yml").exists());
        assert!(
            bundle_path
                .join("default/manifest/saved_objects.json")
                .exists()
        );
        assert!(
            bundle_path
                .join("default/objects/dashboard/dashboard-1.json")
                .exists()
        );
        assert!(
            bundle_path
                .join("default/workflows/daily_workflow--workflow-1.json")
                .exists()
        );
        assert!(
            bundle_path
                .join("default/agents/Support Agent--agent-1.json")
                .exists()
        );
        assert!(
            bundle_path
                .join("default/tools/Search Tool--tool-1.json")
                .exists()
        );
        assert!(bundle_path.join("default/manifest/skills.yml").exists());
        let skill_dir = skill_directory_name(&bundle.by_space["default"].skills[0]).unwrap();
        assert!(
            bundle_path
                .join("default/skills")
                .join(&skill_dir)
                .join("SKILL.md")
                .exists()
        );
        assert!(
            bundle_path
                .join("default/skills")
                .join(&skill_dir)
                .join("examples/query.md")
                .exists()
        );

        let reader = KibanaFsBundle::open(&bundle_path).unwrap();
        let read = reader.read_all().unwrap();

        assert_eq!(read.spaces, bundle.spaces);
        assert_eq!(
            read.by_space["default"].saved_objects,
            bundle.by_space["default"].saved_objects
        );
        assert_eq!(
            read.by_space["default"].workflows,
            bundle.by_space["default"].workflows
        );
        assert_eq!(
            read.by_space["default"].agents,
            bundle.by_space["default"].agents
        );
        assert_eq!(
            read.by_space["default"].tools,
            bundle.by_space["default"].tools
        );
        assert_eq!(
            read.by_space["default"].skills,
            bundle.by_space["default"].skills
        );
    }

    #[test]
    fn round_trips_skills_only_bundle() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");

        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                skills: vec![json!({
                    "id": "skill-only",
                    "name": "Skill Only",
                    "description": "Only skill resources",
                    "content": "Main instructions\n",
                    "tool_ids": [],
                    "referenced_content": []
                })],
                ..SpaceBundle::default()
            },
        );

        KibanaFsBundle::create(&bundle_path)
            .unwrap()
            .write(&bundle)
            .unwrap();

        let skill_dir = skill_directory_name(&bundle.by_space["default"].skills[0]).unwrap();
        assert!(
            bundle_path
                .join("default/skills")
                .join(&skill_dir)
                .join("SKILL.md")
                .exists()
        );
        assert!(bundle_path.join("default/manifest/skills.yml").exists());

        let read = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap();

        assert_eq!(
            read.by_space["default"].skills,
            bundle.by_space["default"].skills
        );
        assert!(read.by_space["default"].saved_objects.is_empty());
        assert!(read.by_space["default"].workflows.is_empty());
        assert!(read.by_space["default"].agents.is_empty());
        assert!(read.by_space["default"].tools.is_empty());
    }

    #[test]
    fn read_honors_explicit_selection() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        let fs_bundle = KibanaFsBundle::create(&bundle_path).unwrap();

        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "marketing".to_string(),
            SpaceBundle {
                saved_objects: vec![json!({"type": "dashboard", "id": "dashboard-1"})],
                workflows: vec![json!({"id": "workflow-1", "name": "Workflow"})],
                agents: vec![json!({"id": "agent-1", "name": "Agent"})],
                tools: vec![json!({"id": "tool-1", "name": "Tool"})],
                skills: vec![json!({"id": "skill-1", "name": "Skill", "content": "Body\n"})],
            },
        );
        fs_bundle.write(&bundle).unwrap();

        let selection = SyncSelection {
            spaces: vec!["marketing".to_string()],
            saved_objects: None,
            include_spaces: false,
            include_workflows: true,
            include_agents: false,
            include_tools: true,
            include_skills: true,
        };

        let read = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read(&selection)
            .unwrap();
        let space = &read.by_space["marketing"];

        assert!(space.saved_objects.is_empty());
        assert_eq!(space.workflows.len(), 1);
        assert!(space.agents.is_empty());
        assert_eq!(space.tools.len(), 1);
        assert_eq!(space.skills.len(), 1);
    }

    #[test]
    fn read_uses_caller_provided_path_only() {
        let temp = TempDir::new().unwrap();
        let first_path = temp.path().join("first");
        let second_path = temp.path().join("second");

        let mut first = SyncBundle::default();
        first.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                tools: vec![json!({"id": "first-tool"})],
                ..SpaceBundle::default()
            },
        );
        KibanaFsBundle::create(&first_path)
            .unwrap()
            .write(&first)
            .unwrap();

        let mut second = SyncBundle::default();
        second.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                tools: vec![json!({"id": "second-tool"})],
                ..SpaceBundle::default()
            },
        );
        KibanaFsBundle::create(&second_path)
            .unwrap()
            .write(&second)
            .unwrap();

        let selection = SyncSelection {
            spaces: vec!["default".to_string()],
            include_tools: true,
            ..SyncSelection::default()
        };
        let read = KibanaFsBundle::open(&second_path)
            .unwrap()
            .read(&selection)
            .unwrap();

        assert_eq!(read.by_space["default"].tools[0]["id"], "second-tool");
    }

    #[test]
    fn read_uses_per_space_manifests_as_source_of_truth() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        let fs_bundle = KibanaFsBundle::create(&bundle_path).unwrap();

        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                saved_objects: vec![
                    json!({"type": "dashboard", "id": "dash-1"}),
                    json!({"type": "dashboard", "id": "dash-2"}),
                ],
                workflows: vec![
                    json!({"id": "workflow-1", "name": "Workflow One"}),
                    json!({"id": "workflow-2", "name": "Workflow Two"}),
                ],
                agents: vec![
                    json!({"id": "agent-1", "name": "Agent One"}),
                    json!({"id": "agent-2", "name": "Agent Two"}),
                ],
                tools: vec![
                    json!({"id": "tool-1", "name": "Tool One"}),
                    json!({"id": "tool-2", "name": "Tool Two"}),
                ],
                skills: vec![
                    json!({"id": "skill-1", "name": "Skill One", "content": "Body one\n"}),
                    json!({"id": "skill-2", "name": "Skill Two", "content": "Body two\n"}),
                ],
            },
        );
        fs_bundle.write(&bundle).unwrap();

        SavedObjectsManifest::with_objects(vec![SavedObject::new("dashboard", "dash-2")])
            .write(bundle_path.join("default/manifest/saved_objects.json"))
            .unwrap();
        WorkflowsManifest::with_workflows(vec![WorkflowEntry::new("workflow-2", "Workflow Two")])
            .write(bundle_path.join("default/manifest/workflows.yml"))
            .unwrap();
        AgentsManifest::with_agents(vec![AgentEntry::new("agent-2", "Agent Two")])
            .write(bundle_path.join("default/manifest/agents.yml"))
            .unwrap();
        ToolsManifest::with_tools(vec!["tool-2".to_string()])
            .write(bundle_path.join("default/manifest/tools.yml"))
            .unwrap();
        SkillsManifest::with_skills(vec![SkillEntry::new("skill-2", "Skill Two")])
            .write(bundle_path.join("default/manifest/skills.yml"))
            .unwrap();

        write_json_file(
            &bundle_path.join("default/tools/extra.json"),
            &json!({"id": "extra-tool", "name": "Extra Tool"}),
        )
        .unwrap();

        let read = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap();
        let space = &read.by_space["default"];

        assert_eq!(space.saved_objects.len(), 1);
        assert_eq!(space.saved_objects[0]["id"], "dash-2");
        assert_eq!(space.workflows.len(), 1);
        assert_eq!(space.workflows[0]["id"], "workflow-2");
        assert_eq!(space.agents.len(), 1);
        assert_eq!(space.agents[0]["id"], "agent-2");
        assert_eq!(space.tools.len(), 1);
        assert_eq!(space.tools[0]["id"], "tool-2");
        assert_eq!(space.skills.len(), 1);
        assert_eq!(space.skills[0]["id"], "skill-2");
    }

    #[test]
    fn manifest_listed_missing_resource_is_an_error() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        let fs_bundle = KibanaFsBundle::create(&bundle_path).unwrap();

        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                tools: vec![json!({"id": "tool-1", "name": "Tool One"})],
                ..SpaceBundle::default()
            },
        );
        fs_bundle.write(&bundle).unwrap();

        ToolsManifest::with_tools(vec!["missing-tool".to_string()])
            .write(bundle_path.join("default/manifest/tools.yml"))
            .unwrap();

        let err = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(err.to_string().contains("missing-tool"));
        assert!(err.to_string().contains("listed in the manifest"));
    }

    #[test]
    fn manifest_listed_missing_skill_is_an_error() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        let fs_bundle = KibanaFsBundle::create(&bundle_path).unwrap();

        let mut bundle = SyncBundle::default();
        bundle.by_space.insert(
            "default".to_string(),
            SpaceBundle {
                skills: vec![json!({"id": "skill-1", "name": "Skill One", "content": "Body\n"})],
                ..SpaceBundle::default()
            },
        );
        fs_bundle.write(&bundle).unwrap();

        SkillsManifest::with_skills(vec![SkillEntry::new("missing-skill", "Missing Skill")])
            .write(bundle_path.join("default/manifest/skills.yml"))
            .unwrap();

        let err = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(err.to_string().contains("missing-skill"));
        assert!(err.to_string().contains("listed in the manifest"));
        assert!(err.to_string().contains("SKILL.md"));
    }

    #[test]
    fn manifest_listed_skill_without_skills_directory_is_an_error() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        KibanaFsBundle::create(&bundle_path).unwrap();

        SkillsManifest::with_skills(vec![SkillEntry::new("missing-skill", "Missing Skill")])
            .write(bundle_path.join("default/manifest/skills.yml"))
            .unwrap();

        let err = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(err.to_string().contains("missing-skill"));
        assert!(err.to_string().contains("listed in the manifest"));
        assert!(err.to_string().contains("SKILL.md"));
    }

    #[test]
    fn symlinked_skill_directory_is_an_error() {
        let temp = TempDir::new().unwrap();
        let bundle_path = temp.path().join("bundle");
        let skills_path = bundle_path.join("default/skills");
        let outside_path = temp.path().join("outside-skill");
        std::fs::create_dir_all(&skills_path).unwrap();
        std::fs::create_dir(&outside_path).unwrap();
        std::fs::write(
            outside_path.join("SKILL.md"),
            "---\nid: outside-skill\n---\nBody\n",
        )
        .unwrap();
        symlink_dir(&outside_path, &skills_path.join("linked")).unwrap();

        let err = KibanaFsBundle::open(&bundle_path)
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("skill directory cannot be a symlink")
        );
    }

    #[test]
    fn missing_root_is_an_error_for_open() {
        let temp = TempDir::new().unwrap();
        let result = KibanaFsBundle::open(temp.path().join("missing"));

        assert!(result.is_err());
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
