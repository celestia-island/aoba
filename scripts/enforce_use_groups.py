#!/usr/bin/env python3
"""Enforce workspace-specific `use` statement layout rules.

The policy is described in `.github/copilot-instructions.md` and can be summarized as:

1. Group imports into three sections: shared utility crates (`std`, `anyhow`, `serde`, ...),
   domain-specific external crates (e.g. `serialport`, `rmodbus`, `ratatui`), and
   workspace/internal crates (`crate::`, `super::`, or other workspace packages).
2. Place a single blank line between groups and after the final group before code.
3. In `mod.rs`, `lib.rs`, and `main.rs`, emit all `mod` declarations before the first
   `use` block.
4. Merge consecutive simple paths that share a prefix (`use std::sync::Arc;` and
   `use std::collections::HashMap;` -> `use std::{collections::HashMap, sync::Arc};`).

This script rewrites files in-place and prints a summary of modified files.
"""

from __future__ import annotations

import os
import re
import sys
from collections import OrderedDict
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

try:  # Python 3.11+
    import tomllib  # type: ignore
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

GROUP1_CRATES = {
    "std",
    "core",
    "alloc",
    "anyhow",
    "serde",
    "serde_json",
    "serde_yaml",
    "serde_repr",
    "serde_with",
    "serde_bytes",
    "serde_path_to_error",
    "serde_cbor",
    "serde_urlencoded",
    "toml",
    "ron",
    "regex",
    "lazy_static",
    "once_cell",
    "clap",
    "tokio",
    "futures",
    "async_std",
    "time",
    "chrono",
    "log",
    "env_logger",
    "thiserror",
    "color_eyre",
    "eyre",
    "uuid",
    "tempfile",
    "rand",
    "base64",
    "bytes",
    "smallvec",
    "itertools",
    "indexmap",
    "cfg_if",
    "tracing",
    "tracing_subscriber",
    "indicatif",
    "parking_lot",
    "crossbeam",
    "tokio_stream",
    "tokio_util",
    "tokio_macros",
    "reqwest",
    "hyper",
    "anymap",
    "bstr",
    "dashmap",
    "hashbrown",
    "im",
    "prost",
    "prost_types",
    "tonic",
    "url",
    "urlencoding",
    "walkdir",
    "which",
    "rayon",
    "backtrace",
    "bytemuck",
    "bytesize",
    "csv",
    "flate2",
    "glob",
    "hex",
    "http",
    "num",
    "num_traits",
    "ordered_float",
    "petgraph",
    "rand_chacha",
    "rand_core",
    "redox_syscall",
    "rust_decimal",
    "serde_xml_rs",
    "sha2",
    "snap",
    "strum",
    "strum_macros",
    "tokio_test",
    "tracing_core",
    "typemap",
    "warp",
    "whoami",
    "xml_rs",
}

USE_RE = re.compile(r"^\s*(pub\s+)?use\b")
MOD_RE = re.compile(r"^\s*(pub\s+)?mod\b")
ATTR_RE = re.compile(r"^\s*#\[")
COMMENT_RE = re.compile(r"^\s*//")
BLOCK_COMMENT_START_RE = re.compile(r"^\s*/\*")


WORKSPACE_CRATES: set[str] = set()


@dataclass
class UseStatement:
    lines: List[str]
    path: Optional[str]
    is_pub: bool
    group: int
    simple_prefix: Optional[str]
    simple_leaf: Optional[str]
    has_attrs: bool

    def text(self) -> str:
        return "".join(self.lines)


@dataclass
class Statement:
    kind: str  # "use" or "mod"
    lines: List[str]
    use_stmt: Optional[UseStatement] = None


def load_workspace_crates(root: Path) -> set[str]:
    crates: set[str] = set()
    for dirpath, dirnames, filenames in os.walk(root):
        if "target" in dirnames:
            dirnames.remove("target")
        if "Cargo.toml" not in filenames:
            continue
        path = Path(dirpath) / "Cargo.toml"
        try:
            with path.open("rb") as fh:
                data = tomllib.load(fh)
        except Exception:
            continue
        package = data.get("package")
        if package and "name" in package:
            crates.add(package["name"])
    return crates


