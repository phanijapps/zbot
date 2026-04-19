---
name: duckduckgo-search
description: >
  Search the web, news, and images using DuckDuckGo, and convert any webpage into clean
  readable markdown. Use this skill whenever the agent needs to search the internet, look up
  current information, find news articles, search for images, or read/extract content from
  a URL. Trigger when the user says "search for", "look up", "find", "what's the latest on",
  "read this article", "get the content from", "summarize this page", or any request requiring
  web information the agent doesn't already know. Also trigger for research tasks, fact-checking,
  gathering context for decisions, reading Reddit posts/threads, or extracting article text
  from any URL. This is the agent's primary tool for accessing web information.
  Do NOT use for Polymarket-specific queries (use polymarket-trader) or for
  financial market data (use yfinance/ML skills).
license: Apache-2.0
metadata:
  author: phani
  version: "1.0"
  tags: "search, web, news, images, duckduckgo, readability, markdown, articles, reddit"
---

# DuckDuckGo Search & Web Reader

A skill for searching the web and converting webpages into clean, readable markdown.
Provides four search modes (text, news, images, videos) and a webpage reader that strips
boilerplate and extracts the main content.

## Installation

```bash
# Primary library (metasearch — aggregates DDG + Google + Bing + Brave)
pip install ddgs

# Fallback library (DDG-only, if ddgs fails)
pip install duckduckgo-search

# Webpage reader (article extraction + markdown conversion)
pip install trafilatura

# Optional: for enhanced HTML handling
pip install html2text
```

The skill tries `ddgs` first (newer, metasearch across multiple engines). If unavailable
or rate-limited, it falls back to `duckduckgo-search` (DDG-only). Both share the same
`DDGS` class interface but import from different packages.

## Quick Start

Run `scripts/ddg_search.py` for all search operations:

```bash
# Web search
python scripts/ddg_search.py text "latest AI agent frameworks" --max-results 10

# News search
python scripts/ddg_search.py news "federal reserve interest rates" --timelimit d

# Image search
python scripts/ddg_search.py images "transformer architecture diagram" --max-results 5

# Video search
python scripts/ddg_search.py videos "andrej karpathy lecture" --max-results 5
```

Run `scripts/web_reader.py` for reading any webpage:

```bash
# Read an article and get clean markdown
python scripts/web_reader.py "https://example.com/some-article"

# Read with metadata (title, author, date)
python scripts/web_reader.py "https://example.com/article" --metadata

# Read a Reddit thread
python scripts/web_reader.py "https://www.reddit.com/r/MachineLearning/comments/abc123/title"

# Save to file
python scripts/web_reader.py "https://example.com/article" --output article.md
```

## Search Modes

### 1. Text Search (Web)

The primary search mode. Returns titles, URLs, and snippets.

```python
from ddgs import DDGS  # or: from duckduckgo_search import DDGS

results = DDGS().text(
    "agentic AI patterns 2025",
    region="us-en",         # region code (see references/search_patterns.md)
    safesearch="moderate",  # on, moderate, off
    timelimit=None,         # d=day, w=week, m=month, y=year
    max_results=10,
)

for r in results:
    print(f"{r['title']}")
    print(f"  {r['href']}")
    print(f"  {r['body'][:150]}")
```

**Response format:**
```python
[
    {
        "title": "Page Title",
        "href": "https://example.com/page",
        "body": "Snippet text from the page..."
    },
    ...
]
```

**Tips for effective queries:**
- Keep queries specific: "XGBoost hyperparameter tuning" > "machine learning"
- Use `site:` operator: "site:reddit.com ADHD study tips"
- Use `filetype:` operator: "transformer paper filetype:pdf"
- Use quotes for exact phrases: '"agentic AI" framework comparison'
- Use timelimit="d" for breaking news, "w" for recent developments

### 2. News Search

Returns recent news articles with publication dates and sources.

```python
results = DDGS().news(
    "federal reserve rate decision",
    region="us-en",
    safesearch="moderate",
    timelimit="w",          # d=day, w=week, m=month
    max_results=10,
)
```

**Response format:**
```python
[
    {
        "date": "2025-12-15T10:30:00",
        "title": "Article Title",
        "body": "Article snippet...",
        "url": "https://news-source.com/article",
        "image": "https://...",       # thumbnail URL
        "source": "Reuters"
    },
    ...
]
```

### 3. Image Search

Returns image URLs with metadata.

```python
results = DDGS().images(
    "neural network architecture diagram",
    region="us-en",
    safesearch="moderate",
    timelimit=None,
    max_results=5,
    size=None,              # Small, Medium, Large, Wallpaper
    color=None,             # Red, Orange, Yellow, Green, Blue, Purple, Pink, Brown, Black, Gray, Teal, White, Monochrome
    type_image=None,        # photo, clipart, gif, transparent, line
    layout=None,            # Square, Tall, Wide
    license_image=None,     # any, Public, Share, ShareCommercially, Modify, ModifyCommercially
)
```

**Response format:**
```python
[
    {
        "title": "Image title",
        "image": "https://full-resolution-url.com/image.png",
        "thumbnail": "https://thumbnail-url.com/thumb.jpg",
        "url": "https://source-page.com/page",
        "height": 1080,
        "width": 1920,
        "source": "Bing"
    },
    ...
]
```

### 4. Video Search

Returns video results with durations and sources.

```python
results = DDGS().videos(
    "andrej karpathy transformers lecture",
    region="us-en",
    safesearch="moderate",
    timelimit=None,
    max_results=5,
    resolution=None,        # high, standard
    duration=None,          # short, medium, long
)
```

