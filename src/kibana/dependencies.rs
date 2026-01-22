//! Dependency discovery and resolution logic
//!
//! This module provides functions to identify dependencies between Kibana objects
//! (Agents, Tools, and Workflows) based on their JSON definitions.

use serde_json::Value;
use std::collections::HashSet;

/// Represents a dependency on another Kibana object
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Dependency {
    Agent(String),
    Tool(String),
    Workflow(String),
}

/// Find dependencies for an Agent
///
/// Agents usually depend on Tools listed in `configuration.tools`.
pub fn find_agent_dependencies(agent: &Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    if let Some(tools) = agent
        .get("configuration")
        .and_then(|c| c.get("tools"))
        .and_then(|t| t.as_array())
    {
        for tool in tools {
            if let Some(tool_id) = tool.as_str() {
                // Legacy or simple format: array of strings
                deps.push(Dependency::Tool(tool_id.to_string()));
            } else if let Some(tool_ids) = tool.get("tool_ids").and_then(|ids| ids.as_array()) {
                // New format: array of objects with tool_ids array
                for id in tool_ids {
                    if let Some(id_str) = id.as_str() {
                        deps.push(Dependency::Tool(id_str.to_string()));
                    }
                }
            }
        }
    }
    deps
}

/// Find dependencies for a Tool
///
/// Tools can depend on a Workflow listed in `configuration.workflow_id`.
pub fn find_tool_dependencies(tool: &Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    if let Some(workflow_id) = tool
        .get("configuration")
        .and_then(|c| c.get("workflow_id"))
        .and_then(|w| w.as_str())
    {
        deps.push(Dependency::Workflow(workflow_id.to_string()));
    }
    deps
}

/// Find dependencies for a Workflow
///
/// Workflows are complex and can contain references to Agents, Tools,
/// and other Workflows anywhere in their definition.
pub fn find_workflow_dependencies(workflow: &Value) -> Vec<Dependency> {
    let mut agents = HashSet::new();
    let mut tools = HashSet::new();
    let mut workflows = HashSet::new();

    // Skip the root "id" if it matches common names to avoid self-dependency
    // but the recursive resolution logic will handle this anyway.

    recursive_find_deps(workflow, &mut agents, &mut tools, &mut workflows);

    let mut deps = Vec::new();
    for id in agents {
        deps.push(Dependency::Agent(id));
    }
    for id in tools {
        deps.push(Dependency::Tool(id));
    }
    for id in workflows {
        deps.push(Dependency::Workflow(id));
    }
    deps
}

/// Recursively search for dependency-like keys in a JSON value
fn recursive_find_deps(
    value: &Value,
    agents: &mut HashSet<String>,
    tools: &mut HashSet<String>,
    workflows: &mut HashSet<String>,
) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                match k.as_str() {
                    "agent_id" | "agentId" => {
                        if let Some(id) = v.as_str() {
                            agents.insert(id.to_string());
                        }
                    }
                    "tool_id" | "toolId" | "tool_ids" | "toolIds" => {
                        if let Some(id) = v.as_str() {
                            tools.insert(id.to_string());
                        } else if let Some(arr) = v.as_array() {
                            for item in arr {
                                if let Some(id) = item.as_str() {
                                    tools.insert(id.to_string());
                                }
                            }
                        }
                    }
                    "workflow_id" | "workflowId" => {
                        if let Some(id) = v.as_str() {
                            workflows.insert(id.to_string());
                        }
                    }
                    _ => recursive_find_deps(v, agents, tools, workflows),
                }
            }
        }
        Value::Array(arr) => {
            for v in arr {
                recursive_find_deps(v, agents, tools, workflows);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_find_agent_deps() {
        // Test simple string format
        let agent_simple = json!({
            "id": "agent-1",
            "configuration": {
                "tools": ["tool-a", "tool-b"]
            }
        });
        let deps = find_agent_dependencies(&agent_simple);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&Dependency::Tool("tool-a".to_string())));
        assert!(deps.contains(&Dependency::Tool("tool-b".to_string())));

        // Test nested object format
        let agent_nested = json!({
            "id": "agent-2",
            "configuration": {
                "tools": [
                    {
                        "tool_ids": ["tool-c", "tool-d"]
                    }
                ]
            }
        });
        let deps = find_agent_dependencies(&agent_nested);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&Dependency::Tool("tool-c".to_string())));
        assert!(deps.contains(&Dependency::Tool("tool-d".to_string())));
    }

    #[test]
    fn test_find_tool_deps() {
        let tool = json!({
            "id": "tool-1",
            "configuration": {
                "workflow_id": "wf-1"
            }
        });
        let deps = find_tool_dependencies(&tool);
        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&Dependency::Workflow("wf-1".to_string())));
    }

    #[test]
    fn test_find_workflow_deps() {
        let workflow = json!({
            "id": "wf-1",
            "definition": {
                "nodes": [
                    { "agent_id": "agent-1" },
                    { "toolId": "tool-x" },
                    { "tool_ids": ["tool-y", "tool-z"] },
                    { "sub_workflow": { "workflowId": "wf-2" } }
                ]
            }
        });
        let deps = find_workflow_dependencies(&workflow);
        assert_eq!(deps.len(), 5);
        assert!(deps.contains(&Dependency::Agent("agent-1".to_string())));
        assert!(deps.contains(&Dependency::Tool("tool-x".to_string())));
        assert!(deps.contains(&Dependency::Tool("tool-y".to_string())));
        assert!(deps.contains(&Dependency::Tool("tool-z".to_string())));
        assert!(deps.contains(&Dependency::Workflow("wf-2".to_string())));
    }
}
