// ============================================================================
// SKILLS FEATURE - Types
// TypeScript types for Agent Skills specification (https://agentskills.io/specification)
// ============================================================================

/** Skill following Agent Skills specification */
export interface Skill {
  /** Unique identifier for the skill */
  id: string;
  /** Skill name (lowercase, numbers, hyphens only) */
  name: string;
  /** Description of what the skill does and when to use it */
  description: string;
  /** Category for organization */
  category: SkillCategory;
  /** License information */
  license?: string;
  /** Compatibility information */
  compatibility?: string;
  /** Additional metadata */
  metadata?: SkillMetadata;
  /** The markdown content of the skill (instructions) */
  content: string;
  /** When the skill was created */
  createdAt: string;
  /** Directory path where skill is stored */
  path?: string;
}

/** Skill categories for organization */
export type SkillCategory =
  | 'Code Analysis'
  | 'Data Processing'
  | 'Web Scraping'
  | 'File Operations'
  | 'API Integration'
  | 'Database'
  | 'Machine Learning'
  | 'Natural Language'
  | 'System'
  | 'Utilities';

/** Additional metadata for skills */
export interface SkillMetadata {
  author?: string;
  version?: string;
  tags?: string[];
  [key: string]: any;
}

/** Skill file structure on disk */
export interface SkillFile {
  /** The SKILL.md content with YAML frontmatter */
  content: string;
  /** Optional script files */
  scripts?: Record<string, string>;
  /** Optional reference files */
  references?: Record<string, string>;
  /** Optional asset files */
  assets?: Record<string, string>;
}

/** Frontmatter structure for SKILL.md */
export interface SkillFrontmatter {
  name: string;
  description: string;
  license?: string;
  compatibility?: string;
  metadata?: SkillMetadata;
}

/** Preset skill template */
export interface PresetSkill {
  name: string;
  description: string;
  category: SkillCategory;
  license?: string;
  content: string;
}

/** All skill categories */
export const SKILL_CATEGORIES: SkillCategory[] = [
  'Code Analysis',
  'Data Processing',
  'Web Scraping',
  'File Operations',
  'API Integration',
  'Database',
  'Machine Learning',
  'Natural Language',
  'System',
  'Utilities',
];

/** Generate SKILL.md content from skill data */
export function generateSkillMarkdown(skill: Omit<Skill, 'id' | 'createdAt'>): string {
  let markdown = '---\n';
  markdown += `name: ${skill.name}\n`;
  markdown += `description: ${skill.description}\n`;

  if (skill.license) {
    markdown += `license: ${skill.license}\n`;
  }

  if (skill.compatibility) {
    markdown += `compatibility: ${skill.compatibility}\n`;
  }

  if (skill.metadata && Object.keys(skill.metadata).length > 0) {
    markdown += `metadata:\n`;
    Object.entries(skill.metadata).forEach(([key, value]) => {
      if (key === 'tags' && Array.isArray(value)) {
        markdown += `  ${key}: [${value.map((v: string) => `"${v}"`).join(', ')}]\n`;
      } else if (typeof value === 'string') {
        markdown += `  ${key}: "${value}"\n`;
      } else {
        markdown += `  ${key}: ${value}\n`;
      }
    });
  }

  markdown += '---\n\n';
  markdown += skill.content || '# Instructions\n\nAdd your skill instructions here.';

  return markdown;
}

/** Parse SKILL.md content to extract frontmatter and content */
export function parseSkillMarkdown(content: string): { frontmatter: SkillFrontmatter; body: string } {
  const frontmatterRegex = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/;
  const match = content.match(frontmatterRegex);

  if (!match) {
    return {
      frontmatter: { name: '', description: '' },
      body: content,
    };
  }

  const yamlContent = match[1];
  const body = match[2];

  // Simple YAML parser for our specific format
  const frontmatter: SkillFrontmatter = {
    name: extractYamlField(yamlContent, 'name') || '',
    description: extractYamlField(yamlContent, 'description') || '',
  };

  const license = extractYamlField(yamlContent, 'license');
  if (license) frontmatter.license = license;

  const compatibility = extractYamlField(yamlContent, 'compatibility');
  if (compatibility) frontmatter.compatibility = compatibility;

  return { frontmatter, body };
}

/** Extract a field value from YAML content */
function extractYamlField(yaml: string, fieldName: string): string | undefined {
  const regex = new RegExp(`^${fieldName}:\\s*(.+)$`, 'm');
  const match = yaml.match(regex);
  if (match) {
    const value = match[1].trim();
    // Remove quotes if present
    if ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))) {
      return value.slice(1, -1);
    }
    return value;
  }
  return undefined;
}

