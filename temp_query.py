import sqlite3
import json

db_path = r"C:\Users\rampi\Documents\agentzero\conversations.db"
conn = sqlite3.connect(db_path)
conn.row_factory = sqlite3.Row
c = conn.cursor()

session_id = "sess-ad8ee60c-529e-40f1-919d-1d42b4ecae48"

# 1. Session info
print("=== SESSION ===")
c.execute("SELECT id, status, root_agent_id, parent_session_id, ward_id, created_at, total_tokens_in, total_tokens_out FROM sessions WHERE id=?", (session_id,))
row = c.fetchone()
if row:
    for key in row.keys():
        print(f"  {key}: {row[key]}")

# 2. Executions
print("\n=== EXECUTIONS ===")
c.execute("SELECT id, agent_id, delegation_type, status, tokens_in, tokens_out FROM agent_executions WHERE session_id=?", (session_id,))
for row in c.fetchall():
    print(f"  {row['id']} | agent={row['agent_id']} | type={row['delegation_type']} | status={row['status']} | tokens={row['tokens_in']}/{row['tokens_out']}")

# 3. Child sessions
print("\n=== CHILD SESSIONS ===")
c.execute("SELECT id, root_agent_id, status FROM sessions WHERE parent_session_id=?", (session_id,))
rows = c.fetchall()
if rows:
    for row in rows:
        print(f"  {row['id']} | agent={row['root_agent_id']} | status={row['status']}")
else:
    print("  (none)")

# 4. Message counts by role
print("\n=== MESSAGE COUNTS BY ROLE ===")
c.execute("SELECT role, COUNT(*) as cnt FROM messages WHERE session_id=? GROUP BY role ORDER BY cnt DESC", (session_id,))
for row in c.fetchall():
    print(f"  {row['role']}: {row['cnt']}")

# 5. Total messages
c.execute("SELECT COUNT(*) as total FROM messages WHERE session_id=?", (session_id,))
total = c.fetchone()['total']
print(f"\n  TOTAL MESSAGES: {total}")

# 6. ID prefix check
print("\n=== ID PREFIX CHECK ===")
c.execute("SELECT id FROM messages WHERE session_id=? LIMIT 5", (session_id,))
for row in c.fetchall():
    prefix = "msg-" if row['id'].startswith("msg-") else "bare UUID"
    print(f"  {row['id'][:40]}... ({prefix})")

# 7. All messages timeline with gaps
print("\n=== FULL MESSAGE TIMELINE ===")
c.execute("""
    SELECT created_at, role, substr(content, 1, 100) as preview, tool_call_id,
           CASE WHEN tool_calls IS NOT NULL THEN 'has_tool_calls' ELSE NULL END as has_tc
    FROM messages 
    WHERE session_id=? 
    ORDER BY created_at ASC
""", (session_id,))
rows = c.fetchall()
prev_time = None
for i, row in enumerate(rows):
    gap = ""
    if prev_time and row['created_at']:
        from datetime import datetime
        try:
            t1 = datetime.fromisoformat(prev_time.replace('Z', '+00:00'))
            t2 = datetime.fromisoformat(row['created_at'].replace('Z', '+00:00'))
            diff = (t2 - t1).total_seconds()
            if diff > 5:
                gap = f" [GAP: {diff:.1f}s]"
        except:
            pass
    tc_id = f" tc_id={row['tool_call_id']}" if row['tool_call_id'] else ""
    tc = f" [{row['has_tc']}]" if row['has_tc'] else ""
    preview = (row['preview'] or '').replace('\n', '\n')[:80]
    print(f"  {i+1:3d}. [{row['created_at'][-12:]}] {row['role']:10s}{tc}{tc_id}{gap}")
    print(f"       {preview}")
    prev_time = row['created_at']

# 8. Check for TRUNCATED errors
print("\n=== TRUNCATED ARGUMENTS ===")
c.execute("SELECT COUNT(*) as cnt FROM messages WHERE session_id=? AND content LIKE '%TRUNCATED%'", (session_id,))
cnt = c.fetchone()['cnt']
print(f"  Count: {cnt}")

# 9. Timing stats
print("\n=== TIMING ===")
c.execute("SELECT MIN(created_at) as first, MAX(created_at) as last FROM messages WHERE session_id=?", (session_id,))
row = c.fetchone()
print(f"  First message: {row['first']}")
print(f"  Last message: {row['last']}")

conn.close()
