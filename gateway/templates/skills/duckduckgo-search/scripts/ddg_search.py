#!/usr/bin/env python3
"""
DuckDuckGo Search — Unified search across text, news, images, and videos.

Tries the newer `ddgs` metasearch library first, falls back to `duckduckgo-search`.
No API key required.

Usage:
    python ddg_search.py text "query" [OPTIONS]
    python ddg_search.py news "query" [OPTIONS]
    python ddg_search.py images "query" [OPTIONS]
    python ddg_search.py videos "query" [OPTIONS]

    Options:
      --max-results N     Maximum results (default: 10)
      --region REGION     Region code like us-en, uk-en (default: us-en)
      --timelimit T       d=day, w=week, m=month, y=year (default: None)
      --safesearch S      on, moderate, off (default: moderate)
      --json              Output raw JSON
      --output FILE       Save results to file
"""

import argparse
import json
import sys
import time

# Try importing ddgs (newer metasearch), fall back to duckduckgo-search
DDGS = None
LIB_NAME = None

try:
    from ddgs import DDGS as _DDGS
    DDGS = _DDGS
    LIB_NAME = "ddgs"
except ImportError:
    try:
        from duckduckgo_search import DDGS as _DDGS
        DDGS = _DDGS
        LIB_NAME = "duckduckgo-search"
    except ImportError:
        print("ERROR: Neither 'ddgs' nor 'duckduckgo-search' is installed.", file=sys.stderr)
        print("Install with: pip install ddgs  (or)  pip install duckduckgo-search", file=sys.stderr)
        sys.exit(1)


def search_text(query, max_results=10, region="us-en", timelimit=None, safesearch="moderate"):
    """Search the web for text results."""
    try:
        results = DDGS().text(
            query,
            region=region,
            safesearch=safesearch,
            timelimit=timelimit,
            max_results=max_results,
        )
        return list(results) if hasattr(results, '__iter__') and not isinstance(results, list) else results
    except Exception as e:
        print(f"Search error: {e}", file=sys.stderr)
        return []


def search_news(query, max_results=10, region="us-en", timelimit=None, safesearch="moderate"):
    """Search for news articles."""
    try:
        results = DDGS().news(
            query,
            region=region,
            safesearch=safesearch,
            timelimit=timelimit,
            max_results=max_results,
        )
        return list(results) if hasattr(results, '__iter__') and not isinstance(results, list) else results
    except Exception as e:
        print(f"News search error: {e}", file=sys.stderr)
        return []


def search_images(query, max_results=5, region="us-en", timelimit=None, safesearch="moderate",
                  size=None, color=None, type_image=None, layout=None, license_image=None):
    """Search for images."""
    try:
        kwargs = {
            "region": region,
            "safesearch": safesearch,
            "timelimit": timelimit,
            "max_results": max_results,
        }
        # Only pass optional params if set (API compatibility)
        if size:
            kwargs["size"] = size
        if color:
            kwargs["color"] = color
        if type_image:
            kwargs["type_image"] = type_image
        if layout:
            kwargs["layout"] = layout
        if license_image:
            kwargs["license_image"] = license_image

        results = DDGS().images(query, **kwargs)
        return list(results) if hasattr(results, '__iter__') and not isinstance(results, list) else results
    except Exception as e:
        print(f"Image search error: {e}", file=sys.stderr)
        return []


def search_videos(query, max_results=5, region="us-en", timelimit=None, safesearch="moderate",
                  resolution=None, duration=None):
    """Search for videos."""
    try:
        kwargs = {
            "region": region,
            "safesearch": safesearch,
            "timelimit": timelimit,
            "max_results": max_results,
        }
        if resolution:
            kwargs["resolution"] = resolution
        if duration:
            kwargs["duration"] = duration

        results = DDGS().videos(query, **kwargs)
        return list(results) if hasattr(results, '__iter__') and not isinstance(results, list) else results
    except Exception as e:
        print(f"Video search error: {e}", file=sys.stderr)
        return []


def display_text_results(results):
    """Display text search results in readable format."""
    if not results:
        print("No results found.")
        return

    for i, r in enumerate(results, 1):
        title = r.get("title", "No title")
        href = r.get("href", r.get("url", ""))
        body = r.get("body", r.get("snippet", ""))[:200]
        print(f"\n{i}. {title}")
        print(f"   {href}")
        if body:
            print(f"   {body}")


