//! Query session data for debugging
//! Run: cargo run -p gateway --example query_session -- <session_id>

use rusqlite::{params, Connection};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let session_id = args.get(1).map(|s| s.as_str()).unwrap_or("sess-9fa1604d-a82b-48e1-a47e-176d594a3b7b");

    let db_path = r"C:\Users\rampi\Documents\zbot\conversations.db";
    let conn = Connection::open(db_path)?;

    println!("\n=== SESSION: {} ===\n", session_id);

    // Query session
    let mut stmt = conn.prepare("SELECT id, status, root_agent_id, pending_delegations, continuation_needed FROM sessions WHERE id = ?1")?;
    if let Ok((id, status, agent, pending, cont)) = stmt.query_row(params![session_id], |row| {
        Ok::<_, rusqlite::Error>((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    }) {
        println!("Session: {} | Status: {} | Agent: {} | Pending: {} | Continuation: {}", id, status, agent, pending, cont);
    }

    println!("\n=== EXECUTIONS ===\n");

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
        println!("{} | {:20} | {:10} | {:10} | parent: {:?}",
            &id[..24], agent, dtype, status, parent.as_ref().map(|p| &p[..24.min(p.len())]));
    }

    println!("\n=== MESSAGES (by execution) ===\n");

    let mut stmt = conn.prepare(
        "SELECT m.execution_id, m.role, substr(m.content, 1, 150), e.delegation_type, e.agent_id
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
        let agent: String = row.get(4)?;
        println!("[{} | {}] {} ({}): {}",
            &exec[..16], agent, role, dtype,
            content.replace('\n', " ").chars().take(80).collect::<String>());
    }

    Ok(())
}
