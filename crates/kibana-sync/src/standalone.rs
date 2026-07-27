//! Manifest-free local artifacts used by standalone import and export commands.

use crate::kibana::skills::skill_to_value;
use crate::{Error, Result, ResultContext, json5};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};

const SKILL_FILE: &str = "SKILL.md";

/// A file-backed Kibana API family supported by standalone transfer.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum ResourceFamily {
    Skills,
    Tools,
    Agents,
    Workflows,
}

impl ResourceFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Tools => "tools",
            Self::Agents => "agents",
            Self::Workflows => "workflows",
        }
    }

    fn resource_name(self) -> &'static str {
        match self {
            Self::Skills => "skill",
            Self::Tools => "tool",
            Self::Agents => "agent",
            Self::Workflows => "workflow",
        }
    }
}

impl fmt::Display for ResourceFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The remote mutation attempted for a standalone resource.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResourceOperation {
    Create,
    Update,
}

/// The final application state of one resource in a standalone batch.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResourceOutcomeStatus {
    Applied,
    Skipped,
    Failed,
}

/// The structured result of applying one resource.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResourceOutcome {
    family: ResourceFamily,
    id: String,
    status: ResourceOutcomeStatus,
    operation: Option<ResourceOperation>,
    detail: Option<String>,
}

impl ResourceOutcome {
    pub fn applied(
        family: ResourceFamily,
        id: impl Into<String>,
        operation: ResourceOperation,
    ) -> Self {
        Self {
            family,
            id: id.into(),
            status: ResourceOutcomeStatus::Applied,
            operation: Some(operation),
            detail: None,
        }
    }

    pub fn written(family: ResourceFamily, id: impl Into<String>) -> Self {
        Self {
            family,
            id: id.into(),
            status: ResourceOutcomeStatus::Applied,
            operation: None,
            detail: None,
        }
    }

    pub fn skipped(
        family: ResourceFamily,
        id: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            family,
            id: id.into(),
            status: ResourceOutcomeStatus::Skipped,
            operation: None,
            detail: Some(detail.into()),
        }
    }

    pub fn failed(
        family: ResourceFamily,
        id: impl Into<String>,
        operation: Option<ResourceOperation>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            family,
            id: id.into(),
            status: ResourceOutcomeStatus::Failed,
            operation,
            detail: Some(detail.into()),
        }
    }

    pub fn family(&self) -> ResourceFamily {
        self.family
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn status(&self) -> ResourceOutcomeStatus {
        self.status
    }

    pub fn operation(&self) -> Option<ResourceOperation> {
        self.operation
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// Aggregate counts for a completed standalone resource batch.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ResourceBatchCounts {
    pub attempted: usize,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Deterministically ordered outcomes for a standalone resource batch.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ResourceBatchReport {
    outcomes: Vec<ResourceOutcome>,
}

impl ResourceBatchReport {
    pub fn new(outcomes: Vec<ResourceOutcome>) -> Self {
        Self { outcomes }
    }

    pub fn outcomes(&self) -> &[ResourceOutcome] {
        &self.outcomes
    }

    pub fn counts(&self) -> ResourceBatchCounts {
        let mut counts = ResourceBatchCounts {
            attempted: self.outcomes.len(),
            ..ResourceBatchCounts::default()
        };

        for outcome in &self.outcomes {
            match outcome.status {
                ResourceOutcomeStatus::Applied => counts.applied += 1,
                ResourceOutcomeStatus::Skipped => counts.skipped += 1,
                ResourceOutcomeStatus::Failed => counts.failed += 1,
            }
        }

        counts
    }

    pub fn has_failures(&self) -> bool {
        self.outcomes
            .iter()
            .any(|outcome| outcome.status == ResourceOutcomeStatus::Failed)
    }
}

/// Marker state for a plan whose source paths have been discovered.
#[derive(Debug)]
pub struct Discovered;

/// Marker state for a plan whose complete batch has been projected and validated.
#[derive(Debug)]
pub struct Validated;

mod private {
    pub trait Sealed {}
}

/// State-specific data held by an [`ImportPlan`].
pub trait ImportPlanState: private::Sealed {
    type Resource;
}

impl private::Sealed for Discovered {}
impl private::Sealed for Validated {}

impl ImportPlanState for Discovered {
    type Resource = DiscoveredResource;
}

impl ImportPlanState for Validated {
    type Resource = ValidatedResource;
}

/// A local artifact selected during manifest-free discovery.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiscoveredResource {
    source: PathBuf,
}

impl DiscoveredResource {
    pub fn source(&self) -> &Path {
        &self.source
    }
}

/// A completely projected local artifact with its authoritative resource ID.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedResource {
    source: PathBuf,
    id: String,
    value: Value,
}

