//! # Goal Adapter
//!
//! Bridges [`zero_stores_sqlite::GoalRepository`] to [`agent_tools::GoalAccess`]
//! so the `goal` tool can create/update/list agent goals.

use async_trait::async_trait;
use std::sync::Arc;

use agent_tools::{GoalAccess, GoalSummary};
use zero_stores_sqlite::{Goal, GoalRepository};

/// Adapter that implements [`GoalAccess`] by delegating to a [`GoalRepository`].
pub struct GoalAdapter {
    repo: Arc<GoalRepository>,
}

impl GoalAdapter {
    pub fn new(repo: Arc<GoalRepository>) -> Self {
        Self { repo }
    }
}

fn to_summary(g: Goal) -> GoalSummary {
    GoalSummary {
        id: g.id,
        title: g.title,
        description: g.description,
        state: g.state,
        slots: g.slots,
        filled_slots: g.filled_slots,
    }
}

#[async_trait]
impl GoalAccess for GoalAdapter {
    async fn create(
        &self,
        agent_id: &str,
        title: &str,
        description: Option<&str>,
        slots_json: Option<&str>,
    ) -> std::result::Result<GoalSummary, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let goal = Goal {
            id: format!("goal-{}", uuid::Uuid::new_v4()),
            agent_id: agent_id.to_string(),
            ward_id: None,
            title: title.to_string(),
            description: description.map(String::from),
            state: "active".to_string(),
            parent_goal_id: None,
            slots: slots_json.map(String::from),
            filled_slots: None,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
        };
        self.repo.create(&goal)?;
        Ok(to_summary(goal))
    }

    async fn update_state(
        &self,
        goal_id: &str,
        new_state: &str,
    ) -> std::result::Result<(), String> {
        self.repo.update_state(goal_id, new_state)
    }

    async fn update_filled_slots(
        &self,
        goal_id: &str,
        filled_slots_json: &str,
    ) -> std::result::Result<(), String> {
        self.repo.update_filled_slots(goal_id, filled_slots_json)
    }

    async fn list_active(&self, agent_id: &str) -> std::result::Result<Vec<GoalSummary>, String> {
        Ok(self
            .repo
            .list_active(agent_id)?
            .into_iter()
            .map(to_summary)
            .collect())
    }

    async fn get(&self, goal_id: &str) -> std::result::Result<Option<GoalSummary>, String> {
        Ok(self.repo.get(goal_id)?.map(to_summary))
    }
}
