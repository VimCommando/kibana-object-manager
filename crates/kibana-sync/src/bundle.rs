//! Generic bundle access for filesystem and in-memory Kibana assets.

use crate::kibana::agents::AgentsManifest;
use crate::kibana::saved_objects::SavedObjectsManifest;
use crate::kibana::skills::{
    SKILL_FILE, SkillsManifest, skill_files_to_value,
};
use crate::kibana::spaces::{SpaceEntry, SpacesManifest};
use crate::kibana::tools::ToolsManifest;
use crate::kibana::workflows::WorkflowsManifest;
use crate::sync::{SpaceBundle, SyncBundle, SyncSelection};
use crate::{Error, Result, ResultContext, json5};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

mod sealed {
    pub trait Sealed {}
}

/// Read operations required by [`KibanaBundle`].
///
/// This trait is sealed. Use [`Filesystem`] or [`Entries`] as the source.
pub trait BundleSource: sealed::Sealed {
    /// Borrowed or owned content returned by this source.
    type Content<'a>: AsRef<[u8]>
    where
        Self: 'a;

    #[doc(hidden)]
    fn is_file(&self, path: &Path) -> bool;
    #[doc(hidden)]
    fn is_dir(&self, path: &Path) -> bool;
    #[doc(hidden)]
    fn read(&self, path: &Path) -> Result<Self::Content<'_>>;
    #[doc(hidden)]
    fn files_under(&self, path: &Path) -> Result<Vec<PathBuf>>;
    #[doc(hidden)]
    fn immediate_directories(&self, path: &Path) -> Result<Vec<PathBuf>>;
    #[doc(hidden)]
    fn display_path(&self, path: &Path) -> String;
    #[doc(hidden)]
    fn validate_skill_directory(&self, _path: &Path) -> Result<()> {
        Ok(())
    }
}

/// A Kibana asset bundle backed by a source type.
#[derive(Clone, Debug)]
pub struct KibanaBundle<S> {
    source: S,
}

/// Filesystem-backed bundle source.
#[derive(Clone, Debug)]
pub struct Filesystem {
    root: PathBuf,
}

/// Entry-backed bundle source with caller-selected byte storage.
#[derive(Clone, Debug)]
pub struct Entries<B> {
    files: BTreeMap<PathBuf, B>,
    directories: BTreeSet<PathBuf>,
}

impl sealed::Sealed for Filesystem {}
impl<B> sealed::Sealed for Entries<B> {}

impl KibanaBundle<Filesystem> {
    /// Open an existing filesystem bundle.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if !root.exists() {
            return Err(Error::message(format!(
                "filesystem bundle root does not exist: {}",
                root.display()
            )));
        }
        if std::fs::symlink_metadata(&root)?.file_type().is_symlink() {
            return Err(bundle_symlink_error(&root));
        }
        Ok(Self {
            source: Filesystem { root },
        })
    }

    /// Create or open a filesystem bundle.
    pub fn create(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create bundle root: {}", root.display()))?;
        if std::fs::symlink_metadata(&root)?.file_type().is_symlink() {
            return Err(bundle_symlink_error(&root));
        }
        Ok(Self {
            source: Filesystem { root },
        })
    }

    /// Return the filesystem bundle root.
    pub fn root(&self) -> &Path {
        &self.source.root
    }

    /// Write a storage-neutral bundle to the stable filesystem layout.
    pub fn write(&self, bundle: &SyncBundle) -> Result<()> {
        crate::fs::FilesystemWriter::create(&self.source.root)?.write(bundle)
    }
}

impl<B: AsRef<[u8]>> KibanaBundle<Entries<B>> {
    /// Construct a bundle from root-relative path and content pairs.
    pub fn from_entries<P>(entries: impl IntoIterator<Item = (P, B)>) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut files = BTreeMap::new();
        let mut directories = BTreeSet::new();

        for (path, content) in entries {
            let original = path.as_ref();
            let normalized = normalize_entry_path(original)?;
            if directories.contains(&normalized) {
                return Err(Error::message(format!(
                    "bundle entry path conflicts with an implicit directory: {}",
                    logical_path(&normalized)
                )));
            }
            if files.insert(normalized.clone(), content).is_some() {
                return Err(Error::message(format!(
                    "duplicate bundle entry path: {}",
                    logical_path(&normalized)
                )));
            }

            let mut parent = normalized.parent();
            while let Some(directory) = parent {
                if directory.as_os_str().is_empty() {
                    break;
                }
                if files.contains_key(directory) {
                    return Err(Error::message(format!(
                        "bundle entry path conflicts with a file: {}",
                        logical_path(directory)
                    )));
                }
                directories.insert(directory.to_path_buf());
                parent = directory.parent();
            }
        }