impl ValidatedResource {
    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

/// A deterministic, family-specific standalone import batch.
#[derive(Debug)]
pub struct ImportPlan<State: ImportPlanState> {
    family: ResourceFamily,
    resources: Vec<State::Resource>,
    state: PhantomData<State>,
}

impl<State: ImportPlanState> ImportPlan<State> {
    pub fn family(&self) -> ResourceFamily {
        self.family
    }

    pub fn resources(&self) -> &[State::Resource] {
        &self.resources
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

impl ImportPlan<Discovered> {
    /// Discover one supported standalone artifact source without reading a manifest.
    pub fn discover(family: ResourceFamily, source: impl AsRef<Path>) -> Result<Self> {
        let source = source.as_ref();
        let sources = match family {
            ResourceFamily::Skills => discover_skill_sources(source)?,
            ResourceFamily::Tools | ResourceFamily::Agents | ResourceFamily::Workflows => {
                discover_json_sources(family, source)?
            }
        };

        Ok(Self {
            family,
            resources: sources
                .into_iter()
                .map(|source| DiscoveredResource { source })
                .collect(),
            state: PhantomData,
        })
    }

    /// Project and validate every discovered resource before exposing the batch.
    pub fn validate(self) -> Result<ImportPlan<Validated>> {
        let mut resources = Vec::with_capacity(self.resources.len());
        let mut sources_by_id = HashMap::<String, PathBuf>::new();
        let mut failures = Vec::new();

        for discovered in self.resources {
            let source = discovered.source;
            let projected = project_resource(self.family, &source).and_then(|value| {
                let id = authoritative_id(self.family, &value)?;
                Ok((id, value))
            });

            match projected {
                Ok((id, value)) => {
                    if let Some(first_source) = sources_by_id.get(&id) {
                        failures.push(format!(
                            "duplicate {} ID '{id}' in {} and {}",
                            self.family.resource_name(),
                            first_source.display(),
                            source.display()
                        ));
                        continue;
                    }
                    sources_by_id.insert(id.clone(), source.clone());
                    resources.push(ValidatedResource { source, id, value });
                }
                Err(error) => failures.push(format!("{}: {error}", source.display())),
            }
        }

        if !failures.is_empty() {
            return Err(Error::message(format!(
                "standalone {} import validation failed:\n- {}",
                self.family,
                failures.join("\n- ")
            )));
        }

        Ok(ImportPlan {
            family: self.family,
            resources,
            state: PhantomData,
        })
    }
}

impl ImportPlan<Validated> {
    pub fn into_values(self) -> Vec<Value> {
        self.resources
            .into_iter()
            .map(ValidatedResource::into_value)
            .collect()
    }
}

/// Marker state for an export plan containing fetched selections.
#[derive(Debug)]
pub struct Selected;

/// Marker state for an export plan whose output paths passed collision checks.
#[derive(Debug)]
pub struct ReadyToWrite;

/// State-specific data held by an [`ExportPlan`].
pub trait ExportPlanState: private::Sealed {
    type Resource;
}

impl private::Sealed for Selected {}
impl private::Sealed for ReadyToWrite {}

impl ExportPlanState for Selected {
    type Resource = SelectedExportResource;
}

impl ExportPlanState for ReadyToWrite {
    type Resource = ReadyExportResource;
}

/// One selected and completely fetched resource.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedExportResource {
    id: String,
    value: Value,
}

impl SelectedExportResource {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// One fetched resource with a preflighted output path.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadyExportResource {
    id: String,
    value: Value,
    output: PathBuf,
}

impl ReadyExportResource {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn output(&self) -> &Path {
        &self.output
    }
}

/// A deterministic, family-specific standalone export batch.
#[derive(Debug)]
pub struct ExportPlan<State: ExportPlanState> {
    family: ResourceFamily,
    destination: PathBuf,
    overwrite: bool,
    resources: Vec<State::Resource>,
    state: PhantomData<State>,
}

impl<State: ExportPlanState> ExportPlan<State> {
    pub fn family(&self) -> ResourceFamily {
        self.family
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn overwrite(&self) -> bool {
        self.overwrite
    }

    pub fn resources(&self) -> &[State::Resource] {
        &self.resources
    }
}

impl ExportPlan<Selected> {
    /// Validate fetched definitions and preserve the requested selection order.
    pub fn selected(
        family: ResourceFamily,
        destination: impl AsRef<Path>,
        resources: Vec<(String, Value)>,
        overwrite: bool,
    ) -> Result<Self> {
        let mut selected = Vec::with_capacity(resources.len());
        let mut first_selection = HashMap::<String, usize>::new();
        let mut failures = Vec::new();

        for (index, (selected_id, value)) in resources.into_iter().enumerate() {
            if selected_id.trim().is_empty() {
                failures.push(format!("selection {} has an empty resource ID", index + 1));
                continue;
            }
            if let Some(first_index) = first_selection.get(&selected_id) {
                failures.push(format!(
                    "duplicate selected {} ID '{}' at positions {} and {}",
                    family.resource_name(),
                    selected_id,
                    first_index + 1,
                    index + 1
                ));
                continue;
            }
            first_selection.insert(selected_id.clone(), index);

            match authoritative_id(family, &value) {
                Ok(fetched_id) if fetched_id != selected_id => failures.push(format!(
                    "selected {} ID '{}' returned definition ID '{}'",
                    family.resource_name(),
                    selected_id,
                    fetched_id
                )),
                Ok(_) if is_readonly(&value) => failures.push(format!(
                    "selected {} '{}' is readonly and cannot be standalone-imported",
                    family.resource_name(),
                    selected_id
                )),
                Ok(_) => selected.push(SelectedExportResource {
                    id: selected_id,
                    value,
                }),
                Err(error) => failures.push(format!(
                    "selected {} '{}': {error}",
                    family.resource_name(),
                    selected_id
                )),
            }
        }

        if !failures.is_empty() {
            return Err(Error::message(format!(
                "standalone {family} export validation failed:\n- {}",
                failures.join("\n- ")
            )));
        }

        Ok(Self {
            family,
            destination: destination.as_ref().to_path_buf(),
            overwrite,
            resources: selected,
            state: PhantomData,
        })
    }

