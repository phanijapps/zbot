//! # Agent Service
//!
//! Service for managing agent configurations.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent configuration stored in config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "agentType", default)]
    pub agent_type: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens", default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(rename = "thinkingEnabled", default)]
    pub thinking_enabled: bool,
    #[serde(rename = "voiceRecordingEnabled", default = "default_voice_recording_enabled")]
    pub voice_recording_enabled: bool,
    pub skills: Vec<String>,
    pub mcps: Vec<String>,
    #[serde(rename = "systemInstruction", default, skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_voice_recording_enabled() -> bool {
    true
}

/// Full agent data including instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "agentType", default)]
    pub agent_type: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(rename = "thinkingEnabled")]
    pub thinking_enabled: bool,
    #[serde(rename = "voiceRecordingEnabled")]
    pub voice_recording_enabled: bool,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
    pub instructions: String,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middleware: Option<String>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Agent service for managing agent configurations.
pub struct AgentService {
    agents_dir: PathBuf,
    cache: RwLock<Option<Vec<Agent>>>,
}

impl AgentService {
    /// Create a new agent service.
    pub fn new(agents_dir: PathBuf) -> Self {
        Self {
            agents_dir,
            cache: RwLock::new(None),
        }
    }

    /// Get the agents directory.
    pub fn agents_dir(&self) -> &PathBuf {
        &self.agents_dir
    }