def display_news_results(results):
    """Display news search results."""
    if not results:
        print("No news results found.")
        return

    for i, r in enumerate(results, 1):
        title = r.get("title", "No title")
        url = r.get("url", r.get("href", ""))
        source = r.get("source", "Unknown")
        date = r.get("date", "")
        body = r.get("body", "")[:200]

        print(f"\n{i}. [{source}] {title}")
        if date:
            print(f"   Date: {date}")
        print(f"   {url}")
        if body:
            print(f"   {body}")


def display_image_results(results):
    """Display image search results."""
    if not results:
        print("No image results found.")
        return

    for i, r in enumerate(results, 1):
        title = r.get("title", "No title")
        image_url = r.get("image", "")
        source_url = r.get("url", "")
        width = r.get("width", "?")
        height = r.get("height", "?")

        print(f"\n{i}. {title}")
        print(f"   Image: {image_url}")
        print(f"   Source: {source_url}")
        print(f"   Size: {width}x{height}")


def display_video_results(results):
    """Display video search results."""
    if not results:
        print("No video results found.")
        return

    for i, r in enumerate(results, 1):
        title = r.get("title", "No title")
        content_url = r.get("content", "")
        publisher = r.get("publisher", "")
        uploader = r.get("uploader", "")
        duration = r.get("duration", "")
        description = r.get("description", "")[:150]

        print(f"\n{i}. {title}")
        if publisher or uploader:
            print(f"   By: {uploader or publisher}" + (f" ({publisher})" if uploader and publisher else ""))
        if duration:
            print(f"   Duration: {duration}")
        print(f"   {content_url}")
        if description:
            print(f"   {description}")


def main():
    parser = argparse.ArgumentParser(description="DuckDuckGo Search Tool")
    subparsers = parser.add_subparsers(dest="mode", help="Search mode")

    # Shared arguments
    for name in ["text", "news", "images", "videos"]:
        sp = subparsers.add_parser(name, help=f"Search {name}")
        sp.add_argument("query", type=str, help="Search query")
        sp.add_argument("--max-results", type=int, default=10 if name in ("text", "news") else 5)
        sp.add_argument("--region", type=str, default="us-en")
        sp.add_argument("--timelimit", type=str, default=None, choices=["d", "w", "m", "y", None])
        sp.add_argument("--safesearch", type=str, default="moderate", choices=["on", "moderate", "off"])
        sp.add_argument("--json", action="store_true", help="Output raw JSON")
        sp.add_argument("--output", type=str, help="Save results to file")

        # Mode-specific
        if name == "images":
            sp.add_argument("--size", choices=["Small", "Medium", "Large", "Wallpaper"])
            sp.add_argument("--color", type=str)
            sp.add_argument("--type", dest="type_image", choices=["photo", "clipart", "gif", "transparent", "line"])
            sp.add_argument("--layout", choices=["Square", "Tall", "Wide"])
        elif name == "videos":
            sp.add_argument("--resolution", choices=["high", "standard"])
            sp.add_argument("--duration", choices=["short", "medium", "long"])

    args = parser.parse_args()

    if not args.mode:
        parser.print_help()
        sys.exit(1)

    print(f"[Using library: {LIB_NAME}]", file=sys.stderr)

    # Execute search
    if args.mode == "text":
        results = search_text(args.query, args.max_results, args.region, args.timelimit, args.safesearch)
    elif args.mode == "news":
        results = search_news(args.query, args.max_results, args.region, args.timelimit, args.safesearch)
    elif args.mode == "images":
        results = search_images(args.query, args.max_results, args.region, args.timelimit, args.safesearch,
                                getattr(args, "size", None), getattr(args, "color", None),
                                getattr(args, "type_image", None), getattr(args, "layout", None))
    elif args.mode == "videos":
        results = search_videos(args.query, args.max_results, args.region, args.timelimit, args.safesearch,
                                getattr(args, "resolution", None), getattr(args, "duration", None))
    else:
        parser.print_help()
        sys.exit(1)

    # Output
    if args.json:
        output = json.dumps(results, indent=2, default=str, ensure_ascii=False)
        if args.output:
            with open(args.output, "w") as f:
                f.write(output)
            print(f"Saved {len(results)} results to {args.output}")
        else:
            print(output)
    else:
        display_fn = {
            "text": display_text_results,
            "news": display_news_results,
            "images": display_image_results,
            "videos": display_video_results,
        }[args.mode]
        display_fn(results)

        if args.output:
            with open(args.output, "w") as f:
                json.dump(results, f, indent=2, default=str, ensure_ascii=False)
            print(f"\nSaved {len(results)} results to {args.output}")

    print(f"\n[{len(results)} results returned]", file=sys.stderr)


if __name__ == "__main__":
    main()