        Ok(Self {
            source: Entries { files, directories },
        })
    }
}

impl<S: BundleSource> KibanaBundle<S> {
    /// Read all resource families discoverable in the bundle.
    pub fn read_all(&self) -> Result<SyncBundle> {
        let spaces = self.discover_space_ids()?;
        let selection = SyncSelection {
            spaces,
            saved_objects: Some(SavedObjectsManifest::new()),
            include_spaces: self.manifest_is_file(Path::new("spaces.yml"))?,
            include_workflows: true,
            include_agents: true,
            include_tools: true,
            include_skills: true,
        };
        self.read(&selection)
    }

    /// Read selected resource families into a storage-neutral bundle.
    pub fn read(&self, selection: &SyncSelection) -> Result<SyncBundle> {
        let mut bundle = SyncBundle::default();
        if selection.include_spaces {
            bundle.spaces = self.read_spaces()?;
        }

        for space_id in &selection.spaces {
            validate_space_id(space_id)?;
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

    fn read_saved_objects(
        &self,
        space_id: &str,
        selection_manifest: &SavedObjectsManifest,
    ) -> Result<Vec<Value>> {
        let directory = resource_dir(space_id, "objects");
        let values = self.read_json_dir(&directory)?;
        let manifest_path = manifest_path(space_id, "saved_objects.json");
        let manifest = if self.manifest_is_file(&manifest_path)? {
            Some(self.read_json_manifest(&manifest_path, "saved objects")?)
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
                        self.missing_manifest_resource(
                            "saved object",
                            format!("{}/{}", object.object_type, object.id),
                            &directory,
                        )
                    })
            })
            .collect()
    }

    fn read_workflows(&self, space_id: &str) -> Result<Vec<Value>> {
        let directory = resource_dir(space_id, "workflows");
        let values = self.read_json_dir(&directory)?;
        let manifest_path = manifest_path(space_id, "workflows.yml");
        if !self.manifest_is_file(&manifest_path)? {
            return Ok(values);
        }
        let manifest: WorkflowsManifest = self.read_yaml_manifest(&manifest_path, "workflows")?;
        manifest
            .workflows
            .iter()
            .map(|entry| {
                find_named_resource(&values, &entry.id, Some(&entry.name)).ok_or_else(|| {
                    self.missing_manifest_resource("workflow", &entry.id, &directory)
                })
            })
            .collect()
    }

    fn read_agents(&self, space_id: &str) -> Result<Vec<Value>> {
        let directory = resource_dir(space_id, "agents");
        let values = self.read_json_dir(&directory)?;
        let manifest_path = manifest_path(space_id, "agents.yml");
        if !self.manifest_is_file(&manifest_path)? {
            return Ok(values);
        }
        let manifest: AgentsManifest = self.read_yaml_manifest(&manifest_path, "agents")?;
        manifest
            .agents
            .iter()
            .map(|entry| {
                find_named_resource(&values, &entry.id, Some(&entry.name))
                    .ok_or_else(|| self.missing_manifest_resource("agent", &entry.id, &directory))
            })
            .collect()
    }

    fn read_tools(&self, space_id: &str) -> Result<Vec<Value>> {
        let directory = resource_dir(space_id, "tools");
        let values = self.read_json_dir(&directory)?;
        let manifest_path = manifest_path(space_id, "tools.yml");
        if !self.manifest_is_file(&manifest_path)? {
            return Ok(values);
        }
        let manifest: ToolsManifest = self.read_yaml_manifest(&manifest_path, "tools")?;
        manifest
            .tools
            .iter()
            .map(|id| {
                find_named_resource(&values, id, None)
                    .ok_or_else(|| self.missing_manifest_resource("tool", id, &directory))
            })
            .collect()
    }

    fn read_skills(&self, space_id: &str) -> Result<Vec<Value>> {
        let root = resource_dir(space_id, "skills");
        let manifest_path = manifest_path(space_id, "skills.yml");
        let manifest: Option<SkillsManifest> = if self.manifest_is_file(&manifest_path)? {
            Some(self.read_yaml_manifest(&manifest_path, "skills")?)
        } else {
            None
        };

        if !self.source.is_dir(&root) {
            if let Some(entry) = manifest.as_ref().and_then(|value| value.skills.first()) {
                return Err(self.missing_skill_manifest_resource(&entry.id, &root));
            }
            return Ok(Vec::new());
        }

        let mut directories = self.source.immediate_directories(&root)?;
        directories.sort();
        let mut values = Vec::new();
        for directory in directories {
            let skill_file = directory.join(SKILL_FILE);
            if !self.source.is_file(&skill_file) {
                continue;
            }
            self.source.validate_skill_directory(&directory)?;
            let skill_markdown = self.read_text(&skill_file, "skill file")?;
            let mut referenced = Vec::new();
            for file in self.source.files_under(&directory)? {
                if file.file_name().and_then(|value| value.to_str()) == Some(SKILL_FILE) {
                    continue;
                }
                let relative = file
                    .strip_prefix(&directory)
                    .map_err(|_| {
                        Error::message(format!(
                            "referenced content escaped skill directory: {}",
                            self.source.display_path(&file)
                        ))
                    })?
                    .to_path_buf();
                referenced.push((relative, self.read_text(&file, "referenced content")?));
            }
            values.push(skill_files_to_value(
                &skill_markdown,
                referenced,
                true,
            )?);
        }

        let Some(manifest) = manifest else {
            return Ok(values);
        };
        manifest
            .skills
            .iter()
            .map(|entry| {
                values
                    .iter()
                    .find(|value| value.get("id").and_then(Value::as_str) == Some(&entry.id))
                    .cloned()
                    .ok_or_else(|| self.missing_skill_manifest_resource(&entry.id, &root))
            })
            .collect()
    }

    fn read_spaces(&self) -> Result<Vec<Value>> {
        let path = Path::new("spaces.yml");
        if !self.manifest_is_file(path)? {
            return Ok(Vec::new());
        }
        let manifest: SpacesManifest = self.read_yaml_manifest(path, "spaces")?;
        manifest
            .spaces
            .into_iter()
            .map(|space| {
                validate_space_id(&space.id)?;
                self.read_space_definition(&space)
            })
            .collect()
    }

    fn read_space_definition(&self, space: &SpaceEntry) -> Result<Value> {
        let path = Path::new(&space.id).join("space.json");
        if !self.source.is_file(&path) {
            return Ok(serde_json::json!({"id": space.id, "name": space.name}));
        }

        let content = self.source.read(&path).with_context(|| {
            format!(
                "Failed to read space definition: {}",
                self.source.display_path(&path)
            )
        })?;
        let text = utf8(content.as_ref(), &self.source.display_path(&path))?;
        let mut definition = json5::from_json5_str(text).with_context(|| {
            format!(
                "Failed to parse space definition: {}",
                self.source.display_path(&path)
            )
        })?;
        let id = definition.get("id").and_then(Value::as_str);
        if id != Some(space.id.as_str()) {
            return Err(Error::message(format!(
                "space definition {} must contain the manifest id '{}'",
                self.source.display_path(&path),
                space.id
            )));
        }
        if definition.get("name").and_then(Value::as_str).is_none() {
            definition["name"] = Value::String(space.name.clone());
        }
        Ok(definition)
    }

    fn discover_space_ids(&self) -> Result<Vec<String>> {
        let mut ids = BTreeSet::new();
        let spaces_path = Path::new("spaces.yml");
        if self.manifest_is_file(spaces_path)? {
            let manifest: SpacesManifest = self.read_yaml_manifest(spaces_path, "spaces")?;
            ids.extend(manifest.spaces.into_iter().map(|space| space.id));
        }
        for directory in self.source.immediate_directories(Path::new(""))? {
            let Some(id) = directory.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if self.has_space_resources(id) {
                ids.insert(id.to_string());
            }
        }
        Ok(ids.into_iter().collect())
    }

    fn has_space_resources(&self, space_id: &str) -> bool {
        [
            "objects",
            "workflows",
            "agents",
            "tools",
            "skills",
            "manifest",
        ]
        .into_iter()
        .any(|name| self.source.is_dir(&resource_dir(space_id, name)))
    }

    fn manifest_is_file(&self, path: &Path) -> Result<bool> {
        if self.source.is_file(path) {
            return Ok(true);
        }
        if self.source.is_dir(path) {
            return Err(Error::message(format!(
                "bundle manifest must be a file: {}",
                self.source.display_path(path)
            )));
        }
        Ok(false)
    }

    fn read_json_dir(&self, directory: &Path) -> Result<Vec<Value>> {
        if !self.source.is_dir(directory) {
            return Ok(Vec::new());
        }
        let mut files = self.source.files_under(directory)?;
        files.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("json"));
        files.sort();
        files
            .into_iter()
            .map(|path| {
                let content = self.source.read(&path).with_context(|| {
                    format!(
                        "Failed to read JSON resource: {}",
                        self.source.display_path(&path)
                    )
                })?;
                let text = utf8(content.as_ref(), &self.source.display_path(&path))?;
                json5::from_json5_str(text).with_context(|| {
                    format!(
                        "Failed to parse JSON resource: {}",
                        self.source.display_path(&path)
                    )
                })
            })
            .collect()
    }

    fn read_text(&self, path: &Path, resource: &str) -> Result<String> {
        let content = self.source.read(path).with_context(|| {
            format!(
                "Failed to read {resource}: {}",
                self.source.display_path(path)
            )
        })?;
        Ok(utf8(content.as_ref(), &self.source.display_path(path))?.to_string())
    }

    fn read_yaml_manifest<T: DeserializeOwned>(&self, path: &Path, kind: &str) -> Result<T> {
        let content = self.source.read(path).with_context(|| {
            format!(
                "Failed to read {kind} manifest: {}",
                self.source.display_path(path)
            )
        })?;
        let text = utf8(content.as_ref(), &self.source.display_path(path))?;
        yaml_serde::from_str(text).with_context(|| {
            format!(
                "Failed to parse {kind} manifest YAML: {}",
                self.source.display_path(path)
            )
        })
    }

    fn read_json_manifest<T: DeserializeOwned>(&self, path: &Path, kind: &str) -> Result<T> {
        let content = self.source.read(path).with_context(|| {
            format!(
                "Failed to read {kind} manifest: {}",
                self.source.display_path(path)
            )
        })?;
        let text = utf8(content.as_ref(), &self.source.display_path(path))?;
        serde_json::from_str(text).with_context(|| {
            format!(
                "Failed to parse {kind} manifest JSON: {}",
                self.source.display_path(path)
            )
        })
    }

    fn missing_manifest_resource(
        &self,
        resource: &str,
        id: impl std::fmt::Display,
        directory: &Path,
    ) -> Error {
        Error::message(format!(
            "{resource} '{id}' is listed in the manifest but no matching JSON resource was found under {}",
            self.source.display_path(directory)
        ))
    }

    fn missing_skill_manifest_resource(
        &self,
        id: impl std::fmt::Display,
        directory: &Path,
    ) -> Error {
        Error::message(format!(
            "skill '{id}' is listed in the manifest but no matching skills/<skill-directory>/SKILL.md resource was found under {}",
            self.source.display_path(directory)
        ))
    }
}