    /// Calculate and preflight every selected output before permitting writes.
    pub fn prepare(
        self,
        mut output_path: impl FnMut(ResourceFamily, &Value, &Path) -> Result<PathBuf>,
    ) -> Result<ExportPlan<ReadyToWrite>> {
        let mut ready = Vec::with_capacity(self.resources.len());
        let mut ids_by_path = HashMap::<String, String>::new();
        let mut failures = Vec::new();

        if self.destination.exists() && !self.destination.is_dir() {
            failures.push(format!(
                "export destination is not a directory: {}",
                self.destination.display()
            ));
        }

        for resource in self.resources {
            let path = match output_path(self.family, &resource.value, &self.destination) {
                Ok(path) => path,
                Err(error) => {
                    failures.push(format!("{}: {error}", resource.id));
                    continue;
                }
            };
            let relative_path = path.strip_prefix(&self.destination).ok();
            let escapes_destination = relative_path.is_none_or(|relative| {
                relative.as_os_str().is_empty()
                    || relative.components().any(|component| {
                        matches!(
                            component,
                            Component::ParentDir | Component::RootDir | Component::Prefix(_)
                        )
                    })
            });
            if escapes_destination {
                failures.push(format!(
                    "{} maps outside the export destination: {}",
                    resource.id,
                    path.display()
                ));
                continue;
            }

            let comparable_path = path.to_string_lossy().to_lowercase();
            if let Some(first_id) = ids_by_path.get(&comparable_path) {
                failures.push(format!(
                    "resource IDs '{}' and '{}' map to the same output path {}",
                    first_id,
                    resource.id,
                    path.display()
                ));
                continue;
            }
            ids_by_path.insert(comparable_path, resource.id.clone());

            match std::fs::symlink_metadata(&path) {
                Ok(_) if !self.overwrite => {
                    failures.push(format!(
                        "output already exists for '{}': {} (use --overwrite to replace it)",
                        resource.id,
                        path.display()
                    ));
                    continue;
                }
                Ok(metadata) => {
                    let expected_type_matches = match self.family {
                        ResourceFamily::Skills => metadata.is_dir(),
                        ResourceFamily::Tools
                        | ResourceFamily::Agents
                        | ResourceFamily::Workflows => metadata.is_file(),
                    };
                    if metadata.file_type().is_symlink() || !expected_type_matches {
                        let expected = if self.family == ResourceFamily::Skills {
                            "directory"
                        } else {
                            "file"
                        };
                        failures.push(format!(
                            "existing output for '{}' is not a replaceable {}: {}",
                            resource.id,
                            expected,
                            path.display()
                        ));
                        continue;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    failures.push(format!(
                        "failed to inspect output for '{}': {}: {error}",
                        resource.id,
                        path.display()
                    ));
                    continue;
                }
            }

            ready.push(ReadyExportResource {
                id: resource.id,
                value: resource.value,
                output: path,
            });
        }

        if !failures.is_empty() {
            return Err(Error::message(format!(
                "standalone {} export output preflight failed:\n- {}",
                self.family,
                failures.join("\n- ")
            )));
        }

        Ok(ExportPlan {
            family: self.family,
            destination: self.destination,
            overwrite: self.overwrite,
            resources: ready,
            state: PhantomData,
        })
    }
}

pub(crate) async fn server_resource_is_readonly(
    client: &crate::client::KibanaClient,
    path: &str,
    internal: bool,
    resource_name: &str,
) -> Result<bool> {
    let response = if internal {
        client.get_internal(path).await?
    } else {
        client.get(path).await?
    };
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::api_response(status, body));
    }