**Response format:**
```python
[
    {
        "content": "https://www.youtube.com/watch?v=...",
        "description": "Video description...",
        "duration": "1:23:45",
        "embed_html": "<iframe ...>",
        "embed_url": "https://...",
        "image_large": "https://...",
        "image_medium": "https://...",
        "image_small": "https://...",
        "image_token": "...",
        "publisher": "YouTube",
        "title": "Video Title",
        "uploader": "Channel Name"
    },
    ...
]
```

## Web Reader (URL → Markdown)

The web reader fetches a URL and converts it to clean, readable markdown using `trafilatura`.
This is the core "readability" feature — it strips navigation, ads, sidebars, and boilerplate,
leaving just the article content.

### Basic Usage

```python
import trafilatura

# Fetch and extract
downloaded = trafilatura.fetch_url("https://example.com/article")
content = trafilatura.extract(
    downloaded,
    output_format="markdown",
    include_links=True,
    include_formatting=True,
    include_tables=True,
    include_images=False,
)
print(content)
```

### With Metadata

```python
result = trafilatura.bare_extraction(
    downloaded,
    url="https://example.com/article",
    include_formatting=True,
    include_links=True,
    include_tables=True,
    with_metadata=True,
    as_dict=True,
)
# result = {
#     "title": "Article Title",
#     "author": "Author Name",
#     "date": "2025-12-15",
#     "text": "Clean article text...",
#     "comments": "...",
#     "categories": [...],
#     "tags": [...],
#     "source": "example.com",
# }
```

### Reading Reddit Posts

Reddit works best through its JSON API — cleaner data, includes comments with structure.

```python
import requests
import json

def read_reddit_thread(url):
    """Fetch a Reddit thread with comments via JSON API."""
    # Convert any Reddit URL to JSON endpoint
    clean_url = url.split("?")[0].rstrip("/")
    if not clean_url.endswith(".json"):
        clean_url += ".json"

    headers = {"User-Agent": "AgentZero/1.0"}
    resp = requests.get(clean_url, headers=headers, timeout=15)
    data = resp.json()

    # First element is the post, second is comments
    post_data = data[0]["data"]["children"][0]["data"]
    comments_data = data[1]["data"]["children"]

    post = {
        "title": post_data.get("title", ""),
        "author": post_data.get("author", "[deleted]"),
        "score": post_data.get("score", 0),
        "selftext": post_data.get("selftext", ""),
        "url": post_data.get("url", ""),
        "subreddit": post_data.get("subreddit", ""),
        "num_comments": post_data.get("num_comments", 0),
        "created_utc": post_data.get("created_utc", 0),
    }

    comments = []
    for c in comments_data:
        if c["kind"] != "t1":
            continue
        cd = c["data"]
        comments.append({
            "author": cd.get("author", "[deleted]"),
            "body": cd.get("body", ""),
            "score": cd.get("score", 0),
        })

    return post, comments
```

**For old.reddit.com URLs**, `trafilatura` works well as a fallback:

```python
# Convert to old Reddit for cleaner HTML
url = url.replace("www.reddit.com", "old.reddit.com")
downloaded = trafilatura.fetch_url(url)
content = trafilatura.extract(downloaded, output_format="markdown", include_formatting=True)
```

### Fallback: html2text

When trafilatura fails (rare, but happens on highly dynamic pages), fall back to `html2text`:

```python
import requests
import html2text

resp = requests.get(url, headers={"User-Agent": "AgentZero/1.0"}, timeout=15)
h = html2text.HTML2Text()
h.ignore_links = False
h.ignore_images = True
h.body_width = 0          # no line wrapping
h.skip_internal_links = True
markdown = h.handle(resp.text)
```

This preserves everything (including boilerplate), so it's noisier but more complete.

## Combining Search + Read

The most powerful workflow: search for relevant pages, then read the best ones.

```python
# 1. Search
results = DDGS().text("ADHD study techniques research 2025", max_results=5, timelimit="m")

# 2. Pick the most relevant results
best_urls = [r["href"] for r in results[:3]]

# 3. Read each article
for url in best_urls:
    downloaded = trafilatura.fetch_url(url)
    if downloaded:
        content = trafilatura.extract(
            downloaded,
            output_format="markdown",
            include_formatting=True,
            include_links=True,
            with_metadata=True,
        )
        if content:
            print(f"--- {url} ---")
            print(content[:500])  # preview
```

The `scripts/ddg_search.py` and `scripts/web_reader.py` scripts handle this workflow
with proper error handling, rate limiting, and output formatting.

## Error Handling & Rate Limits

DuckDuckGo may rate-limit or temporarily block requests. Handle this gracefully:

```python
from duckduckgo_search.exceptions import RatelimitException, TimeoutException

try:
    results = DDGS().text("query", max_results=10)
except RatelimitException:
    print("Rate limited — waiting 30s before retry")
    import time
    time.sleep(30)
    results = DDGS().text("query", max_results=10)
except TimeoutException:
    print("Timeout — try a simpler query or reduce max_results")
    results = []
```

**Best practices to avoid rate limits:**
- Don't make more than ~20 searches per minute
- Add 1-2 second delays between batch searches
- Use proxies for heavy usage (DDGS supports `proxy=` parameter)
- Cache results when running repeated searches

## Integration with Other Skills

- **ml-pipeline-builder**: Search for research papers, then read them to extract methodology details
- **polymarket-trader**: Search for news about prediction market events to inform trading decisions
- **General research**: Search → Read → Summarize → Act

## Dependencies

```
# Required (one of these)
ddgs>=9.0                  # preferred — metasearch
duckduckgo-search>=8.0     # fallback — DDG only

# Required for web reading
trafilatura>=2.0

# Optional
html2text>=2024.0          # fallback HTML→markdown
requests>=2.28             # for Reddit JSON API
```

Install all: `pip install ddgs duckduckgo-search trafilatura html2text requests`
