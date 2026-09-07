"""Validate PR titles and committed, PR-scoped OpenSpec completion.

Compare full requirement bodies after whitespace normalization. This deliberately
requires synchronized wording; it does not claim to infer semantic equivalence.
"""
import argparse
import json
import re
import subprocess
from pathlib import Path


def git(*args):
    return subprocess.check_output(["git", *args]).decode()


def requirements(text):
    blocks = re.split(r"(?m)^### Requirement: (.+)\n", text)
    return {
        blocks[i].strip(): " ".join(blocks[i + 1].split())
        for i in range(1, len(blocks), 2)
    }


def compare_delta(delta, main):
    current = requirements(main)
    errors = []
    sections = re.split(r"(?m)^## (ADDED|MODIFIED|REMOVED|RENAMED) Requirements\s*\n", delta)
    if len(sections) == 1:
        return ["delta has no recognized requirement sections"]
    for i in range(1, len(sections), 2):
        operation, body = sections[i:i + 2]
        if operation == "RENAMED":
            pairs = re.findall(
                r'- FROM: `### Requirement: (.+)`\s+- TO: `### Requirement: (.+)`', body
            )
            if not pairs:
                errors.append("unrecognized rename syntax")
            for old, new in pairs:
                if old in current or new not in current:
                    errors.append(f"rename not synchronized: {old} -> {new}")
            continue
        changed = requirements(body)
        if not changed:
            errors.append(f"empty {operation} section")
        for name, content in changed.items():
            if operation == "REMOVED":
                if name in current:
                    errors.append(f"removed requirement remains: {name}")
            elif current.get(name) != content:
                errors.append(f"{operation.lower()} requirement not synchronized: {name}")
    return errors


def associated_ids(paths):
    result = set()
    for path in paths:
        parts = path.split("/")
        if parts[:2] != ["openspec", "changes"] or len(parts) < 4:
            continue
        if parts[2] == "archive" and len(parts) >= 5:
            match = re.fullmatch(r"\d{4}-\d{2}-\d{2}-(.+)", parts[3])
            if match:
                result.add(match[1])
        elif parts[2] != "archive":
            result.add(parts[2])
    return result


def validate(base, head, title, body):
    errors = []
    if not re.fullmatch(r"(?:feat|fix|docs|refactor|perf|test|build|ci|chore|revert)(?:\([^)\n]+\))?!?: .+", title):
        errors.append("PR title must be a Conventional Commit squash message")
    field = re.findall(r"(?m)^OpenSpec-Changes:\s*([^\n]+)$", body)
    if len(field) != 1:
        errors.append("Specify one OpenSpec-Changes: none or a comma-separated list of change IDs")
        declared = set()
    else:
        declared = {item.strip() for item in field[0].split(",")}
        if declared == {"none"}:
            declared = set()
        elif any(not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", item) or item == "none" for item in declared):
            errors.append("Invalid OpenSpec change ID list")
            declared = set()
    merge_base = git("merge-base", base, head).strip()
    paths = git("diff", "--name-only", "--no-renames", "-z", merge_base, head).split("\0")
    ids = associated_ids(paths) | declared
    files = set(git("ls-tree", "-r", "--name-only", head).splitlines())
    read = lambda path: git("show", f"{head}:{path}")
    for change in sorted(ids):
        active = f"openspec/changes/{change}/"
        archives = {
            "/".join(path.split("/")[:4]) for path in files
            if re.match(rf"openspec/changes/archive/\d{{4}}-\d{{2}}-\d{{2}}-{re.escape(change)}/", path)
        }
        if any(path.startswith(active) for path in files):
            errors.append(f"{change}: active change must be archived")
        if len(archives) != 1:
            errors.append(f"{change}: expected exactly one preserved archive")
            continue
        archive = archives.pop()
        for artifact in ["proposal.md", "tasks.md"]:
            if f"{archive}/{artifact}" not in files:
                errors.append(f"{change}: archive missing {artifact}")
        tasks = f"{archive}/tasks.md"
        if tasks in files and re.search(r"(?m)^\s*- \[ \]", read(tasks)):
            errors.append(f"{change}: unfinished archived tasks")
        deltas = sorted(p for p in files if p.startswith(f"{archive}/specs/") and p.endswith("/spec.md"))
        if not deltas:
            explanation = f"{archive}/no-spec-deltas.md"
            if explanation not in files or not read(explanation).strip():
                errors.append(f"{change}: document and review why this change has no spec deltas")
        for delta in deltas:
            main = "openspec/" + delta.removeprefix(archive + "/")
            main_text = read(main) if main in files else ""
            errors.extend(f"{change}: {delta}: {error}" for error in compare_delta(read(delta), main_text))
    if errors:
        raise ValueError("\n".join(errors))
    print("PR contract passes; OpenSpec changes: " + (", ".join(sorted(ids)) or "not applicable"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--event", type=Path, required=True, help="GitHub pull_request event JSON")
    args = parser.parse_args()
    pr = json.loads(args.event.read_text())["pull_request"]
    try:
        validate(args.base, args.head, pr["title"], pr.get("body") or "")
    except ValueError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
