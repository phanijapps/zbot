# Memory System Wishlist

## Overview

The Agent Channel architecture is designed to support three types of memory for AI agents: **Semantic**, **Episodic**, and **Procedural** memory.

---

## Memory Type Mapping

### 1. Semantic Memory (Facts & Concepts)

**Current Implementation:**
```sql
-- In schema_v2.rs:
kg_entities (id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at)
kg_relationships (source_entity_id, target_entity_id, relationship_type, properties)
```

**How it works:**
- As conversations happen, entities (people, concepts, topics) are extracted
- Stored in `kg_entities` with properties
- Relationships between entities stored in `kg_relationships`
- Example: From a conversation about refactoring:
  - Entity: "Rust", type: "ProgrammingLanguage"
  - Entity: "Agent Channel", type: "ArchitecturePattern"
  - Relationship: Agent Channel *implemented_in* Rust

**Future Extension:**
```rust
// application/knowledge-graph/
pub struct SemanticMemory {
    entities: HashMap<String, Entity>,
    relationships: Vec<Relationship>,
    embeddings: Option<VectorStore>, // For semantic search
}

impl SemanticMemory {
    // Extract entities from conversation
    pub async fn extract_from_conversation(&self, session_id: &str) -> Result<Vec<Entity>>;

    // Semantic search: "What do we know about X?"
    pub async fn query_semantic(&self, query: &str, agent_id: &str) -> Result<Vec<Fact>>;
}
```

---

### 2. Episodic Memory (Personal Experiences)

**Current Implementation:**
```rust
// DailySession already has:
pub struct DailySession {
    pub summary: Option<String>,           // End-of-day summary
    pub previous_session_ids: Option<Vec<String>>, // Links to past episodes
    pub message_count: i64,
    pub session_date: String,              // When it happened
}
```

**How it works:**
- Each day is an "episode"
- Summaries are compressed episodic memories
- `previous_session_ids` creates temporal links
- Expandable history UI shows episodic timeline

**Future Extension:**
```rust
// Enhanced episodic memory with retrieval
pub struct EpisodicMemory {
    sessions: Vec<DailySession>,
    embeddings: Option<VectorStore>,      // Embed summaries for retrieval
}

impl EpisodicMemory {
    // When asked a question, search relevant episodes
    pub async fn recall_relevant_episodes(
        &self,
        query: &str,
        agent_id: &str,
        k: usize
    ) -> Result<Vec<DailySession>>;

    // Build episodic timeline
    pub async fn build_timeline(&self, agent_id: &str) -> Result<EpisodeTimeline>;
}
```

---

### 3. Procedural Memory (Skills & Know-How)

**Current Implementation:**
```bash
agents_data/{agent-id}/
├── documents/           # ← Store procedural knowledge here
├── skills/              # ← Agent already has skills system
└── knowledge_graph/     # ← Can store "how-to" patterns
```

**How it works:**
- Skills system exists for explicit procedures
- `documents/` can store SOPs, guides, patterns
- Can learn procedural patterns from repeated successful interactions

**Future Extension:**
```rust
pub struct ProceduralMemory {
    skills: Vec<SkillPattern>,
    learned_workflows: Vec<Workflow>,
    success_patterns: Vec<Pattern>,
}

impl ProceduralMemory {
    // Learn from successful interactions
    pub async fn learn_from_session(&self, session: &DailySession) -> Result<()>;

    // Recall "how did we solve X before?"
    pub async fn recall_procedure(&self, problem: &str) -> Result<Vec<Workflow>>;
}
```

---

## Unified Memory System Architecture

The three memory types work together:

```rust
// Future: application/memory-system/
pub struct UnifiedMemory {
    semantic: Arc<SemanticMemory>,
    episodic: Arc<EpisodicMemory>,
    procedural: Arc<ProceduralMemory>,
}

impl UnifiedMemory {
    /// Answer: "What do we know about X?"
    pub async fn recall_semantic(&self, query: &str, agent_id: &str) -> Result<Vec<Fact>>;

    /// Answer: "What happened when we discussed X?"
    pub async fn recall_episodic(&self, query: &str, agent_id: &str) -> Result<Vec<Episode>>;

    /// Answer: "How do we usually solve X?"
    pub async fn recall_procedural(&self, task: &str, agent_id: &str) -> Result<Vec<Workflow>>;

    /// Combined: "Tell me everything about refactoring"
    pub async fn recall_all(&self, topic: &str, agent_id: &str) -> Result<MemoryGraph> {
        let semantic = self.recall_semantic(topic, agent_id).await?;
        let episodic = self.recall_episodic(topic, agent_id).await?;
        let procedural = self.recall_procedural(topic, agent_id).await?;

        // Build knowledge graph combining all three
        Ok(MemoryGraph::combine(semantic, episodic, procedural))
    }
}
```

---

## Integration with Agent Execution

The system prompt would include retrieved memories:

```rust
// In agents_runtime.rs (future enhancement)
pub async fn build_context_with_memory(
    &self,
    agent_id: &str,
    current_session_id: &str,
    user_message: &str
) -> Result<String> {
    // 1. Get today's session
    let session = self.get_or_create_today_session(agent_id).await?;

    // 2. Retrieve relevant memories
    let semantic_facts = memory.recall_semantic(user_message, agent_id).await?;
    let relevant_episodes = memory.recall_episodic(user_message, agent_id).await?;
    let procedures = memory.recall_procedural(user_message, agent_id).await?;

    // 3. Build enhanced context
    format!(
        "=== SEMANTIC MEMORY ===\n{:#?}\n\n\
         === EPISODIC MEMORY ===\n{:#?}\n\n\
         === PROCEDURAL MEMORY ===\n{:#?}\n\n\
         === TODAY'S CONTEXT ===\n\
         Previous summary: {}",
        semantic_facts, relevant_episodes, procedures,
        session.summary.as_deref()
    )
}
```

---

## Summary Table

| Memory Type | Current Implementation | Future Extension |
|-------------|------------------------|-------------------|
| **Semantic** | `kg_entities`/`kg_relationships` tables | Entity extraction + embeddings |
| **Episodic** | Daily sessions with summaries | Vector search over summaries |
| **Procedural** | Skills system + documents/ folder | Pattern learning from interactions |

---

## Implementation Phases

### Phase 1: Foundation (Current)
- ✅ Database schema with `kg_entities` and `kg_relationships`
- ✅ Daily sessions with summaries
- ✅ File structure for `documents/`, `knowledge_graph/`

### Phase 2: Basic Semantic Memory
- Entity extraction from conversations
- Basic relationship storage
- Simple query interface

### Phase 3: Enhanced Episodic Memory
- End-of-day summary generation (LLM-based)
- Vector embeddings for summaries
- Semantic search over episodes

### Phase 4: Procedural Learning
- Pattern extraction from successful interactions
- Workflow templates
- Skill recommendation system

### Phase 5: Unified Memory System
- Combined retrieval across all memory types
- Memory consolidation (moving from episodic to semantic)
- Forgetting mechanisms (old, unused memories)
