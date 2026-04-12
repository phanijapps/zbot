//! Working Memory — live, mutable context that evolves during execution.
//!
//! Tracks active entities, session discoveries, corrections, and delegation
//! status. Injected as a system message before each LLM iteration.

use indexmap::IndexMap;

/// An entity actively tracked in working memory.
#[derive(Debug, Clone)]
pub struct WorkingEntity {
    pub name: String,
    pub entity_type: Option<String>,
    pub summary: String,
    pub last_referenced_iteration: u32,
}

/// A discovery made during the session.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub content: String,
    pub iteration: u32,
    pub source: String,
}

/// Summary of a delegation's status and findings.
#[derive(Debug, Clone)]
pub struct DelegationSummary {
    pub agent_id: String,
    pub task_summary: String,
    pub key_findings: Vec<String>,
    pub status: String,
}

/// Live working memory that updates each iteration.
///
/// Budget-managed: when total tokens exceed `token_budget`,
/// least-recently-referenced entities are evicted first.
pub struct WorkingMemory {
    entities: IndexMap<String, WorkingEntity>,
    discoveries: Vec<Discovery>,
    corrections: Vec<String>,
    delegations: Vec<DelegationSummary>,
    token_budget: usize,
}

impl WorkingMemory {
    /// Create a new working memory with the given token budget.
    pub fn new(token_budget: usize) -> Self {
        Self {
            entities: IndexMap::new(),
            discoveries: Vec::new(),
            corrections: Vec::new(),
            delegations: Vec::new(),
            token_budget,
        }
    }

    /// Add or update an entity in working memory.
    pub fn add_entity(
        &mut self,
        name: &str,
        entity_type: Option<&str>,
        summary: &str,
        iteration: u32,
    ) {
        let key = name.to_lowercase();
        if let Some(existing) = self.entities.get_mut(&key) {
            existing.summary = summary.to_string();
            existing.last_referenced_iteration = iteration;
            if entity_type.is_some() {
                existing.entity_type = entity_type.map(|s| s.to_string());
            }
        } else {
            self.entities.insert(
                key,
                WorkingEntity {
                    name: name.to_string(),
                    entity_type: entity_type.map(|s| s.to_string()),
                    summary: summary.to_string(),
                    last_referenced_iteration: iteration,
                },
            );
        }
        self.evict_if_over_budget();
    }

    /// Record a session discovery.
    pub fn add_discovery(&mut self, content: &str, iteration: u32, source: &str) {
        // Avoid duplicate discoveries
        if self.discoveries.iter().any(|d| d.content == content) {
            return;
        }
        self.discoveries.push(Discovery {
            content: content.to_string(),
            iteration,
            source: source.to_string(),
        });
        self.evict_if_over_budget();
    }

    /// Record an active correction.
    pub fn add_correction(&mut self, correction: &str) {
        if !self.corrections.contains(&correction.to_string()) {
            self.corrections.push(correction.to_string());
        }
    }

    /// Update delegation status and findings.
    pub fn update_delegation(&mut self, agent_id: &str, status: &str, findings: Vec<String>) {
        if let Some(d) = self.delegations.iter_mut().find(|d| d.agent_id == agent_id) {
            d.status = status.to_string();
            if !findings.is_empty() {
                d.key_findings = findings;
            }
        } else {
            self.delegations.push(DelegationSummary {
                agent_id: agent_id.to_string(),
                task_summary: String::new(),
                key_findings: findings,
                status: status.to_string(),
            });
        }
    }

    /// Set the task summary for a delegation (called when delegation starts).
    pub fn set_delegation_task(&mut self, agent_id: &str, task: &str) {
        if let Some(d) = self.delegations.iter_mut().find(|d| d.agent_id == agent_id) {
            d.task_summary = task.to_string();
        } else {
            self.delegations.push(DelegationSummary {
                agent_id: agent_id.to_string(),
                task_summary: task.to_string(),
                key_findings: Vec::new(),
                status: "running".to_string(),
            });
        }
    }

    /// Estimated token count (chars / 4).
    pub fn token_count(&self) -> usize {
        self.format_for_prompt().len() / 4
    }

    /// Evict least-recently-referenced entities until under budget.
    pub fn evict_if_over_budget(&mut self) {
        while self.token_count() > self.token_budget && !self.entities.is_empty() {
            // Find entity with lowest last_referenced_iteration
            let lru_key = self
                .entities
                .iter()
                .min_by_key(|(_, e)| e.last_referenced_iteration)
                .map(|(k, _)| k.clone());

            if let Some(key) = lru_key {
                self.entities.shift_remove(&key);
            } else {
                break;
            }
        }

        // Also evict old discoveries if still over budget
        while self.token_count() > self.token_budget && !self.discoveries.is_empty() {
            self.discoveries.remove(0); // Remove oldest first
        }
    }

