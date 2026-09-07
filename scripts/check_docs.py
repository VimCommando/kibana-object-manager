"""Check authored local Markdown links and the declared bundle index."""
import re
import os
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "crates/kibana-object-manager/docs"


def prose(text):
    # Examples inside fences describe user projects, not repository paths.
    return re.sub(r"(?ms)^(`{3,}|~{3,}).*?^\1[^\n]*$", "", text)


def check():
    errors = []
    for path in BUNDLE.rglob("*"):
        if path.name != path.name.lower():
            errors.append(f"{path.relative_to(ROOT)}: docs filenames and directories must be lowercase")
    files = [ROOT / "README.md", ROOT / "AGENTS.md", *BUNDLE.rglob("*.md")]
    for path in files:
        text = prose(path.read_text())
        for target in re.findall(r"\]\(([^)]+)\)", text):
            target = target.split(' "', 1)[0].strip("<>")
            url = urlsplit(target)
            if url.scheme or url.netloc or not url.path:
                continue
            resolved = path.parent / unquote(url.path)
            if not resolved.exists():
                errors.append(f"{path.relative_to(ROOT)}: missing link target {target}")
            else:
                # exists() alone accepts incorrect case on macOS.
                candidate = Path(os.path.abspath(resolved))
                while candidate != candidate.parent:
                    if candidate.name not in {entry.name for entry in candidate.parent.iterdir()}:
                        errors.append(f"{path.relative_to(ROOT)}: link case does not match {target}")
                        break
                    candidate = candidate.parent
    index = (BUNDLE / "index.md").read_text()
    for path in BUNDLE.glob("*.md"):
        if path.name != "index.md" and f"]({path.name})" not in index:
            errors.append(f"Bundle index omits {path.name}")
    if errors:
        raise SystemExit("\n".join(errors))
    print("Documentation links and bundle index pass")


if __name__ == "__main__":
    check()
