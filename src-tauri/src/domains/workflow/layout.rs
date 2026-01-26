// ============================================================================
// WORKFLOW LAYOUT
// Types and functions for .workflow/layout.json (visual state)
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================================================
// LAYOUT TYPES
// ============================================================================

/// Workflow layout (stored in .workflow/layout.json)
/// This is purely visual state, not versioned logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowLayout {
    /// Schema version
    #[serde(default = "default_version")]
    pub version: u32,

    /// Viewport state (pan and zoom)
    #[serde(default)]
    pub viewport: Viewport,

    /// Node positions keyed by node ID
    #[serde(default)]
    pub nodes: HashMap<String, Position>,
}

/// Viewport state for the canvas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

/// Position of a node on the canvas
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

fn default_version() -> u32 {
    1
}

fn default_zoom() -> f64 {
    1.0
}

// ============================================================================
// LAYOUT OPERATIONS
// ============================================================================

impl WorkflowLayout {
    /// Create a new empty layout
    pub fn new() -> Self {
        Self {
            version: 1,
            viewport: Viewport::default(),
            nodes: HashMap::new(),
        }
    }

    /// Load layout from .workflow/layout.json
    pub fn load(agent_dir: &Path) -> Result<Self, String> {
        let layout_path = agent_dir.join(".workflow").join("layout.json");

        if !layout_path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&layout_path)
            .map_err(|e| format!("Failed to read layout.json: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse layout.json: {}", e))
    }

    /// Save layout to .workflow/layout.json
    pub fn save(&self, agent_dir: &Path) -> Result<(), String> {
        let workflow_dir = agent_dir.join(".workflow");

        // Ensure .workflow directory exists
        fs::create_dir_all(&workflow_dir)
            .map_err(|e| format!("Failed to create .workflow directory: {}", e))?;

        let layout_path = workflow_dir.join("layout.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize layout.json: {}", e))?;

        fs::write(&layout_path, content)
            .map_err(|e| format!("Failed to write layout.json: {}", e))?;

        Ok(())
    }

    /// Get position for a node, with default if not found
    pub fn get_position(&self, node_id: &str) -> Position {
        self.nodes.get(node_id).copied().unwrap_or_default()
    }

    /// Set position for a node
    pub fn set_position(&mut self, node_id: String, position: Position) {
        self.nodes.insert(node_id, position);
    }

    /// Remove position for a node
    pub fn remove_position(&mut self, node_id: &str) {
        self.nodes.remove(node_id);
    }

    /// Generate default layout for a list of node IDs
    /// Arranges nodes in a vertical pipeline layout
    pub fn generate_default(&mut self, node_ids: &[String]) {
        let start_x = 250.0;
        let start_y = 50.0;
        let y_spacing = 150.0;

        // Start node
        self.set_position("start".to_string(), Position { x: start_x, y: start_y });

        // Subagent nodes
        for (i, node_id) in node_ids.iter().enumerate() {
            let y = start_y + 150.0 + (i as f64) * y_spacing;
            self.set_position(node_id.clone(), Position { x: start_x, y });
        }

        // End node
        let end_y = start_y + 150.0 + (node_ids.len() as f64) * y_spacing;
        self.set_position("end".to_string(), Position { x: start_x, y: end_y });
    }

    /// Generate layout for parallel nodes (horizontal arrangement)
    pub fn generate_parallel(&mut self, node_ids: &[String], row_y: f64) {
        let x_spacing = 280.0;
        let start_x = 100.0;

        for (i, node_id) in node_ids.iter().enumerate() {
            let x = start_x + (i as f64) * x_spacing;
            self.set_position(node_id.clone(), Position { x, y: row_y });
        }
    }
}

impl Default for WorkflowLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let mut layout = WorkflowLayout::new();
        let nodes = vec![
            "inventory_checker".to_string(),
            "recipe_finder".to_string(),
            "instruction_formatter".to_string(),
        ];

        layout.generate_default(&nodes);

        assert!(layout.nodes.contains_key("start"));
        assert!(layout.nodes.contains_key("end"));
        assert!(layout.nodes.contains_key("inventory_checker"));

        let start_pos = layout.get_position("start");
        let first_pos = layout.get_position("inventory_checker");
        assert!(first_pos.y > start_pos.y);
    }

    #[test]
    fn test_serialize_layout() {
        let mut layout = WorkflowLayout::new();
        layout.set_position("test".to_string(), Position { x: 100.0, y: 200.0 });

        let json = serde_json::to_string(&layout).unwrap();
        let parsed: WorkflowLayout = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.get_position("test").x, 100.0);
        assert_eq!(parsed.get_position("test").y, 200.0);
    }
}