/** Preset skill templates */
export const PRESET_SKILLS: PresetSkill[] = [
  {
    name: 'python-code-executor',
    description: 'Execute Python code in a sandboxed environment for data analysis, computation, and automation tasks. Use when the user needs to run Python code, perform calculations, or process data.',
    category: 'Code Analysis',
    license: 'Apache-2.0',
    content: `# Python Code Executor

Executes Python code in a sandboxed environment with the following capabilities:

## Available Libraries
- numpy, pandas for data manipulation
- matplotlib, plotly for visualization
- requests for HTTP requests
- json, yaml for data parsing

## Usage
1. Receive Python code from user input
2. Execute in isolated environment with timeout
3. Return output, errors, or execution results

## Safety
- Execution timeout: 30 seconds
- Memory limit: 512MB
- No file system access outside sandbox
- No network access unless explicitly allowed

## Example
\`\`\`python
import pandas as pd
df = pd.read_csv('data.csv')
print(df.describe())
\`\`\``,
  },
  {
    name: 'web-scraper',
    description: 'Scrape content from websites with rate limiting and content extraction. Use when the user needs data from websites, web scraping, or extracting structured data from HTML.',
    category: 'Web Scraping',
    license: 'MIT',
    content: `# Web Scraper

Scrapes website content with the following features:

## Capabilities
- Extract text content from HTML
- Follow links within domain
- Rate limiting (1 request per second)
- Respect robots.txt
- Handle JavaScript rendering

## Usage
\`\`\`typescript
await scrapeWebsite({
  url: 'https://example.com',
  selector: '.content',
  followLinks: false,
  maxDepth: 1
});
\`\`\`

## Output Format
Returns structured data with:
- url: Source URL
- title: Page title
- content: Extracted text
- links: Found links (if followLinks: true)`,
  },
  {
    name: 'file-operations',
    description: 'Perform safe file operations including read, write, search, and transform files. Use when the user needs to work with files, search code, or batch process documents.',
    category: 'File Operations',
    license: 'Apache-2.0',
    content: `# File Operations

Safe file system operations for:

## Supported Operations
- **Read**: Read file contents with encoding detection
- **Write**: Write content with atomic operations
- **Search**: Search text/regex across files
- **List**: Directory listings with filtering
- **Move/Rename**: Atomic move operations

## Safety
- All operations within allowed directories
- Size limits for read operations
- Backup before modifications
- Confirmation for destructive operations

## Example
\`\`\`typescript
// Search for pattern
const results = await searchFiles({
  pattern: 'TODO:',
  directory: './src',
  filePattern: '*.ts'
});
\`\`\``,
  },
  {
    name: 'database-query',
    description: 'Execute safe parameterized database queries with SQL injection protection. Use when the user needs to query databases, analyze data, or perform database operations.',
    category: 'Database',
    license: 'Apache-2.0',
    content: `# Database Query

Safe database query execution:

## Supported Databases
- PostgreSQL
- MySQL
- SQLite
- MSSQL (limited support)

## Features
- Parameterized queries (SQL injection protection)
- Connection pooling
- Query result formatting
- Transaction support
- Query timeout (default: 30s)

## Usage
\`\`\`typescript
const result = await executeQuery({
  database: 'mydb',
  query: 'SELECT * FROM users WHERE id = ?',
  params: [userId]
});
\`\`\`

## Safety
- Read-only mode by default
- Row limit for SELECT queries
- No DDL without explicit permission`,
  },
  {
    name: 'json-schema-validator',
    description: 'Validate JSON data against schemas with detailed error reporting. Use when the user needs to validate data, check API responses, or ensure data integrity.',
    category: 'Data Processing',
    license: 'MIT',
    content: `# JSON Schema Validator

Validate JSON data against JSON Schema drafts:

## Supported Drafts
- Draft 7
- Draft 2019-09
- Draft 2020-12
- OpenAPI Schema 3.x

## Features
- Full JSON Schema validation
- Detailed error messages with paths
- Custom schema keywords
- Schema composition (allOf, anyOf, oneOf)
- Format validation (email, uri, date-time, etc.)

## Usage
\`\`\`typescript
const result = await validateJSON({
  data: userInput,
  schema: userSchema
});
// Returns: { valid: true/false, errors: [...] }
\`\`\``,
  },
];