    /// List all agents.
    pub async fn list(&self) -> Result<Vec<Agent>, String> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(agents) = cache.as_ref() {
                return Ok(agents.clone());
            }
        }

        // Read from disk
        let agents = self.list_from_disk()?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(agents.clone());
        }

        Ok(agents)
    }

    /// List agents from disk (bypasses cache).
    fn list_from_disk(&self) -> Result<Vec<Agent>, String> {
        if !self.agents_dir.exists() {
            return Ok(vec![]);
        }

        let mut agents = Vec::new();

        let entries = fs::read_dir(&self.agents_dir)
            .map_err(|e| format!("Failed to read agents directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let config_yaml = path.join("config.yaml");
            if !config_yaml.exists() {
                continue;
            }

            if let Ok(agent) = self.read_agent_folder(&path) {
                agents.push(agent);
            }
        }

        Ok(agents)
    }

    /// Get a single agent by ID.
    pub async fn get(&self, id: &str) -> Result<Agent, String> {
        let agent_dir = self.agents_dir.join(id);

        if !agent_dir.exists() {
            return Err(format!("Agent not found: {}", id));
        }

        self.read_agent_folder(&agent_dir)
    }

    /// Create a new agent.
    pub async fn create(&self, agent: Agent) -> Result<Agent, String> {
        // Ensure agents directory exists
        fs::create_dir_all(&self.agents_dir)
            .map_err(|e| format!("Failed to create agents directory: {}", e))?;

        // Create agent directory
        let agent_dir = self.agents_dir.join(&agent.name);
        fs::create_dir_all(&agent_dir)
            .map_err(|e| format!("Failed to create agent directory: {}", e))?;

        // Write config.yaml
        let config = AgentConfig {
            name: agent.name.clone(),
            display_name: agent.display_name.clone(),
            description: agent.description.clone(),
            provider_id: agent.provider_id.clone(),
            model: agent.model.clone(),
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            thinking_enabled: agent.thinking_enabled,
            voice_recording_enabled: agent.voice_recording_enabled,
            skills: agent.skills.clone(),
            mcps: agent.mcps.clone(),
            agent_type: agent.agent_type.clone(),
            system_instruction: agent.system_instruction.clone(),
        };

        let config_yaml = serde_yaml::to_string(&config)
            .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;

        // Append middleware YAML if provided
        let final_yaml = if let Some(middleware_yaml) = &agent.middleware {
            format!("{}\n{}", config_yaml.trim_end(), middleware_yaml.trim_end())
        } else {
            config_yaml
        };

        fs::write(agent_dir.join("config.yaml"), final_yaml)
            .map_err(|e| format!("Failed to write config.yaml: {}", e))?;

        // Write AGENTS.md
        fs::write(agent_dir.join("AGENTS.md"), format!("{}\n", agent.instructions))
            .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(Agent {
            id: agent.name.clone(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            ..agent
        })
    }

    /// Update an existing agent.
    pub async fn update(&self, id: &str, agent: Agent) -> Result<Agent, String> {
        let agent_dir = self.agents_dir.join(id);

        if !agent_dir.exists() {
            return Err(format!("Agent not found: {}", id));
        }

        // If name changed, rename directory
        if agent.name != id {
            let new_dir = self.agents_dir.join(&agent.name);
            fs::rename(&agent_dir, &new_dir)
                .map_err(|e| format!("Failed to rename agent directory: {}", e))?;
        }

        // Use the new directory name if changed
        let target_dir = self.agents_dir.join(&agent.name);

        // Write config.yaml
        let config = AgentConfig {
            name: agent.name.clone(),
            display_name: agent.display_name.clone(),
            description: agent.description.clone(),
            provider_id: agent.provider_id.clone(),
            model: agent.model.clone(),
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            thinking_enabled: agent.thinking_enabled,
            voice_recording_enabled: agent.voice_recording_enabled,
            skills: agent.skills.clone(),
            mcps: agent.mcps.clone(),
            agent_type: agent.agent_type.clone(),
            system_instruction: agent.system_instruction.clone(),
        };

        let config_yaml = serde_yaml::to_string(&config)
            .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;

        let final_yaml = if let Some(middleware_yaml) = &agent.middleware {
            format!("{}\n{}", config_yaml.trim_end(), middleware_yaml.trim_end())
        } else {
            config_yaml
        };

        fs::write(target_dir.join("config.yaml"), final_yaml)
            .map_err(|e| format!("Failed to write config.yaml: {}", e))?;

        // Write AGENTS.md
        fs::write(target_dir.join("AGENTS.md"), format!("{}\n", agent.instructions))
            .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(agent)
    }

    /// Delete an agent.
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let agent_path = self.agents_dir.join(id);

        if !agent_path.exists() {
            return Err(format!("Agent not found: {}", id));
        }

        fs::remove_dir_all(&agent_path)
            .map_err(|e| format!("Failed to delete agent directory: {}", e))?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(())
    }

    /// Invalidate the agent cache.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    /// Read an agent folder and parse config.yaml and AGENTS.md
    fn read_agent_folder(&self, agent_dir: &PathBuf) -> Result<Agent, String> {
        let config_path = agent_dir.join("config.yaml");
        let agents_md_path = agent_dir.join("AGENTS.md");

        if !config_path.exists() {
            return Err(format!("config.yaml not found in {:?}", agent_dir));
        }

        if !agents_md_path.exists() {
            return Err(format!("AGENTS.md not found in {:?}", agent_dir));
        }

        // Read config.yaml
        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.yaml: {}", e))?;
        let config: AgentConfig = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.yaml: {}", e))?;

        // Read AGENTS.md
        let instructions = fs::read_to_string(&agents_md_path)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?;

        // Get agent name from directory name
        let name = agent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Agent {
            id: name.clone(),
            name,
            display_name: config.display_name,
            description: config.description,
            agent_type: config.agent_type,
            provider_id: config.provider_id,
            model: config.model,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            thinking_enabled: config.thinking_enabled,
            voice_recording_enabled: config.voice_recording_enabled,
            system_instruction: config.system_instruction,
            instructions,
            mcps: config.mcps,
            skills: config.skills,
            middleware: None,
            created_at: Some("1970-01-01T00:00:00Z".to_string()),
        })
    }

    /// Seed default subagents if they don't exist.
    ///
    /// Accepts agent definitions as JSON (loaded from `default_agents.json` by caller).
    /// Falls back to hardcoded defaults if `template_json` is None.
    /// Seed default subagents if they don't exist.
    ///
    /// - `template_json`: Agent definitions from `default_agents.json`
    /// - `instructions_loader`: Callback to load AGENTS.md from embedded templates by agent name
    pub async fn seed_default_agents(
        &self,
        default_provider_id: &str,
        default_model: &str,
        template_json: Option<&[u8]>,
        instructions_loader: impl Fn(&str) -> Option<String>,
    ) -> Result<(), String> {
        let template_agents: Vec<serde_json::Value> = template_json
            .and_then(|data| serde_json::from_slice(data).ok())
            .unwrap_or_else(|| {
                serde_json::json!([
                    {"name": "research-agent", "displayName": "Research Agent", "description": "Specialized in gathering, analyzing, and synthesizing information from various sources.", "temperature": 0.7, "maxTokens": 8192, "skills": [], "mcps": []},
                    {"name": "code-agent", "displayName": "Code Agent", "description": "Specialized in code generation, review, debugging, and software engineering tasks.", "temperature": 0.7, "maxTokens": 8192, "skills": [], "mcps": []},
                    {"name": "writing-agent", "displayName": "Writing Agent", "description": "Specialized in content creation, editing, and written communication.", "temperature": 0.7, "maxTokens": 8192, "skills": [], "mcps": []}
                ]).as_array().cloned().unwrap_or_default()
            });

        for entry in &template_agents {
            let name = entry["name"].as_str().unwrap_or_default();
            if name.is_empty() { continue; }

            if self.get(name).await.is_ok() {
                tracing::debug!("Agent {} already exists, skipping seed", name);
                continue;
            }

            tracing::info!("Seeding default agent: {}", name);

            let display_name = entry["displayName"].as_str().unwrap_or(name);
            let description = entry["description"].as_str().unwrap_or("");
            let agent_type = entry["agentType"].as_str().unwrap_or("specialist");
            let temperature = entry["temperature"].as_f64().unwrap_or(0.7);
            let max_tokens = entry["maxTokens"].as_u64().unwrap_or(8192) as u32;
            let skills: Vec<String> = entry["skills"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                .unwrap_or_default();
            let mcps: Vec<String> = entry["mcps"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                .unwrap_or_default();

            // Load instructions: try bundled template first, then hardcoded fallback
            let instructions = instructions_loader(name)
                .or_else(|| match name {
                    "research-agent" => Some(RESEARCH_AGENT_INSTRUCTIONS.to_string()),
                    "code-agent" => Some(CODE_AGENT_INSTRUCTIONS.to_string()),
                    "writing-agent" => Some(WRITING_AGENT_INSTRUCTIONS.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "You are a helpful AI assistant.".to_string());

            let agent = Agent {
                id: name.to_string(),
                name: name.to_string(),
                display_name: display_name.to_string(),
                description: description.to_string(),
                agent_type: Some(agent_type.to_string()),
                provider_id: default_provider_id.to_string(),
                model: default_model.to_string(),
                temperature,
                max_tokens,
                thinking_enabled: false,
                voice_recording_enabled: false,
                system_instruction: None,
                instructions,
                mcps,
                skills,
                middleware: None,
                created_at: None,
            };

            if let Err(e) = self.create(agent).await {
                tracing::warn!("Failed to seed agent {}: {}", name, e);
            }
        }

        Ok(())
    }
}

// ============================================================================
// DEFAULT AGENT INSTRUCTIONS
// ============================================================================

const RESEARCH_AGENT_INSTRUCTIONS: &str = r#"# Research Agent

You are a specialized research agent focused on gathering, analyzing, and synthesizing information.

## Capabilities

- Deep research on any topic
- Information synthesis and summarization
- Fact-checking and verification
- Source evaluation and citation

## Approach

1. **Understand the Query**: Clarify what information is needed
2. **Gather Information**: Use available tools to search and collect data
3. **Analyze**: Evaluate sources, identify patterns, cross-reference facts
4. **Synthesize**: Combine findings into a coherent summary
5. **Report**: Provide clear, well-structured findings

## Output Format

Structure your findings as:
- **Summary**: Key findings in 2-3 sentences
- **Details**: Organized by subtopic
- **Sources**: List of references used
- **Confidence**: How confident you are in the findings

## Guidelines

- Prioritize accuracy over speed
- Cite sources when possible
- Acknowledge uncertainty
- Provide balanced perspectives on controversial topics
"#;

const CODE_AGENT_INSTRUCTIONS: &str = r#"# Code Agent

You are a specialized coding agent focused on software development tasks.

## Capabilities

- Code generation in any language
- Code review and analysis
- Debugging and troubleshooting
- Architecture design
- Best practices and optimization

## Approach

1. **Understand Requirements**: Clarify what needs to be built
2. **Design**: Plan the structure before coding
3. **Implement**: Write clean, maintainable code
4. **Test**: Verify the code works correctly
5. **Document**: Add appropriate comments and documentation

## Output Format

When providing code:
- Use proper syntax highlighting
- Include comments explaining complex logic
- Provide usage examples when helpful
- Note any dependencies or prerequisites

## Guidelines

- Follow language-specific best practices
- Write readable, maintainable code
- Consider edge cases and error handling
- Suggest improvements when reviewing existing code
- Keep security in mind
"#;

const WRITING_AGENT_INSTRUCTIONS: &str = r#"# Writing Agent

You are a specialized writing agent focused on content creation and communication.

## Capabilities

- Content creation (articles, blog posts, documentation)
- Editing and proofreading
- Tone and style adaptation
- Summarization and expansion
- Translation and localization support

## Approach

1. **Understand the Brief**: Clarify purpose, audience, tone
2. **Outline**: Structure the content logically
3. **Draft**: Write the initial content
4. **Refine**: Edit for clarity, flow, and impact
5. **Polish**: Final proofreading and formatting

## Output Format

- Use clear headings and structure
- Match the requested tone and style
- Include formatting appropriate for the medium
- Provide word count if requested

## Guidelines

- Adapt tone to audience (professional, casual, technical)
- Be concise without sacrificing clarity
- Use active voice when possible
- Vary sentence structure for readability
- Fact-check any claims made
"#;

/// Create a shared agent service.
pub fn shared_agent_service(agents_dir: PathBuf) -> Arc<AgentService> {
    Arc::new(AgentService::new(agents_dir))
}