    let existing = response.json::<Value>().await.map_err(|error| {
        Error::message(format!("Failed to parse existing {resource_name}: {error}"))
    })?;
    Ok(is_readonly(&existing))
}

fn is_readonly(value: &Value) -> bool {
    value.get("readonly").and_then(Value::as_bool) == Some(true)
}

fn discover_skill_sources(source: &Path) -> Result<Vec<PathBuf>> {
    let metadata = source_metadata(source, "Skill")?;
    if !metadata.is_dir() {
        return Err(Error::message(format!(
            "unsupported Skills source '{}': expected a directory containing {SKILL_FILE} or immediate child directories containing {SKILL_FILE}",
            source.display()
        )));
    }

    let direct_skill = path_entry_exists(&source.join(SKILL_FILE));
    let mut child_skills = Vec::new();
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("Failed to read Skills source: {}", source.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect entry in Skills source: {}",
                source.display()
            )
        })?;
        if entry
            .file_type()
            .with_context(|| format!("Failed to inspect path: {}", entry.path().display()))?
            .is_dir()
            && path_entry_exists(&entry.path().join(SKILL_FILE))
        {
            child_skills.push(entry.path());
        }
    }
    child_skills.sort();

    match (direct_skill, child_skills.is_empty()) {
        (true, false) => Err(Error::message(format!(
            "ambiguous Skills source '{}': it contains {SKILL_FILE} and immediate child Skill directories; select either one Skill directory or a collection root",
            source.display()
        ))),
        (true, true) => Ok(vec![source.to_path_buf()]),
        (false, false) => Ok(child_skills),
        (false, true) => Err(Error::message(format!(
            "no Skills found in '{}': expected {SKILL_FILE} or immediate child directories containing {SKILL_FILE}",
            source.display()
        ))),
    }
}

