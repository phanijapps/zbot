# Knowledge Graph

## Overview

The Knowledge Graph is a **semantic memory system** for agents that stores entities (people, organizations, locations, concepts, tools, projects) and their relationships. It allows agents to remember and query information across conversations.

## When to Use Knowledge Graph

Enable the Knowledge Graph for agents that need to:

1. **Remember people and relationships** - CRM-like use cases where agents track contacts, their roles, and connections
2. **Maintain context about organizations** - Business intelligence, company structure tracking
3. **Build domain expertise** - Technical agents that learn concepts, tools, and how they relate
4. **Support long-term projects** - Project tracking with entities for tasks, milestones, dependencies
5. **Provide personalized assistance** - Agents that learn user preferences, habits, and connections

## When NOT to Use Knowledge Graph

Skip the Knowledge Graph for agents that:

1. **Process transient tasks** - One-off code reviews, simple Q&A, ephemeral conversations
2. **Work with stateless data** - Agents that don't need to remember context between sessions
3. **Have short conversations** - Single-turn interactions where memory isn't valuable
4. **Focus on execution** - Agents that primarily run tools without building knowledge

## How It Works

### Data Structure

```
Entity: {name, type, properties, mention_count, first_seen, last_seen}
  ├─ Types: Person, Organization, Location, Concept, Tool, Project
  └─ Properties: Custom key-value metadata

Relationship: {source, target, type, properties, mention_count}
  └─ Types: WorksFor, LocatedIn, RelatedTo, Created, Uses, PartOf, Mentions
```

### Storage

- **Database**: SQLite (`agent_channels.db`)
- **Tables**: `kg_entities`, `kg_relationships`
- **Agent-scoped**: Each agent has isolated knowledge
- **Location**: `{vault_path}/db/agent_channels.db`

### Access Methods

#### Automatic (Future - Not Yet Implemented)

Currently, the Knowledge Graph does **NOT** automatically extract entities from messages. Agents must explicitly use tools to add/query knowledge.

#### Manual Tools (Available Now)

Agents have 5 tools to interact with the Knowledge Graph:

| Tool | Description |
|------|-------------|
| `list_entities` | List all entities, optionally filter by type |
| `search_entities` | Search entities by name (partial match) |
| `get_entity_relationships` | Get relationships for a specific entity |
| `add_entity` | Add a new entity with properties |
| `add_relationship` | Add a relationship between two entities |

### Current Limitations

- ❌ No automatic entity extraction from messages
- ❌ No LLM-powered extraction (only rule-based)
- ❌ No frontend UI for visualization
- ❌ No relationship inference (relationships must be explicitly added)

## Example Agent: Relationship Manager

Here's an example agent configuration that uses the Knowledge Graph to track professional relationships:

```yaml
# ~/.config/agentzero/agents/relationship-manager/config.yaml

name: relationship-manager
displayName: Relationship Manager
description: Tracks contacts, companies, and professional relationships

providerId: openai
model: gpt-4o-mini
temperature: 0.7
maxTokens: 2000

systemPrompt: |
  You are a Relationship Manager assistant. You help track professional contacts,
  companies, and how people relate to each other.

  ## Your Capabilities

  You can remember and query information about:
  - **People**: Names, roles, email addresses, phone numbers
  - **Organizations**: Companies, departments, teams
  - **Relationships**: Who works for which company, who knows whom

  ## Using the Knowledge Graph

  When learning new information, proactively use the knowledge graph tools:

  ### Adding People
  When you learn about a new person, use add_entity:
  ```
  add_entity(name="John Smith", entity_type="person",
             properties={"role": "Engineer", "email": "john@example.com"})
  ```

  ### Adding Companies
  Use add_entity for organizations:
  ```
  add_entity(name="Acme Corp", entity_type="organization",
             properties={"industry": "Technology", "website": "acme.com"})
  ```

  ### Adding Relationships
  Connect entities with add_relationship:
  ```
  add_relationship(source="John Smith", target="Acme Corp",
                   relationship_type="works_for")
  ```

  ### Querying Information
  - "Who do I know at Acme Corp?" → search_entities then get_entity_relationships
  - "What do you know about John Smith?" → get_entity_relationships
  - "List all people I've met" → list_entities with entity_type="person"

  ## Best Practices

  1. **Always add entities first** - Before adding relationships, ensure both entities exist
  2. **Use consistent names** - "John Smith" vs "John" - pick one and stick to it
  3. **Include useful properties** - Email, phone, role, industry make entities more useful
  4. **Proactively add information** - Don't wait for users to ask, learn from context

  ## Example Conversation

  User: "I met Sarah Johnson today, she's a Product Manager at TechStart."

  You: "I've added Sarah Johnson to your contacts as a Product Manager at TechStart.
        I've also connected her to the TechStart organization."

  User: "Who do I know at TechStart?"

  You: "You have one contact at TechStart:
        - Sarah Johnson (Product Manager)"
```