impl BundleSource for Filesystem {
    type Content<'a> = Vec<u8>;

    fn is_file(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(self.root.join(path))
            .map(|metadata| metadata.is_file() || metadata.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn is_dir(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(self.root.join(path))
            .map(|metadata| metadata.is_dir() || metadata.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn read(&self, path: &Path) -> Result<Self::Content<'_>> {
        let mut full_path = self.root.clone();
        for component in path.components() {
            full_path.push(component);
            let metadata = std::fs::symlink_metadata(&full_path)?;
            if metadata.file_type().is_symlink() {
                return Err(bundle_symlink_error(&full_path));
            }
        }
        let metadata = std::fs::symlink_metadata(&full_path)?;
        if metadata.file_type().is_symlink() {
            return Err(bundle_symlink_error(&full_path));
        }
        std::fs::read(full_path).map_err(Into::into)
    }

    fn files_under(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        collect_files(&self.root, &self.root.join(path), &mut files)?;
        files.sort();
        Ok(files)
    }

    fn immediate_directories(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let directory = self.root.join(path);
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() {
            return Err(bundle_symlink_error(&directory));
        }
        if !metadata.is_dir() {
            return Ok(Vec::new());
        }
        let mut directories = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let entry_path = entry.path();
            let metadata = std::fs::symlink_metadata(&entry_path)?;
            if metadata.file_type().is_symlink() {
                return Err(bundle_symlink_error(&entry_path));
            }
            if metadata.is_dir() {
                directories.push(path.join(entry.file_name()));
            }
        }
        directories.sort();
        Ok(directories)
    }

    fn display_path(&self, path: &Path) -> String {
        self.root.join(path).display().to_string()
    }

    fn validate_skill_directory(&self, path: &Path) -> Result<()> {
        let directory = self.root.join(path);
        let metadata = std::fs::symlink_metadata(&directory)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::message(format!(
                "skill directory cannot be a symlink: {}",
                directory.display()
            )));
        }
        let canonical = directory.canonicalize().with_context(|| {
            format!("Failed to resolve skill directory: {}", directory.display())
        })?;
        validate_filesystem_tree(&canonical, &directory)
    }
}

