#!/usr/bin/env python3
"""
Web Reader — Fetch any URL and convert to clean, readable markdown.

Uses trafilatura for article extraction (strips boilerplate, nav, ads, footers).
Special handling for Reddit posts via JSON API.
Falls back to html2text for pages trafilatura can't handle.

Usage:
    # Read any article
    python web_reader.py "https://example.com/article"

    # Read with metadata (title, author, date)
    python web_reader.py "https://example.com/article" --metadata

    # Read a Reddit post/thread
    python web_reader.py "https://reddit.com/r/MachineLearning/comments/abc123/title"

    # Read Reddit with N top comments
    python web_reader.py "https://reddit.com/r/sub/comments/id/title" --comments 20

    # Save output to file
    python web_reader.py "https://example.com/article" --output article.md

    # Force html2text instead of trafilatura
    python web_reader.py "https://example.com/page" --raw

    # Just fetch HTML (no conversion)
    python web_reader.py "https://example.com/page" --html-only

    # Favor precision (less noise) or recall (more content)
    python web_reader.py "https://example.com/article" --favor precision
    python web_reader.py "https://example.com/article" --favor recall
"""

import argparse
import json
import sys
import re
from datetime import datetime, timezone
from urllib.parse import urlparse

USER_AGENT = USER_AGENT
DELETED_MARKER = DELETED_MARKER

# --- Lazy imports with availability tracking ---
_trafilatura = None
_html2text = None
_requests = None


def get_trafilatura():
    global _trafilatura
    if _trafilatura is None:
        try:
            import trafilatura
            _trafilatura = trafilatura
        except ImportError:
            pass
    return _trafilatura


def get_html2text():
    global _html2text
    if _html2text is None:
        try:
            import html2text
            _html2text = html2text
        except ImportError:
            pass
    return _html2text


def get_requests():
    global _requests
    if _requests is None:
        import requests
        _requests = requests
    return _requests


# --- Reddit handling ---

def is_reddit_url(url):
    """Check if URL is a Reddit link."""
    parsed = urlparse(url)
    return any(domain in parsed.netloc for domain in [
        "reddit.com", "old.reddit.com", "www.reddit.com",
        "np.reddit.com", "i.reddit.com", "m.reddit.com"
    ])


def is_reddit_thread(url):
    """Check if it's a specific Reddit thread (not a subreddit listing)."""
    return is_reddit_url(url) and "/comments/" in url


def fetch_reddit_thread(url, max_comments=15):
    """Fetch a Reddit thread with comments via JSON API."""
    requests = get_requests()

    # Normalize URL to JSON endpoint
    clean_url = url.split("?")[0].rstrip("/")
    # Remove any existing .json
    clean_url = re.sub(r'\.json$', '', clean_url)
    json_url = clean_url + ".json"

    headers = {"User-Agent": USER_AGENT}

    try:
        resp = requests.get(json_url, headers=headers, timeout=20, allow_redirects=True)
        resp.raise_for_status()
        data = resp.json()
    except Exception as e:
        return None, f"Reddit API error: {e}"

    if not isinstance(data, list) or len(data) < 2:
        return None, "Unexpected Reddit API response format"

    # --- Parse post ---
    post_data = data[0]["data"]["children"][0]["data"]

    post = {
        "subreddit": post_data.get("subreddit", ""),
        "title": post_data.get("title", ""),
        "author": post_data.get("author", DELETED_MARKER),
        "score": post_data.get("score", 0),
        "upvote_ratio": post_data.get("upvote_ratio", 0),
        "selftext": post_data.get("selftext", ""),
        "url": post_data.get("url", ""),
        "permalink": post_data.get("permalink", ""),
        "num_comments": post_data.get("num_comments", 0),
        "created_utc": post_data.get("created_utc", 0),
        "link_flair_text": post_data.get("link_flair_text", ""),
        "is_self": post_data.get("is_self", True),
    }

    # --- Parse comments ---
    comments = []
    _parse_comments_recursive(data[1]["data"]["children"], comments, depth=0, max_total=max_comments)

    return {"post": post, "comments": comments}, None


def _parse_comments_recursive(children, comments, depth=0, max_total=15):
    """Recursively parse Reddit comments with threading."""
    for child in children:
        if len(comments) >= max_total:
            break
        if child.get("kind") != "t1":
            continue

        cd = child["data"]
        if cd.get("body") in (DELETED_MARKER, "[removed]", None):
            continue

        comments.append({
            "author": cd.get("author", DELETED_MARKER),
            "body": cd.get("body", ""),
            "score": cd.get("score", 0),
            "depth": depth,
            "created_utc": cd.get("created_utc", 0),
        })

        # Recurse into replies
        replies = cd.get("replies")
        if isinstance(replies, dict) and "data" in replies:
            _parse_comments_recursive(
                replies["data"]["children"], comments,
                depth=depth + 1, max_total=max_total
            )