def extract_use_path(lines: Sequence[str]) -> Optional[str]:
    joined = " ".join(line.strip() for line in lines if not ATTR_RE.match(line))
    match = re.search(r"\buse\s+([^;]+);", joined)
    return match.group(1).strip() if match else None


def classify_use(path: Optional[str], workspace_crates: set[str]) -> int:
    if not path:
        return 2
    token = path.strip()
    if token.startswith("pub "):
        token = token[4:].strip()
    if token.startswith("use "):
        token = token[4:].strip()
    if token.startswith(("crate::", "self::", "super::")):
        return 3
    if token.startswith("::"):
        token = token.lstrip(":")
    base = re.split(r"::|,|\s|{", token, maxsplit=1)[0]
    if base in ("crate", "self", "super"):
        return 3
    if base in workspace_crates:
        return 3
    if base in GROUP1_CRATES:
        return 1
    return 2


def compute_simple_components(path: Optional[str], has_attrs: bool) -> Tuple[Optional[str], Optional[str]]:
    if has_attrs or not path:
        return None, None
    token = path.strip()
    if any(ch in token for ch in "{}*"):
        return None, None
    if " as " in token:
        return None, None
    if token.endswith(":"):
        return None, None
    parts = token.split("::")
    if len(parts) < 2:
        return None, None
    prefix = "::".join(parts[:-1])
    leaf = parts[-1].strip()
    if not prefix or not leaf:
        return None, None
    return prefix, leaf


def append_blank_line(buf: List[str]) -> None:
    if not buf:
        return
    if buf[-1].strip():
        buf.append("\n")


def collect_statement(lines: List[str], idx: int) -> Tuple[Optional[Statement], int]:
    attrs: List[str] = []
    cur = idx
    while cur < len(lines) and ATTR_RE.match(lines[cur]):
        attrs.append(lines[cur])
        cur += 1
    if cur >= len(lines):
        return None, idx

    line = lines[cur]
    if USE_RE.match(line):
        stmt_lines = attrs + [line]
        cur += 1
        brace_balance = line.count("{") - line.count("}")
        semicolon_found = ";" in line and brace_balance <= 0
        while not semicolon_found and cur < len(lines):
            stmt_lines.append(lines[cur])
            brace_balance += lines[cur].count("{") - lines[cur].count("}")
            if ";" in lines[cur] and brace_balance <= 0:
                semicolon_found = True
            cur += 1
        use_stmt = build_use_statement(stmt_lines)
        return Statement("use", stmt_lines, use_stmt), cur

    if MOD_RE.match(line):
        stmt_lines = attrs + [line]
        cur += 1
        return Statement("mod", stmt_lines), cur

    return None, idx


def build_use_statement(lines: List[str]) -> UseStatement:
    code_lines = [line for line in lines if not ATTR_RE.match(line)]
    path = extract_use_path(lines)
    is_pub = bool(code_lines and code_lines[0].lstrip().startswith("pub "))
    has_attrs = any(ATTR_RE.match(line) for line in lines)
    prefix, leaf = compute_simple_components(path, has_attrs)
    group = classify_use(path, WORKSPACE_CRATES)
    return UseStatement(lines, path, is_pub, group, prefix, leaf, has_attrs)


def flush_simple(pending: OrderedDict[Tuple[bool, str], List[str]], output: List[str]) -> None:
    for (is_pub, prefix), leaves in pending.items():
        unique_leaves: List[str] = []
        seen = set()
        for leaf in leaves:
            if leaf not in seen:
                unique_leaves.append(leaf)
                seen.add(leaf)
        if not unique_leaves:
            continue
        if len(unique_leaves) == 1:
            line = f"{'pub ' if is_pub else ''}use {prefix}::{unique_leaves[0]};\n"
        else:
            inner = ", ".join(unique_leaves)
            line = f"{'pub ' if is_pub else ''}use {prefix}::{{{inner}}};\n"
        output.append(line)
    pending.clear()


