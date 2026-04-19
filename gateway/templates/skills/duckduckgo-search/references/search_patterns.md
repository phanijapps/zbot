# Search Patterns & Advanced Usage

Reference for query syntax, region codes, filters, and advanced patterns.

## Query Operators

DuckDuckGo supports these operators in search queries:

| Operator | Example | Description |
|----------|---------|-------------|
| `"..."` | `"agentic AI"` | Exact phrase match |
| `site:` | `site:reddit.com ADHD tips` | Search within a specific domain |
| `filetype:` | `machine learning filetype:pdf` | Find specific file types (pdf, doc, xls, ppt) |
| `-` | `python -snake` | Exclude a term |
| `intitle:` | `intitle:transformer architecture` | Term must appear in page title |
| `inurl:` | `inurl:api documentation` | Term must appear in URL |
| `OR` | `cats OR dogs` | Match either term |

### Combining Operators

```
"reinforcement learning" site:arxiv.org filetype:pdf
ADHD study techniques site:reddit.com -medication
intitle:tutorial "XGBoost" site:medium.com
```

## Region Codes

Control which region's results you see. Format: `{language}-{country}`

| Code | Region |
|------|--------|
| `us-en` | United States (English) |
| `uk-en` | United Kingdom (English) |
| `ca-en` | Canada (English) |
| `au-en` | Australia (English) |
| `in-en` | India (English) |
| `de-de` | Germany (German) |
| `fr-fr` | France (French) |
| `es-es` | Spain (Spanish) |
| `jp-jp` | Japan (Japanese) |
| `cn-zh` | China (Chinese) |
| `kr-ko` | South Korea (Korean) |
| `br-pt` | Brazil (Portuguese) |
| `ru-ru` | Russia (Russian) |
| `wt-wt` | No region (worldwide) |

Use `wt-wt` for the broadest results with no regional bias.

## Time Limits

Filter results by recency:

| Value | Text/Images | News |
|-------|-------------|------|
| `d` | Past day | Past day |
| `w` | Past week | Past week |
| `m` | Past month | Past month |
| `y` | Past year | N/A |

## Image Search Filters

### Size
`Small`, `Medium`, `Large`, `Wallpaper`

### Color
`Red`, `Orange`, `Yellow`, `Green`, `Blue`, `Purple`, `Pink`, `Brown`, `Black`, `Gray`, `Teal`, `White`, `Monochrome`

### Type
`photo`, `clipart`, `gif`, `transparent`, `line`

### Layout
`Square`, `Tall`, `Wide`

### License
`any`, `Public`, `Share`, `ShareCommercially`, `Modify`, `ModifyCommercially`

## Video Search Filters

### Resolution
`high`, `standard`

### Duration
`short` (< 5 min), `medium` (5-20 min), `long` (> 20 min)

## Rate Limit Avoidance

DuckDuckGo will rate-limit aggressive usage. Best practices:

1. **Space requests**: 1-2 seconds between consecutive searches
2. **Batch wisely**: Don't run more than 20 searches per minute
3. **Cache results**: If you'll re-query the same thing, save the results
4. **Use proxies for heavy usage**: Pass `proxy=` to the DDGS constructor
5. **Reduce max_results**: 10 results is usually enough; don't ask for 50 unless needed

### Retry Pattern

```python
import time
from ddgs import DDGS

def search_with_retry(query, max_retries=3, **kwargs):
    """Search with exponential backoff on rate limit."""
    for attempt in range(max_retries):
        try:
            return DDGS().text(query, **kwargs)
        except Exception as e:
            if "ratelimit" in str(e).lower() or "429" in str(e):
                wait = 2 ** attempt * 15  # 15s, 30s, 60s
                print(f"Rate limited, waiting {wait}s...")
                time.sleep(wait)
            else:
                raise
    return []
```

## Web Reader Patterns

### Reading Article Series

```python
from scripts.web_reader import read_url

# Search for a multi-part series
results = DDGS().text("site:example.com tutorial part", max_results=20)

# Read each part
for r in results:
    content, err = read_url(r["href"], include_metadata=True)
    if content:
        with open(f"article_{i}.md", "w") as f:
            f.write(content)
```

### Reddit Research Workflow

```python
from scripts.web_reader import read_url
from scripts.ddg_search import search_text

# 1. Search Reddit for relevant threads
results = search_text(
    "site:reddit.com ADHD productivity tools 2025",
    max_results=10,
    timelimit="m"
)

# 2. Read each thread with comments
for r in results:
    url = r.get("href", "")
    if "/comments/" in url:
        content, err = read_url(url, max_comments=20)
        if content:
            print(content[:500])
            print("---")
```

### Comparing Multiple Sources

```python
# Search for a topic
results = search_text("transformer vs mamba architecture comparison", max_results=5)

# Read top 3 articles
articles = []
for r in results[:3]:
    content, err = read_url(r["href"], include_metadata=True, favor="precision")
    if content:
        articles.append({
            "url": r["href"],
            "title": r["title"],
            "content": content
        })

# Now the agent has 3 articles to synthesize
```

### Trafilatura Configuration Tips

For different content types, adjust the extraction strategy:

```python
import trafilatura

downloaded = trafilatura.fetch_url(url)

# News articles — favor precision (less noise)
content = trafilatura.extract(downloaded, output_format="markdown",
                               favor_precision=True, include_formatting=True)

# Long-form essays/docs — favor recall (don't miss sections)
content = trafilatura.extract(downloaded, output_format="markdown",
                               favor_recall=True, include_formatting=True,
                               include_tables=True, include_links=True)

# Forums/discussions — include comments
content = trafilatura.extract(downloaded, output_format="markdown",
                               include_comments=True, include_formatting=True)

# Fast mode (skip fallback algorithms)
content = trafilatura.extract(downloaded, output_format="markdown",
                               no_fallback=True, include_formatting=True)
```

### Handling Problematic URLs

Some sites block automated access or have heavy JavaScript rendering.
Strategies:

```python
# 1. Try with different User-Agent
import requests
resp = requests.get(url, headers={
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
})

# 2. Try cached/archived versions
archive_url = f"https://web.archive.org/web/2/{url}"
google_cache = f"https://webcache.googleusercontent.com/search?q=cache:{url}"

# 3. For Reddit, always prefer JSON API over HTML scraping
# (handled automatically by web_reader.py)

# 4. For paywalled sites, try the text-only version if available
# Many news sites have /amp/ or /text/ versions
```

## Common Workflows

### Research a Topic End-to-End

```bash
# 1. Broad search to find key sources
python scripts/ddg_search.py text "agentic AI frameworks 2025" --max-results 15 --json --output sources.json

# 2. Read the top articles
python scripts/web_reader.py "https://article-url-1.com" --metadata --output article1.md
python scripts/web_reader.py "https://article-url-2.com" --metadata --output article2.md

# 3. Check Reddit for community perspectives
python scripts/ddg_search.py text "site:reddit.com agentic AI" --timelimit m --max-results 5
python scripts/web_reader.py "https://reddit.com/r/.../comments/..." --comments 25 --output reddit.md
```

### News Monitoring

```bash
# Get today's news on a topic
python scripts/ddg_search.py news "AI regulation" --timelimit d --max-results 20 --json --output news.json

# Read specific articles for details
python scripts/web_reader.py "https://news-article-url.com" --metadata --favor precision
```

### Image Research

```bash
# Find architecture diagrams
python scripts/ddg_search.py images "transformer attention mechanism diagram" \
    --type photo --size Large --max-results 10 --json --output diagrams.json
```