fn discover_json_sources(family: ResourceFamily, source: &Path) -> Result<Vec<PathBuf>> {
    let metadata = source_metadata(source, family.as_str())?;
    if metadata.is_file() {
        if has_json_extension(source) {
            return Ok(vec![source.to_path_buf()]);
        }
        return Err(unsupported_json_source(family, source));
    }
    if !metadata.is_dir() {
        return Err(unsupported_json_source(family, source));
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(source).with_context(|| {
        format!(
            "Failed to read standalone {} source: {}",
            family,
            source.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect entry in standalone {} source: {}",
                family,
                source.display()
            )
        })?;
        if entry
            .file_type()
            .with_context(|| format!("Failed to inspect path: {}", entry.path().display()))?
            .is_file()
            && has_json_extension(&entry.path())
        {
            files.push(entry.path());
        }
    }
    files.sort();

    if files.is_empty() {
        return Err(unsupported_json_source(family, source));
    }

    Ok(files)
}

fn project_resource(family: ResourceFamily, source: &Path) -> Result<Value> {
    match family {
        ResourceFamily::Skills => skill_to_value(source, true),
        ResourceFamily::Tools | ResourceFamily::Agents | ResourceFamily::Workflows => {
            let text = std::fs::read_to_string(source)
                .with_context(|| format!("Failed to read JSON resource: {}", source.display()))?;
            json5::from_json5_str(&text)
        }
    }
}

fn authoritative_id(family: ResourceFamily, value: &Value) -> Result<String> {
    value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(Error::MissingResourceId {
            resource: family.resource_name(),
        })
}

fn source_metadata(source: &Path, family: &str) -> Result<std::fs::Metadata> {
    std::fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::message(format!(
                "standalone {family} source does not exist: {}",
                source.display()
            ))
        } else {
            Error::context(
                format!("Failed to inspect standalone {family} source"),
                error.into(),
            )
        }
    })
}

fn path_entry_exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn has_json_extension(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("json")
}

