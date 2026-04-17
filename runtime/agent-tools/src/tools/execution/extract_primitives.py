#!/usr/bin/env python3
"""Extract function primitives from a Python file.

Usage: extract_primitives.py <path>
Stdout: JSON list of {name, signature, summary} per top-level function or class method.
Stderr: parse errors (non-fatal; caller logs and skips).

Deterministic. No LLM. Used by the write_file/edit_file post-hook to
populate memory_facts with primitive signatures. Stays tight and
focused: top-level defs + class methods, public (non-underscore)
names only, first line of docstring as summary.
"""
import ast
import json
import sys


def _format_args(args: ast.arguments) -> str:
    pos = [a.arg for a in args.args]
    if args.vararg:
        pos.append(f"*{args.vararg.arg}")
    if args.kwonlyargs:
        if not args.vararg:
            pos.append("*")
        pos.extend(a.arg for a in args.kwonlyargs)
    if args.kwarg:
        pos.append(f"**{args.kwarg.arg}")
    return ", ".join(pos)


def _format_return(node) -> str:
    if node.returns is None:
        return ""
    try:
        return f" -> {ast.unparse(node.returns)}"
    except Exception:
        return ""


def _summary(node) -> str:
    doc = ast.get_docstring(node) or ""
    first_line = doc.strip().split("\n", 1)[0].strip()
    return first_line[:200]


def _is_public(name: str) -> bool:
    return not name.startswith("_")


def extract(source: str) -> list[dict]:
    tree = ast.parse(source)
    out = []
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and _is_public(node.name):
            out.append({
                "name": node.name,
                "signature": f"{node.name}({_format_args(node.args)}){_format_return(node)}",
                "summary": _summary(node),
            })
        elif isinstance(node, ast.ClassDef) and _is_public(node.name):
            for item in node.body:
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)) and _is_public(item.name):
                    out.append({
                        "name": f"{node.name}.{item.name}",
                        "signature": f"{node.name}.{item.name}({_format_args(item.args)}){_format_return(item)}",
                        "summary": _summary(item),
                    })
    return out


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.stderr.write("usage: extract_primitives.py <path>\n")
        sys.exit(2)
    try:
        with open(sys.argv[1], "r", encoding="utf-8") as f:
            source = f.read()
        print(json.dumps(extract(source)))
    except SyntaxError as e:
        sys.stderr.write(f"syntax error: {e}\n")
        sys.exit(1)
    except Exception as e:
        sys.stderr.write(f"extract failed: {e}\n")
        sys.exit(1)
