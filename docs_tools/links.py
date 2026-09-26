"""Keep source links local in Markdown and point outside-site links to GitHub."""
from pathlib import Path
import re

REPOSITORIES = {
    "reps-for-prompts": "https://github.com/factory-level/reps-for-prompts",
    "usb-mcp-hub": "https://github.com/factory-level/usb-mcp-hub",
}


def on_page_markdown(markdown, page, config, **kwargs):
    docs = Path(config["docs_dir"]).resolve()
    workspace = docs.parent.parent
    source = Path(page.file.abs_src_path)

    def replace(match):
        target = match.group(1)
        if re.match(r"\w+:|#|/", target):
            return match.group(0)
        path, separator, fragment = target.partition("#")
        resolved = (source.parent / path).resolve()
        if resolved.is_relative_to(docs):
            return match.group(0)
        for name, url in REPOSITORIES.items():
            repo = workspace / name
            if resolved.is_relative_to(repo):
                relative = resolved.relative_to(repo).as_posix()
                return f"]({url}/blob/HEAD/{relative}{separator}{fragment})"
        return match.group(0)

    # Leave fenced examples intact.
    parts = re.split(r"(```.*?```|~~~.*?~~~)", markdown, flags=re.S)
    for i in range(0, len(parts), 2):
        parts[i] = re.sub(r"\]\(([^\s)]+)\)", replace, parts[i])
    return "".join(parts)