    /// Format working memory as markdown for system prompt injection.
    pub fn format_for_prompt(&self) -> String {
        let mut output = String::from("## Working Memory (auto-updated)\n");

        if !self.entities.is_empty() {
            output.push_str("\n### Active Entities\n");
            for entity in self.entities.values() {
                let type_label = entity
                    .entity_type
                    .as_deref()
                    .map(|t| format!(" ({t})"))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "- **{}**{}: {}\n",
                    entity.name, type_label, entity.summary
                ));
            }
        }

        if !self.discoveries.is_empty() {
            output.push_str("\n### Session Discoveries\n");
            for d in &self.discoveries {
                output.push_str(&format!(
                    "- {} [iter {}, {}]\n",
                    d.content, d.iteration, d.source
                ));
            }
        }

        if !self.corrections.is_empty() {
            output.push_str("\n### Active Corrections\n");
            for c in &self.corrections {
                output.push_str(&format!("- {c}\n"));
            }
        }

        if !self.delegations.is_empty() {
            output.push_str("\n### Delegation Status\n");
            for d in &self.delegations {
                let task = if d.task_summary.is_empty() {
                    String::new()
                } else {
                    format!(" \u{2014} {}", truncate_str(&d.task_summary, 80))
                };
                if d.key_findings.is_empty() {
                    output.push_str(&format!("- {}: {}{}\n", d.agent_id, d.status, task));
                } else {
                    let findings = d.key_findings.join("; ");
                    output.push_str(&format!(
                        "- {}: {}{} \u{2014} {}\n",
                        d.agent_id,
                        d.status,
                        task,
                        truncate_str(&findings, 120)
                    ));
                }
            }
        }

        output
    }

    /// Check whether an entity (by name) is already tracked.
    pub fn has_entity(&self, name: &str) -> bool {
        self.entities.contains_key(&name.to_lowercase())
    }

    /// Whether working memory has any content worth injecting.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
            && self.discoveries.is_empty()
            && self.corrections.is_empty()
            && self.delegations.is_empty()
    }
}

/// Truncate a string to max_len, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_entity_and_format() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_entity(
            "yfinance",
            Some("module"),
            "Python library for stock data",
            1,
        );
        let output = wm.format_for_prompt();
        assert!(output.contains("**yfinance** (module): Python library for stock data"));
    }

    #[test]
    fn test_add_entity_updates_existing() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_entity("SPY", None, "S&P 500 ETF", 1);
        wm.add_entity("SPY", None, "S&P 500 ETF. Price: $523", 3);
        let output = wm.format_for_prompt();
        assert!(output.contains("Price: $523"));
        // Should only appear once
        assert_eq!(output.matches("SPY").count(), 1);
    }

    #[test]
    fn test_add_discovery_deduplicates() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_discovery("API is paginated", 5, "shell");
        wm.add_discovery("API is paginated", 6, "shell");
        assert_eq!(wm.discoveries.len(), 1);
    }

    #[test]
    fn test_add_correction() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_correction("Use plotly not matplotlib");
        wm.add_correction("Use plotly not matplotlib"); // dup
        let output = wm.format_for_prompt();
        assert!(output.contains("Use plotly not matplotlib"));
        assert_eq!(output.matches("plotly").count(), 1);
    }

    #[test]
    fn test_delegation_lifecycle() {
        let mut wm = WorkingMemory::new(5000);
        wm.set_delegation_task("research-agent", "fetch stock data");
        wm.update_delegation(
            "research-agent",
            "completed",
            vec!["found 8 sources".into()],
        );
        let output = wm.format_for_prompt();
        assert!(output.contains("research-agent: completed"));
        assert!(output.contains("found 8 sources"));
    }

    #[test]
    fn test_eviction_removes_lru_entity() {
        // Very small budget to force eviction (30 tokens fits one entity but not two)
        let mut wm = WorkingMemory::new(30);
        wm.add_entity("old_entity", None, "should be evicted because LRU", 1);
        wm.add_entity("new_entity", None, "should survive because recent", 10);
        // After eviction, old_entity should be gone
        let output = wm.format_for_prompt();
        assert!(!output.contains("old_entity"));
    }

    #[test]
    fn test_is_empty() {
        let wm = WorkingMemory::new(5000);
        assert!(wm.is_empty());
    }

    #[test]
    fn test_format_empty() {
        let wm = WorkingMemory::new(5000);
        let output = wm.format_for_prompt();
        assert!(output.contains("Working Memory"));
        // No sections rendered
        assert!(!output.contains("Active Entities"));
    }
}