impl<B: AsRef<[u8]>> BundleSource for Entries<B> {
    type Content<'a>
        = &'a B
    where
        B: 'a;

    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.as_os_str().is_empty() || self.directories.contains(path)
    }

    fn read(&self, path: &Path) -> Result<Self::Content<'_>> {
        self.files.get(path).ok_or_else(|| {
            Error::message(format!(
                "bundle entry does not exist: {}",
                self.display_path(path)
            ))
        })
    }

    fn files_under(&self, path: &Path) -> Result<Vec<PathBuf>> {
        Ok(self
            .files
            .keys()
            .filter(|entry| entry.starts_with(path))
            .cloned()
            .collect())
    }

    fn immediate_directories(&self, path: &Path) -> Result<Vec<PathBuf>> {
        Ok(self
            .directories
            .iter()
            .filter(|directory| directory.parent().unwrap_or_else(|| Path::new("")) == path)
            .cloned()
            .collect())
    }

    fn display_path(&self, path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }
}

fn normalize_entry_path(path: &Path) -> Result<PathBuf> {
    let original = path.to_str().ok_or_else(|| {
        Error::message(format!(
            "bundle entry path is not UTF-8: {}",
            path.display()
        ))
    })?;
    if original.is_empty() {
        return Err(Error::message("invalid bundle entry path: path is empty"));
    }
    let logical = original.replace('\\', "/");
    if logical.starts_with('/')
        || logical.as_bytes().get(1) == Some(&b':') && logical.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(Error::message(format!(
            "invalid bundle entry path '{original}': path must be relative"
        )));
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(&logical).components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => {
                return Err(Error::message(format!(
                    "invalid bundle entry path '{original}': only normal relative components are allowed"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() || logical.ends_with('/') {
        return Err(Error::message(format!(
            "invalid bundle entry path '{original}': path must name a file"
        )));
    }
    Ok(normalized)
}

pub(crate) fn validate_space_id(id: &str) -> Result<()> {
    if id.is_empty()
        || matches!(id, "." | "..")
        || id.contains(['/', '\\', ':'])
    {
        return Err(Error::message(format!(
            "invalid space id '{id}': space ids must be a single path component"
        )));
    }
    Ok(())
}

fn logical_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(bundle_symlink_error(directory));
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(bundle_symlink_error(&path));
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|_| {
                        Error::message(format!("bundle file escaped root: {}", path.display()))
                    })?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn bundle_symlink_error(path: &Path) -> Error {
    Error::message(format!(
        "bundle paths cannot be symlinks: {}",
        path.display()
    ))
}

fn validate_filesystem_tree(canonical_root: &Path, directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
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
            validate_filesystem_tree(canonical_root, &path)?;
        }
    }
    Ok(())
}

fn utf8<'a>(content: &'a [u8], path: &str) -> Result<&'a str> {
    std::str::from_utf8(content)
        .map_err(|error| Error::message(format!("bundle text file is not UTF-8: {path}: {error}")))
}