def render_group(statements: List[UseStatement]) -> List[str]:
    if not statements:
        return []
    output: List[str] = []
    pending: OrderedDict[Tuple[bool, str], List[str]] = OrderedDict()

    for stmt in statements:
        if stmt.simple_prefix and stmt.simple_leaf and not stmt.has_attrs:
            key = (stmt.is_pub, stmt.simple_prefix)
            if key not in pending:
                pending[key] = []
            pending[key].append(stmt.simple_leaf)
            continue

        flush_simple(pending, output)
        output.extend(stmt.lines)

    flush_simple(pending, output)
    return output


def render_use_section(use_statements: List[UseStatement]) -> List[str]:
    grouped = {1: [], 2: [], 3: []}
    for stmt in use_statements:
        grouped.setdefault(stmt.group, []).append(stmt)

    rendered: List[str] = []
    for group in (1, 2, 3):
        block = render_group(grouped.get(group, []))
        if not block:
            continue
        if rendered and rendered[-1].strip():
            rendered.append("\n")
        rendered.extend(block)
    if rendered and rendered[-1].strip():
        rendered.append("\n")
    return rendered


def process_file(path: Path) -> Optional[str]:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    idx = 0
    prefix: List[str] = []

    while idx < len(lines):
        stripped = lines[idx].strip()
        if stripped == "" or COMMENT_RE.match(lines[idx]) or BLOCK_COMMENT_START_RE.match(lines[idx]):
            prefix.append(lines[idx])
            idx += 1
            continue
        if lines[idx].startswith("#!"):
            prefix.append(lines[idx])
            idx += 1
            continue
        if ATTR_RE.match(lines[idx]):
            break
        if USE_RE.match(lines[idx]) or MOD_RE.match(lines[idx]):
            break
        return None  # No leading use/mod block to normalize

    statements: List[Statement] = []
    cur = idx
    while cur < len(lines):
        stmt, next_idx = collect_statement(lines, cur)
        if stmt is None:
            break
        statements.append(stmt)
        cur = next_idx
        while cur < len(lines) and lines[cur].strip() == "":
            # absorb blank lines after the block so we can re-insert uniformly later
            cur += 1
    suffix = lines[cur:]

    if not statements:
        return None

    use_statements = [stmt.use_stmt for stmt in statements if stmt.kind == "use" and stmt.use_stmt]
    if not use_statements:
        return None

    use_section = render_use_section(use_statements)
    if not use_section:
        return None

    require_mod_first = path.name in {"mod.rs", "lib.rs", "main.rs"}

    new_lines: List[str] = []
    new_lines.extend(prefix)

    if require_mod_first:
        mods = [stmt.lines for stmt in statements if stmt.kind == "mod"]
        for mod_lines in mods:
            new_lines.extend(mod_lines)
        if mods and use_section:
            append_blank_line(new_lines)
        new_lines.extend(use_section)
    else:
        others_written = False
        for stmt in statements:
            if stmt.kind == "use":
                continue
            new_lines.extend(stmt.lines)
            others_written = True
        if others_written and use_section:
            append_blank_line(new_lines)
        new_lines.extend(use_section)

    new_lines.extend(suffix)

    if new_lines and not new_lines[-1].endswith("\n"):
        new_lines[-1] += "\n"

    new_text = "".join(new_lines)
    original_text = "".join(lines)
    return new_text if new_text != original_text else None


def main() -> int:
    changed: List[str] = []
    for path in Path.cwd().rglob("*.rs"):
        if "target" in path.parts:
            continue
        new_text = process_file(path)
        if new_text is not None:
            path.write_text(new_text, encoding="utf-8")
            changed.append(str(path))

    print(f"Updated {len(changed)} files")
    for item in changed:
        print(item)
    return 0


if __name__ == "__main__":
    WORKSPACE_CRATES = load_workspace_crates(Path.cwd())
    sys.exit(main())