## Example Agent: Codebase Expert

```yaml
# ~/.config/agentzero/agents/codebase-expert/config.yaml

name: codebase-expert
displayName: Codebase Expert
description: Learns and remembers codebase architecture, components, and dependencies

providerId: openai
model: gpt-4o-mini
temperature: 0.3
maxTokens: 4000

systemPrompt: |
  You are a Codebase Expert assistant. You learn and remember the architecture
  of codebases, including components, modules, dependencies, and how they relate.

  ## What You Track

  - **Tools**: Libraries, frameworks, languages (e.g., "React", "Rust", "PostgreSQL")
  - **Concepts**: Architecture patterns, technologies (e.g., "Microservices", "REST API")
  - **Projects**: Repositories, services, modules
  - **Relationships**: What uses what, what depends on what

  ## Knowledge Graph Usage

  ### Tracking Technologies
  ```
  add_entity(name="React", entity_type="tool",
             properties={"type": "framework", "language": "JavaScript"})
  ```

  ### Tracking Projects/Modules
  ```
  add_entity(name="User Service", entity_type="project",
             properties={"language": "Rust", "port": 8080})
  ```

  ### Tracking Dependencies
  ```
  add_relationship(source="User Service", target="PostgreSQL",
                   relationship_type="uses")
  add_relationship(source="User Service", target="React",
                   relationship_type="part_of")
  ```

  ### Querying Architecture
  - "What does User Service depend on?" → get_entity_relationships("User Service")
  - "What projects use PostgreSQL?" → search_entities then check relationships
  - "List all components in this codebase" → list_entities with entity_type="project"
```

## System Prompt Tips

When creating agents that use the Knowledge Graph:

### 1. Explain the Purpose

```markdown
You have access to a knowledge graph that helps you remember information
about [domain]. Use it to build long-term understanding.
```

### 2. Define Your Strategy

```markdown
## Learning Strategy
- When you learn new [entities], immediately add them with add_entity
- When you discover connections, use add_relationship
- Before answering questions about [domain], query your knowledge first
```

### 3. Give Examples

```markdown
## Example: Adding Information
User: "Acme Corp uses Kubernetes for orchestration"
You: [calls add_entity for Acme Corp and Kubernetes, then add_relationship]
```

### 4. Handle Missing Information

```markdown
If someone asks about something not in your knowledge graph, say:
"I don't have information about that in my knowledge graph yet.
Would you like me to add it?"
```

## Testing Knowledge Graph Tools

You can test the knowledge graph tools by creating a simple test agent:

```bash
# Create a test agent
mkdir -p ~/.config/agentzero/agents/kg-test
cat > ~/.config/agentzero/agents/kg-test/config.yaml << 'EOF'
name: kg-test
displayName: Knowledge Graph Test
description: Tests knowledge graph tools

providerId: openai
model: gpt-4o-mini
temperature: 0.7
maxTokens: 1000

systemPrompt: |
  You are a knowledge graph test assistant. Demonstrate the use of knowledge
  graph tools by:
  1. Adding entities when you learn about new things
  2. Adding relationships between entities
  3. Listing and searching entities when asked

  Be proactive about using the tools. When I tell you something,
  immediately add it to the knowledge graph.
EOF

# Test with prompts:
# "Add John Smith as a person who works at Google"
# "What entities do you know about?"
# "What relationships does Google have?"
```

## Future Enhancements

Planned improvements to the Knowledge Graph system:

1. **Automatic Extraction** - Extract entities/relationships from every message
2. **LLM-Powered Extraction** - Use LLM to identify entities more accurately
3. **Graph Visualization** - Frontend UI to view and browse the graph
4. **Semantic Search** - Vector embeddings for concept-based search
5. **Relationship Inference** - Automatically infer relationships from context
6. **Entity Linking** - Recognize when mentions refer to same entity

## Related Documentation

| File | Description |
|------|-------------|
| `application/knowledge-graph/` | Knowledge graph crate implementation |
| `src-tauri/src/commands/agent_channels.rs` | Database operations |
| `application/agent-tools/src/tools/knowledge_graph.rs` | Agent tools |
| `memory-bank/architecture.md` | Overall architecture |