fn resource_dir(space_id: &str, name: &str) -> PathBuf {
    Path::new(space_id).join(name)
}

fn manifest_path(space_id: &str, name: &str) -> PathBuf {
    Path::new(space_id).join("manifest").join(name)
}

fn saved_object_matches(value: &Value, object_type: &str, id: &str) -> bool {
    value.get("type").and_then(Value::as_str) == Some(object_type)
        && value.get("id").and_then(Value::as_str) == Some(id)
}

fn find_named_resource(values: &[Value], id: &str, name: Option<&str>) -> Option<Value> {
    values
        .iter()
        .find(|value| value.get("id").and_then(Value::as_str) == Some(id))
        .or_else(|| {
            name.and_then(|name| {
                values
                    .iter()
                    .find(|value| value.get("name").and_then(Value::as_str) == Some(name))
            })
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    struct ApplicationBytes(Vec<u8>);

    impl AsRef<[u8]> for ApplicationBytes {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    fn fixture() -> Vec<(String, Vec<u8>)> {
        [
            ("spaces.yml", "spaces:\n  - id: default\n    name: Default\n"),
            ("default/manifest/saved_objects.json", r#"{"objects":[{"type":"dashboard","id":"dash-1"}]}"#),
            ("default/objects/dashboard/dash-1.json", r#"{
                // JSON5 comments, unquoted keys, and trailing commas are valid.
                type: "dashboard",
                id: "dash-1",
                attributes: { title: "Dash", },
            }"#),
            ("default/manifest/workflows.yml", "workflows:\n  - id: workflow-1\n    name: Workflow\n"),
            ("default/workflows/workflow.json", r#"{"id":"workflow-1","name":"Workflow"}"#),
            ("default/manifest/agents.yml", "agents:\n  - id: agent-1\n    name: Agent\n"),
            ("default/agents/agent.json", r#"{"id":"agent-1","name":"Agent"}"#),
            ("default/manifest/tools.yml", "tools:\n  - tool-1\n"),
            ("default/tools/tool.json", r#"{"id":"tool-1","name":"Tool"}"#),
            ("default/manifest/skills.yml", "skills:\n  - id: skill-1\n    name: Skill\n"),
            ("default/skills/skill-1/SKILL.md", "---\nid: skill-1\nname: Skill\ntool_ids:\n  - tool-1\n---\nInstructions\n"),
            ("default/skills/skill-1/references/query.txt", "Query body\n"),
            ("default/skills/skill-1/examples/intro.yml", "Intro body\n"),
        ]
        .into_iter()
        .map(|(path, content)| (path.to_string(), content.as_bytes().to_vec()))
        .collect()
    }

    fn write_entries(root: &Path, entries: &[(String, Vec<u8>)]) {
        for (path, content) in entries {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }

    #[test]
    fn entry_sources_support_owned_borrowed_and_shared_bytes() {
        let owned = KibanaBundle::from_entries(fixture())
            .unwrap()
            .read_all()
            .unwrap();
        let fixture = fixture();
        let borrowed = KibanaBundle::from_entries(
            fixture
                .iter()
                .map(|(path, content)| (path, content.as_slice())),
        )
        .unwrap()
        .read_all()
        .unwrap();
        let shared = KibanaBundle::from_entries(
            fixture
                .iter()
                .map(|(path, content)| (path, Arc::<[u8]>::from(content.clone()))),
        )
        .unwrap()
        .read_all()
        .unwrap();

        assert_eq!(owned, borrowed);
        assert_eq!(owned, shared);
        let referenced = owned.by_space["default"].skills[0]["referenced_content"]
            .as_array()
            .unwrap();
        assert_eq!(referenced[0]["name"], "intro");
        assert_eq!(referenced[0]["relativePath"], "./examples");
        assert_eq!(referenced[1]["name"], "query");
        assert_eq!(referenced[1]["relativePath"], "./references");

        let application = KibanaBundle::from_entries(
            fixture
                .iter()
                .map(|(path, content)| (path, ApplicationBytes(content.clone()))),
        )
        .unwrap()
        .read_all()
        .unwrap();
        assert_eq!(owned, application);
    }

    #[test]
    fn filesystem_and_entries_have_read_parity() {
        let temp = TempDir::new().unwrap();
        let entries = fixture();
        write_entries(temp.path(), &entries);

        let filesystem = KibanaBundle::open(temp.path()).unwrap().read_all().unwrap();
        let memory = KibanaBundle::from_entries(
            entries
                .iter()
                .map(|(path, content)| (path, content.as_slice())),
        )
        .unwrap()
        .read_all()
        .unwrap();

        assert_eq!(filesystem, memory);
    }

    #[test]
    fn space_definitions_preserve_full_space_configuration() {
        let bundle = KibanaBundle::from_entries([
            (
                "spaces.yml",
                b"spaces:\n  - id: default\n    name: Default\n".as_slice(),
            ),
            (
                "default/space.json",
                br#"{id: "default", name: "Default", description: "Full definition", solution: "oblt"}"#
                    .as_slice(),
            ),
        ])
        .unwrap()
        .read_all()
        .unwrap();

        assert_eq!(bundle.spaces[0]["description"], "Full definition");
        assert_eq!(bundle.spaces[0]["solution"], "oblt");
    }

    #[test]
    fn space_definitions_default_missing_name_from_manifest() {
        let bundle = KibanaBundle::from_entries([
            (
                "spaces.yml",
                b"spaces:\n  - id: default\n    name: Default\n".as_slice(),
            ),
            (
                "default/space.json",
                br#"{id: "default", solution: "oblt"}"#.as_slice(),
            ),
        ])
        .unwrap()
        .read_all()
        .unwrap();

        assert_eq!(bundle.spaces[0]["name"], "Default");
        assert_eq!(bundle.spaces[0]["solution"], "oblt");
    }

    #[test]
    fn entry_paths_are_validated_and_directories_are_implicit() {
        for path in [
            "",
            "/absolute.json",
            "../escape.json",
            "a/../b.json",
            "C:\\root.json",
            "dir/",
        ] {
            let result = KibanaBundle::from_entries([(path, b"{}".as_slice())]);
            assert!(result.is_err(), "{path} should be invalid");
        }

        let duplicate = KibanaBundle::from_entries([
            ("default/tools/tool.json", b"{}".as_slice()),
            ("default\\tools\\tool.json", b"{}".as_slice()),
        ]);
        assert!(duplicate.unwrap_err().to_string().contains("duplicate"));

        for entries in [
            [
                ("default/tools", b"{}".as_slice()),
                ("default/tools/tool.json", b"{}".as_slice()),
            ],
            [
                ("default/tools/tool.json", b"{}".as_slice()),
                ("default/tools", b"{}".as_slice()),
            ],
        ] {
            let collision = KibanaBundle::from_entries(entries).unwrap_err();
            assert!(collision.to_string().contains("conflicts"));
        }

        let bundle = KibanaBundle::from_entries([(
            "default/tools/tool.json",
            br#"{"id":"tool-1"}"#.as_slice(),
        )])
        .unwrap();
        assert!(bundle.source.is_dir(Path::new("default/tools")));
        assert_eq!(
            bundle
                .source
                .files_under(Path::new("default"))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn entry_files_under_does_not_stop_at_lexicographic_siblings() {
        let bundle = KibanaBundle::from_entries([
            ("default/skills/incident-response/SKILL.md", b"skill".as_slice()),
            (
                "default/skills/incident-response-v2/notes.md",
                b"sibling".as_slice(),
            ),
        ])
        .unwrap();

        assert_eq!(
            bundle
                .source
                .files_under(Path::new("default/skills/incident-response"))
                .unwrap(),
            vec![PathBuf::from("default/skills/incident-response/SKILL.md")]
        );
    }

    #[test]
    fn entry_errors_include_logical_paths() {
        let invalid_utf8 = KibanaBundle::from_entries([("spaces.yml", [0xff])])
            .unwrap()
            .read_all()
            .unwrap_err();
        assert!(invalid_utf8.to_string().contains("spaces.yml"));

        let invalid_json =
            KibanaBundle::from_entries([("default/objects/bad.json", b"{".as_slice())])
                .unwrap()
                .read_all()
                .unwrap_err();
        assert!(
            invalid_json
                .to_string()
                .contains("default/objects/bad.json")
        );
    }

    #[test]
    fn manifest_parse_errors_include_logical_paths() {
        let yaml_error = KibanaBundle::from_entries([("spaces.yml", b"{".as_slice())])
            .unwrap()
            .read_all()
            .unwrap_err();
        assert!(yaml_error.to_string().contains("spaces.yml"));

        let json_error =
            KibanaBundle::from_entries([("default/manifest/saved_objects.json", b"{".as_slice())])
                .unwrap()
                .read_all()
                .unwrap_err();
        assert!(
            json_error
                .to_string()
                .contains("default/manifest/saved_objects.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_sources_reject_symlink_traversal() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("default")).unwrap();

        let error = KibanaBundle::open(temp.path())
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("bundle paths cannot be symlinks")
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_bundle_roots_cannot_be_symlinks() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let root = temp.path().join("bundle");
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();

        for result in [KibanaBundle::open(&root), KibanaBundle::create(&root)] {
            let error = result.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("bundle paths cannot be symlinks")
            );
        }
    }

    #[test]
    fn selection_and_manifest_order_match_filesystem_behavior() {
        let mut entries = fixture();
        entries
            .iter_mut()
            .find(|(path, _)| path == "default/manifest/tools.yml")
            .unwrap()
            .1 = b"tools:\n  - tool-2\n  - tool-1\n".to_vec();
        entries.push((
            "default/tools/z-tool-2.json".to_string(),
            br#"{"id":"tool-2","name":"Tool Two"}"#.to_vec(),
        ));
        entries.push((
            "default/tools/extra.json".to_string(),
            br#"{"id":"extra"}"#.to_vec(),
        ));
        entries.push((
            "secondary/tools/tool.json".to_string(),
            br#"{"id":"secondary-tool"}"#.to_vec(),
        ));
        let temp = TempDir::new().unwrap();
        write_entries(temp.path(), &entries);
        let bundle = KibanaBundle::from_entries(entries).unwrap();
        let selection = SyncSelection {
            spaces: vec!["default".to_string()],
            include_tools: true,
            ..SyncSelection::default()
        };
        let read = bundle.read(&selection).unwrap();
        let filesystem = KibanaBundle::open(temp.path())
            .unwrap()
            .read(&selection)
            .unwrap();

        assert_eq!(
            read.by_space["default"].tools,
            vec![
                json!({"id": "tool-2", "name": "Tool Two"}),
                json!({"id": "tool-1", "name": "Tool"}),
            ]
        );
        assert!(read.by_space["default"].agents.is_empty());
        assert!(!read.by_space.contains_key("secondary"));
        assert_eq!(read, filesystem);
    }

    #[test]
    fn empty_bundle_and_implicit_spaces_are_supported() {
        let empty =
            KibanaBundle::<Entries<&[u8]>>::from_entries(std::iter::empty::<(&str, &[u8])>())
                .unwrap()
                .read_all()
                .unwrap();
        assert!(empty.spaces.is_empty());
        assert!(empty.by_space.is_empty());

        let discovered = KibanaBundle::from_entries([
            (
                "spaces.yml",
                b"spaces:\n  - id: listed\n    name: Listed\n".as_slice(),
            ),
            ("unlisted/tools/tool.json", br#"{"id":"tool-1"}"#.as_slice()),
        ])
        .unwrap()
        .read_all()
        .unwrap();
        assert!(discovered.by_space.contains_key("listed"));
        assert!(discovered.by_space.contains_key("unlisted"));
    }

    #[test]
    fn missing_manifest_resources_report_logical_directory() {
        let entries = vec![
            (
                "default/manifest/tools.yml".to_string(),
                b"tools:\n  - missing-tool\n".to_vec(),
            ),
            (
                "default/tools/present.json".to_string(),
                br#"{"id":"present-tool"}"#.to_vec(),
            ),
        ];
        let error = KibanaBundle::from_entries(entries.clone())
            .unwrap()
            .read_all()
            .unwrap_err();

        let temp = TempDir::new().unwrap();
        write_entries(temp.path(), &entries);
        let filesystem_error = KibanaBundle::open(temp.path())
            .unwrap()
            .read_all()
            .unwrap_err();

        assert!(error.to_string().contains("missing-tool"));
        assert!(error.to_string().contains("default/tools"));
        assert!(filesystem_error.to_string().contains("missing-tool"));
        assert!(filesystem_error.to_string().contains("default/tools"));
    }

    #[test]
    fn filesystem_write_round_trips_through_generic_reader() {
        let temp = TempDir::new().unwrap();
        let expected = KibanaBundle::from_entries(fixture())
            .unwrap()
            .read_all()
            .unwrap();
        let filesystem = KibanaBundle::create(temp.path()).unwrap();

        filesystem.write(&expected).unwrap();
        let actual = KibanaBundle::open(temp.path()).unwrap().read_all().unwrap();

        assert_eq!(actual, expected);
        assert_eq!(filesystem.root(), temp.path());
    }

    #[test]
    fn filesystem_write_defaults_space_definition_name() {
        let temp = TempDir::new().unwrap();
        let filesystem = KibanaBundle::create(temp.path()).unwrap();
        let bundle = SyncBundle {
            spaces: vec![json!({"id": "default"})],
            ..SyncBundle::default()
        };

        filesystem.write(&bundle).unwrap();
        let read = KibanaBundle::open(temp.path()).unwrap().read_all().unwrap();

        assert_eq!(
            read.spaces,
            vec![json!({"id": "default", "name": "default"})]
        );
    }

    #[test]
    fn filesystem_write_rejects_non_object_spaces() {
        let temp = TempDir::new().unwrap();
        let filesystem = KibanaBundle::create(temp.path()).unwrap();
        let bundle = SyncBundle {
            spaces: vec![json!("default")],
            ..SyncBundle::default()
        };

        let error = filesystem.write(&bundle).unwrap_err();

        assert_eq!(error.to_string(), "space must be a JSON object");
    }

    #[test]
    fn bundle_reader_rejects_path_traversal_space_ids() {
        let selection = SyncSelection {
            spaces: vec!["../outside".to_string()],
            include_tools: true,
            ..SyncSelection::default()
        };
        let error = KibanaBundle::<Entries<&[u8]>>::from_entries(std::iter::empty::<(
            &str,
            &[u8],
        )>())
        .unwrap()
            .read(&selection)
            .unwrap_err();

        assert!(error.to_string().contains("invalid space id '../outside'"));
    }

    #[test]
    fn bundle_reader_rejects_path_traversal_space_manifest_ids() {
        let error = KibanaBundle::from_entries([(
            "spaces.yml",
            b"spaces:\n  - id: ../outside\n    name: Outside\n".as_slice(),
        )])
        .unwrap()
        .read_all()
        .unwrap_err();

        assert!(error.to_string().contains("invalid space id '../outside'"));
    }

    #[test]
    fn bundle_reader_rejects_manifest_directories() {
        for entries in [
            vec![("spaces.yml/marker", b"directory".as_slice())],
            vec![(
                "default/manifest/tools.yml/marker",
                b"directory".as_slice(),
            )],
        ] {
            let error = KibanaBundle::from_entries(entries)
                .unwrap()
                .read_all()
                .unwrap_err();

            assert!(error.to_string().contains("bundle manifest must be a file"));
        }
    }

    #[test]
    fn filesystem_write_rejects_path_traversal_space_ids() {
        let temp = TempDir::new().unwrap();
        let filesystem = KibanaBundle::create(temp.path()).unwrap();
        let bundle = SyncBundle {
            by_space: std::collections::HashMap::from([(
                "../outside".to_string(),
                SpaceBundle::default(),
            )]),
            ..SyncBundle::default()
        };

        let error = filesystem.write(&bundle).unwrap_err();

        assert!(error.to_string().contains("invalid space id '../outside'"));
    }
}