fn unsupported_json_source(family: ResourceFamily, source: &Path) -> Error {
    Error::message(format!(
        "unsupported {family} source '{}': expected one .json file or a directory containing immediate .json files",
        source.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_skill(directory: &Path, id: &str, body: &str) {
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(
            directory.join(SKILL_FILE),
            format!("---\nid: {id}\nname: {id}\n---\n{body}"),
        )
        .unwrap();
    }

    fn write_json(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn json_families() -> [(ResourceFamily, &'static str); 3] {
        [
            (ResourceFamily::Tools, "tool"),
            (ResourceFamily::Agents, "agent"),
            (ResourceFamily::Workflows, "workflow"),
        ]
    }

    #[test]
    fn discovers_and_validates_one_skill_directory() {
        let temp = TempDir::new().unwrap();
        write_skill(temp.path(), "one-skill", "Instructions\n");

        let discovered = ImportPlan::discover(ResourceFamily::Skills, temp.path()).unwrap();
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered.resources()[0].source(), temp.path());

        let validated = discovered.validate().unwrap();
        assert_eq!(validated.resources()[0].id(), "one-skill");
        assert_eq!(
            validated.resources()[0].value()["content"],
            "Instructions\n"
        );
    }

    #[test]
    fn discovers_skill_collection_in_deterministic_nonrecursive_order() {
        let temp = TempDir::new().unwrap();
        write_skill(&temp.path().join("z-last"), "z-skill", "Z\n");
        write_skill(&temp.path().join("a-first"), "a-skill", "A\n");
        write_skill(
            &temp.path().join("ignored").join("nested"),
            "nested-skill",
            "Nested\n",
        );
        std::fs::write(temp.path().join("skills.yml"), "{ malformed").unwrap();

        let plan = ImportPlan::discover(ResourceFamily::Skills, temp.path())
            .unwrap()
            .validate()
            .unwrap();

        assert_eq!(
            plan.resources()
                .iter()
                .map(ValidatedResource::id)
                .collect::<Vec<_>>(),
            vec!["a-skill", "z-skill"]
        );
    }

    #[test]
    fn rejects_ambiguous_and_empty_skill_layouts() {
        let ambiguous = TempDir::new().unwrap();
        write_skill(ambiguous.path(), "root-skill", "Root\n");
        write_skill(&ambiguous.path().join("child"), "child-skill", "Child\n");
        let error = ImportPlan::discover(ResourceFamily::Skills, ambiguous.path()).unwrap_err();
        assert!(error.to_string().contains("ambiguous Skills source"));

        let empty = TempDir::new().unwrap();
        let error = ImportPlan::discover(ResourceFamily::Skills, empty.path()).unwrap_err();
        assert!(error.to_string().contains("no Skills found"));
    }

    #[test]
    fn validates_referenced_content_in_deterministic_order() {
        let temp = TempDir::new().unwrap();
        write_skill(temp.path(), "refs-skill", "Instructions\n");
        std::fs::create_dir(temp.path().join("examples")).unwrap();
        std::fs::write(temp.path().join("z.md"), "Z\n").unwrap();
        std::fs::write(temp.path().join("examples/a.md"), "A\n").unwrap();

        let plan = ImportPlan::discover(ResourceFamily::Skills, temp.path())
            .unwrap()
            .validate()
            .unwrap();

        assert_eq!(
            plan.resources()[0].value()["referenced_content"],
            json!([
                {"name": "z", "relativePath": "", "content": "Z\n"},
                {"name": "a", "relativePath": "./examples", "content": "A\n"}
            ])
        );
    }

    #[test]
    fn discovers_one_json5_resource_file() {
        for (family, noun) in json_families() {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join(format!("{noun}.json"));
            let id = format!("{noun}-a");
            write_json(
                &source,
                &format!(
                    r#"{{
                // JSON5 is accepted
                id: "{id}",
                description: """first
second""",
            }}"#
                ),
            );

            let plan = ImportPlan::discover(family, &source)
                .unwrap()
                .validate()
                .unwrap();

            assert_eq!(plan.resources()[0].id(), id);
            assert_eq!(plan.resources()[0].value()["description"], "first\nsecond");
        }
    }

    #[test]
    fn discovers_json_directory_in_deterministic_nonrecursive_order() {
        for (family, noun) in json_families() {
            let temp = TempDir::new().unwrap();
            write_json(
                &temp.path().join("z.json"),
                &format!(r#"{{id: "z-{noun}"}}"#),
            );
            write_json(
                &temp.path().join("a.json"),
                &format!(r#"{{id: "a-{noun}"}}"#),
            );
            write_json(
                &temp.path().join(format!("{}.yml", family.as_str())),
                "{ malformed",
            );
            std::fs::create_dir(temp.path().join("nested")).unwrap();
            write_json(
                &temp.path().join("nested/ignored.json"),
                &format!(r#"{{id: "ignored-{noun}"}}"#),
            );

            let plan = ImportPlan::discover(family, temp.path())
                .unwrap()
                .validate()
                .unwrap();

            assert_eq!(
                plan.resources()
                    .iter()
                    .map(ValidatedResource::id)
                    .collect::<Vec<_>>(),
                vec![format!("a-{noun}"), format!("z-{noun}")]
            );
        }
    }

    #[test]
    fn rejects_invalid_and_empty_json_layouts() {
        for (family, noun) in json_families() {
            let temp = TempDir::new().unwrap();
            let source = temp.path().join(format!("{noun}.json5"));
            write_json(&source, &format!(r#"{{id: "{noun}-a"}}"#));
            let error = ImportPlan::discover(family, &source).unwrap_err();
            assert!(error.to_string().contains("expected one .json file"));

            let empty = TempDir::new().unwrap();
            write_json(
                &empty.path().join(format!("{}.yml", family.as_str())),
                &format!("{}: []", family.as_str()),
            );
            let error = ImportPlan::discover(family, empty.path()).unwrap_err();
            assert!(error.to_string().contains("immediate .json files"));
        }
    }

    #[test]
    fn missing_json_source_names_the_plural_command_family() {
        let temp = TempDir::new().unwrap();

        for (family, _) in json_families() {
            let missing = temp.path().join(format!("missing-{}", family.as_str()));
            let error = ImportPlan::discover(family, &missing).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains(&format!("standalone {} source", family.as_str()))
            );
        }
    }

    #[test]
    fn export_plan_rejects_parent_traversal_below_destination_prefix() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("export");
        let plan = ExportPlan::selected(
            ResourceFamily::Tools,
            &destination,
            vec![(
                "tool-a".to_string(),
                json!({"id": "tool-a", "name": "Tool A"}),
            )],
            false,
        )
        .unwrap();

        let error = plan
            .prepare(|_, _, destination| Ok(destination.join("../outside.json")))
            .unwrap_err();

        assert!(error.to_string().contains("outside the export destination"));
    }

    #[test]
    fn export_plan_rejects_nonreplaceable_existing_output_before_writes() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("export");
        let output = destination.join("Tool A.json");
        std::fs::create_dir_all(&output).unwrap();
        let plan = ExportPlan::selected(
            ResourceFamily::Tools,
            &destination,
            vec![(
                "tool-a".to_string(),
                json!({"id": "tool-a", "name": "Tool A"}),
            )],
            true,
        )
        .unwrap();

        let error = plan.prepare(|_, _, _| Ok(output.clone())).unwrap_err();

        assert!(error.to_string().contains("not a replaceable file"));
        assert!(output.is_dir());
    }

    #[test]
    fn rejects_invalid_resources_with_their_source_paths() {
        for (family, noun) in json_families() {
            let temp = TempDir::new().unwrap();
            write_json(
                &temp.path().join("valid.json"),
                &format!(r#"{{id: "{noun}-a"}}"#),
            );
            write_json(&temp.path().join("invalid.json"), "{");

            let error = ImportPlan::discover(family, temp.path())
                .unwrap()
                .validate()
                .unwrap_err();
            let message = error.to_string();

            assert!(message.contains("invalid.json"));
            assert!(message.contains("validation failed"));
        }

        let skills = TempDir::new().unwrap();
        write_skill(&skills.path().join("valid"), "valid-skill", "Valid\n");
        std::fs::create_dir(skills.path().join("invalid")).unwrap();
        write_json(&skills.path().join("invalid/SKILL.md"), "---\nid:");
        let error = ImportPlan::discover(ResourceFamily::Skills, skills.path())
            .unwrap()
            .validate()
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&skills.path().join("invalid").display().to_string())
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_identifies_both_sources() {
        for (family, noun) in json_families() {
            let temp = TempDir::new().unwrap();
            let body = format!(r#"{{id: "{noun}-a"}}"#);
            write_json(&temp.path().join("first.json"), &body);
            write_json(&temp.path().join("second.json"), &body);

            let error = ImportPlan::discover(family, temp.path())
                .unwrap()
                .validate()
                .unwrap_err();
            let message = error.to_string();

            assert!(message.contains(&format!("duplicate {noun} ID '{noun}-a'")));
            assert!(message.contains("first.json"));
            assert!(message.contains("second.json"));
        }

        let skills = TempDir::new().unwrap();
        write_skill(&skills.path().join("first"), "duplicate-skill", "First\n");
        write_skill(&skills.path().join("second"), "duplicate-skill", "Second\n");
        let error = ImportPlan::discover(ResourceFamily::Skills, skills.path())
            .unwrap()
            .validate()
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("duplicate skill ID 'duplicate-skill'"));
        assert!(message.contains("first"));
        assert!(message.contains("second"));
    }

    #[cfg(unix)]
    #[test]
    fn preserves_skill_path_safety_validation() {
        let temp = TempDir::new().unwrap();
        write_skill(temp.path(), "unsafe-skill", "Instructions\n");
        let outside = temp.path().parent().unwrap().join("outside.md");
        std::fs::write(&outside, "outside").unwrap();
        std::os::unix::fs::symlink(&outside, temp.path().join("escape.md")).unwrap();

        let error = ImportPlan::discover(ResourceFamily::Skills, temp.path())
            .unwrap()
            .validate()
            .unwrap_err();

        assert!(error.to_string().contains("symlink traversal"));
    }

    #[test]
    fn rejects_missing_or_blank_authoritative_ids() {
        let temp = TempDir::new().unwrap();
        write_json(&temp.path().join("missing.json"), r#"{name: "missing"}"#);
        write_json(&temp.path().join("blank.json"), r#"{id: " "}"#);

        let error = ImportPlan::discover(ResourceFamily::Agents, temp.path())
            .unwrap()
            .validate()
            .unwrap_err();
        let message = error.to_string();

        assert!(message.contains("blank.json"));
        assert!(message.contains("missing.json"));
        assert_eq!(message.matches("missing required 'id' field").count(), 2);
    }
}