def format_reddit_markdown(thread_data):
    """Convert Reddit thread data to clean markdown."""
    post = thread_data["post"]
    comments = thread_data["comments"]

    lines = []

    # Post header
    flair = f" [{post['link_flair_text']}]" if post.get("link_flair_text") else ""
    lines.append(f"# {post['title']}{flair}")
    lines.append("")

    # Metadata
    ts = datetime.fromtimestamp(post["created_utc"], tz=timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    lines.append(f"**r/{post['subreddit']}** · u/{post['author']} · {ts}")
    lines.append(f"Score: {post['score']} ({post['upvote_ratio']*100:.0f}% upvoted) · {post['num_comments']} comments")
    lines.append("")

    # Post body
    if post["selftext"]:
        lines.append(post["selftext"])
        lines.append("")
    elif not post["is_self"] and post["url"]:
        lines.append(f"**Link:** {post['url']}")
        lines.append("")

    # Comments
    if comments:
        lines.append("---")
        lines.append("")
        lines.append(f"## Comments ({len(comments)} shown)")
        lines.append("")

        for c in comments:
            indent = "  " * c["depth"]
            score_str = f" [{c['score']} pts]"
            lines.append(f"{indent}**u/{c['author']}**{score_str}:")
            # Indent comment body
            for body_line in c["body"].split("\n"):
                lines.append(f"{indent}> {body_line}")
            lines.append("")

    return "\n".join(lines)


# --- Generic web page reading ---

def read_with_trafilatura(url, include_metadata=False, favor=None):
    """Read a webpage using trafilatura (primary method)."""
    traf = get_trafilatura()
    if traf is None:
        return None, "trafilatura not installed"

    try:
        downloaded = traf.fetch_url(url)
        if not downloaded:
            return None, "Failed to fetch URL"

        kwargs = {
            "output_format": "markdown",
            "include_links": True,
            "include_formatting": True,
            "include_tables": True,
            "include_images": False,
            "include_comments": False,
        }

        if favor == "precision":
            kwargs["favor_precision"] = True
        elif favor == "recall":
            kwargs["favor_recall"] = True

        if include_metadata:
            result = traf.bare_extraction(
                downloaded,
                url=url,
                include_formatting=True,
                include_links=True,
                include_tables=True,
                with_metadata=True,
                as_dict=True,
            )
            if result:
                # Build markdown with metadata header
                lines = []
                if result.get("title"):
                    lines.append(f"# {result['title']}")
                    lines.append("")

                meta_parts = []
                if result.get("author"):
                    meta_parts.append(f"**Author:** {result['author']}")
                if result.get("date"):
                    meta_parts.append(f"**Date:** {result['date']}")
                if result.get("source"):
                    meta_parts.append(f"**Source:** {result['source']}")
                if result.get("categories"):
                    cats = result["categories"]
                    if isinstance(cats, list):
                        meta_parts.append(f"**Categories:** {', '.join(cats)}")
                if result.get("tags"):
                    tags = result["tags"]
                    if isinstance(tags, list):
                        meta_parts.append(f"**Tags:** {', '.join(tags)}")

                if meta_parts:
                    lines.append(" · ".join(meta_parts))
                    lines.append("")
                    lines.append("---")
                    lines.append("")

                # Get the main text (extract with markdown format)
                text = traf.extract(downloaded, **kwargs)
                if text:
                    lines.append(text)

                return "\n".join(lines), None
            else:
                return None, "trafilatura extraction returned empty"
        else:
            content = traf.extract(downloaded, **kwargs)
            if content:
                return content, None
            else:
                return None, "trafilatura extraction returned empty"

    except Exception as e:
        return None, f"trafilatura error: {e}"


def read_with_html2text(url):
    """Read a webpage using html2text (fallback method)."""
    h2t = get_html2text()
    requests = get_requests()

    if h2t is None:
        return None, "html2text not installed"

    try:
        headers = {"User-Agent": USER_AGENT}
        resp = requests.get(url, headers=headers, timeout=20, allow_redirects=True)
        resp.raise_for_status()

        h = h2t.HTML2Text()
        h.ignore_links = False
        h.ignore_images = True
        h.body_width = 0              # no line wrapping
        h.skip_internal_links = True
        h.ignore_emphasis = False
        h.protect_links = True
        h.wrap_links = False
        h.wrap_list_items = True
        h.single_line_break = False

        content = h.handle(resp.text)
        return content, None

    except Exception as e:
        return None, f"html2text error: {e}"


def fetch_html_only(url):
    """Just fetch raw HTML."""
    requests = get_requests()
    try:
        headers = {"User-Agent": USER_AGENT}
        resp = requests.get(url, headers=headers, timeout=20, allow_redirects=True)
        resp.raise_for_status()
        return resp.text, None
    except Exception as e:
        return None, f"Fetch error: {e}"


def read_url(url, include_metadata=False, favor=None, use_raw=False, max_comments=15):
    """
    Main entry point: read any URL and return clean markdown.
    Handles Reddit specially, uses trafilatura by default, falls back to html2text.
    """
    # Reddit thread → use JSON API
    if is_reddit_thread(url) and not use_raw:
        thread_data, err = fetch_reddit_thread(url, max_comments=max_comments)
        if thread_data:
            return format_reddit_markdown(thread_data), None
        # Fall through to trafilatura if Reddit JSON fails
        print(f"Reddit JSON API failed ({err}), trying trafilatura...", file=sys.stderr)

    # Force html2text
    if use_raw:
        return read_with_html2text(url)

    # Try trafilatura first
    content, err = read_with_trafilatura(url, include_metadata=include_metadata, favor=favor)
    if content:
        return content, None

    # Fallback to html2text
    print(f"trafilatura failed ({err}), falling back to html2text...", file=sys.stderr)
    content, err2 = read_with_html2text(url)
    if content:
        return content, None

    return None, f"All methods failed. trafilatura: {err}. html2text: {err2}"


def main():
    parser = argparse.ArgumentParser(
        description="Web Reader — Convert any URL to clean markdown",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python web_reader.py "https://example.com/article"
  python web_reader.py "https://example.com/article" --metadata
  python web_reader.py "https://reddit.com/r/sub/comments/id/title" --comments 20
  python web_reader.py "https://example.com/article" --output article.md
  python web_reader.py "https://example.com/page" --raw
  python web_reader.py "https://example.com/article" --favor precision
        """
    )

    parser.add_argument("url", type=str, help="URL to read")
    parser.add_argument("--metadata", action="store_true", help="Include metadata (title, author, date)")
    parser.add_argument("--output", "-o", type=str, help="Save output to file")
    parser.add_argument("--raw", action="store_true", help="Force html2text (preserves everything, noisier)")
    parser.add_argument("--html-only", action="store_true", help="Just fetch raw HTML, no conversion")
    parser.add_argument("--comments", type=int, default=15, help="Max Reddit comments to include (default: 15)")
    parser.add_argument("--favor", choices=["precision", "recall"],
                        help="precision = less noise, recall = more content")
    parser.add_argument("--json", action="store_true", help="Output as JSON (for programmatic use)")

    args = parser.parse_args()

    # Validate URL
    parsed = urlparse(args.url)
    if not parsed.scheme:
        args.url = "https://" + args.url
        parsed = urlparse(args.url)

    if not parsed.netloc:
        print("ERROR: Invalid URL", file=sys.stderr)
        sys.exit(1)

    print(f"[Reading: {args.url}]", file=sys.stderr)

    # Fetch
    if args.html_only:
        content, err = fetch_html_only(args.url)
    else:
        content, err = read_url(
            args.url,
            include_metadata=args.metadata,
            favor=args.favor,
            use_raw=args.raw,
            max_comments=args.comments,
        )

    if content is None:
        print(f"ERROR: {err}", file=sys.stderr)
        sys.exit(1)

    # Output
    if args.json:
        output = json.dumps({
            "url": args.url,
            "content": content,
            "length": len(content),
            "method": "reddit_json" if is_reddit_thread(args.url) and not args.raw else
                      "html2text" if args.raw else "trafilatura",
        }, indent=2, ensure_ascii=False)
        if args.output:
            with open(args.output, "w") as f:
                f.write(output)
        else:
            print(output)
    else:
        if args.output:
            with open(args.output, "w") as f:
                f.write(content)
            print(f"Saved to {args.output} ({len(content)} chars)", file=sys.stderr)
        else:
            print(content)

    print(f"[{len(content)} characters extracted]", file=sys.stderr)


if __name__ == "__main__":
    main()
