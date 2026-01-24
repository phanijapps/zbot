# Entity Extraction from Transcript

When a user provides a transcript file path (via TRANSCRIPT_FILE annotation):

## 1. USE FILE TOOLS FIRST

Before analyzing the transcript content, use the appropriate file tools to explore it:

- **Use `read`** to view the full transcript: `read("/path/to/transcript_segments.txt")`
- **Use `grep`** to find specific patterns: `grep("TODO|FIXME", "/path/to/transcript_segments.txt")`
- **Use `rg`** (ripgrep) for advanced search: `rg("\\b\\d{1,2}:\\d{2}\\b", "/path/to/transcript_segments.txt")` (find timestamps)
- **Use `glob`** to find related files: `glob("*transcript*", "/vault/agents_data/agent-id/media/*")`

Example patterns to search for:
- Action items: `grep("action|todo|task|follow", transcript_file, case_insensitive=true)`
- Decisions: `grep("decided|agreed|concluded", transcript_file, case_insensitive=true)`
- Questions: `grep("\\?", transcript_file)`
- Names: `grep("[A-Z][a-z]+ [A-Z][a-z]+", transcript_file)` (capitalized words as potential names)

## 2. EXTRACT ENTITIES to knowledge graph

After scanning the transcript, extract and add these entity types:

### People
- Names mentioned (speakers and others)
- Roles or titles

### Organizations
- Company names
- Team names
- Project names

### Topics/Concepts
- Key themes discussed
- Technical concepts
- Domain terms

### Dates/Events
- Meeting dates
- Deadlines mentioned
- Event names

### Action Items
- Tasks assigned
- Follow-ups needed
- Decisions made

Use the knowledge graph tools to create entities and relationships between them.

## 3. PROVIDE SUMMARY

After extracting entities, provide a concise summary including:

- **What was discussed**: Main topics and themes
- **Key decisions made**: Any conclusions or agreements
- **Action items identified**: Tasks, follow-ups, next steps
- **Participants**: Who was involved (if evident from speakers)

Keep the summary structured and easy to scan.

## 4. SUGGEST ACTIONS

Based on the transcript analysis, suggest:

- **Follow-up questions**: What additional information would be useful?
- **Tasks to complete**: Actionable next steps based on the discussion
- **Related information**: What other context might be relevant?
- **Knowledge gaps**: What information is missing that would be valuable?

Format your response clearly with sections:

```
🎯 **Transcript Analysis**

**Summary:**
[Your summary here]

**Entities Extracted:**
- [Entity 1] → Knowledge Graph
- [Entity 2] → Knowledge Graph
...

**Action Items:**
1. [First action item]
2. [Second action item]
...

**Suggestions:**
- [Suggested follow-up]
- [Suggested task]
```

## Important Notes

- The transcript file path is provided as an **absolute path** - use it directly with file tools
- The plain text version (`_segments.txt`) is optimized for grep/rg searches
- Always use file tools first to understand the transcript content before analysis
- Focus on extracting **actionable** information that adds value to the knowledge graph
