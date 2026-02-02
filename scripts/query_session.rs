//! Quick script to query session data
//! Run with: cargo run --manifest-path scripts/Cargo.toml -- <session_id>

use rusqlite::{Connection, params};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let session_id = args.get(1).map(|s| s.as_str()).unwrap_or("sess-9fa1604d-a82b-48e1-a47e-176d594a3b7b");

    let db_path = r"C:\Users\rampi\Documents\agentzero\conversations.db";
    let conn = Connection::open(db_path)?;

    println!("\n=== SESSION: {} ===\n", session_id);

    // Query session
    let mut stmt = conn.prepare("SELECT id, status, root_agent_id, pending_delegations, continuation_needed FROM sessions WHERE id = ?1")?;
    let session: Result<(String, String, String, i64, i64), _> = stmt.query_row(params![session_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    });

    if let Ok((id, status, agent, pending, cont)) = session {
        println!("Session: {} | Status: {} | Agent: {} | Pending: {} | Continuation: {}", id, status, agent, pending, cont);
    }

    println!("\n=== EXECUTIONS ===\n");

    // Query executions
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, delegation_type, status, parent_execution_id
         FROM agent_executions WHERE session_id = ?1 ORDER BY started_at"
    )?;
    let mut rows = stmt.query(params![session_id])?;

    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let agent: String = row.get(1)?;
        let dtype: String = row.get(2)?;
        let status: String = row.get(3)?;
        let parent: Option<String> = row.get(4)?;
        println!("{} | {} | {} | {} | parent: {:?}",
            &id[..20], agent, dtype, status, parent.map(|p| p[..20].to_string()));
    }

    println!("\n=== MESSAGES ===\n");

    // Query messages for all executions in session
    let mut stmt = conn.prepare(
        "SELECT m.execution_id, m.role, substr(m.content, 1, 100), e.delegation_type
         FROM messages m
         JOIN agent_executions e ON m.execution_id = e.id
         WHERE e.session_id = ?1
         ORDER BY m.created_at"
    )?;
    let mut rows = stmt.query(params![session_id])?;

    while let Some(row) = rows.next()? {
        let exec: String = row.get(0)?;
        let role: String = row.get(1)?;
        let content: String = row.get(2)?;
        let dtype: String = row.get(3)?;
        println!("[{}] {} ({}): {}", &exec[..12], role, dtype, content.replace('\n', " "));
    }

    Ok(())
}
