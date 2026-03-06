#!/usr/bin/env python3
"""Validate the repository skill package and its CLI references."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SKILL_DIR = REPO_ROOT / "skills" / "sftool"
SKILL_MD = SKILL_DIR / "SKILL.md"
FORBIDDEN_FILES = {
    "README.md",
    "README_EN.md",
    "CHANGELOG.md",
    "INSTALLATION_GUIDE.md",
    "QUICK_REFERENCE.md",
}
REQUIRED_FILES = [
    SKILL_MD,
    SKILL_DIR / "references" / "commands.md",
    SKILL_DIR / "references" / "config-examples.md",
    SKILL_DIR / "references" / "troubleshooting.md",
    SKILL_DIR / "assets" / "base-config.json",
    SKILL_DIR / "assets" / "write-flash-config.json",
    SKILL_DIR / "assets" / "read-flash-config.json",
]
REQUIRED_LINKS = [
    "./references/commands.md",
    "./references/config-examples.md",
    "./references/troubleshooting.md",
    "./assets/base-config.json",
    "./assets/write-flash-config.json",
    "./assets/read-flash-config.json",
]
REQUIRED_PHRASES = [
    "SFTOOL_BIN",
    "write_flash",
    "read_flash",
    "erase_region",
    "config",
    "--verify",
    "--erase-all",
    "Error: sftool command not found in PATH or SFTOOL_BIN. Stop and ask the user to install sftool or fix the environment before continuing.",
]
HELP_COMMANDS = [
    (
        ["cargo", "run", "-p", "sftool", "--", "--help"],
        ["write_flash", "read_flash", "erase_region", "config"],
    ),
    (
        ["cargo", "run", "-p", "sftool", "--", "write_flash", "--help"],
        ["--verify", "--erase-all", "--no-compress"],
    ),
    (
        ["cargo", "run", "-p", "sftool", "--", "read_flash", "--help"],
        ["filename@address:size"],
    ),
    (
        ["cargo", "run", "-p", "sftool", "--", "erase_region", "--help"],
        ["address:size"],
    ),
    (
        ["cargo", "run", "-p", "sftool", "--", "config", "--help"],
        ["JSON configuration file path"],
    ),
]
COMMAND_KEYS = {"write_flash", "read_flash", "erase_flash", "erase_region", "stub"}


def fail(message: str) -> None:
    print(f"[ERROR] {message}")
    raise SystemExit(1)


def parse_frontmatter(text: str) -> dict[str, str]:
    match = re.match(r"^---\n(.*?)\n---\n", text, re.DOTALL)
    if not match:
        fail("SKILL.md is missing valid YAML frontmatter.")

    data: dict[str, str] = {}
    for raw_line in match.group(1).splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if raw_line.startswith((" ", "\t")):
            fail("Frontmatter must stay flat; nested YAML is not allowed in this repo.")
        if ":" not in line:
            fail(f"Invalid frontmatter line: {raw_line}")
        key, value = line.split(":", 1)
        key = key.strip()
        value = value.strip()
        if not value:
            fail(f"Frontmatter key '{key}' must not be empty.")
        if value.startswith(("'", '"')) and value.endswith(("'", '"')) and len(value) >= 2:
            value = value[1:-1]
        data[key] = value
    return data


def validate_skill_structure() -> None:
    if not SKILL_DIR.is_dir():
        fail("skills/sftool directory is missing.")

    for path in REQUIRED_FILES:
        if not path.exists():
            fail(f"Required skill file is missing: {path.relative_to(REPO_ROOT)}")

    for path in SKILL_DIR.rglob("*"):
        if path.is_file() and path.name in FORBIDDEN_FILES:
            fail(f"Forbidden file in skill package: {path.relative_to(REPO_ROOT)}")


def validate_frontmatter_and_links() -> None:
    content = SKILL_MD.read_text(encoding="utf-8")
    frontmatter = parse_frontmatter(content)

    if set(frontmatter) != {"name", "description"}:
        fail("SKILL.md frontmatter must contain only 'name' and 'description'.")
    if frontmatter["name"] != "sftool":
        fail("SKILL.md name must be 'sftool'.")
    if SKILL_DIR.name != frontmatter["name"]:
        fail("Skill directory name must match the frontmatter name.")
    if len(frontmatter["description"]) > 200:
        fail("SKILL.md description must stay within 200 characters for Claude compatibility.")

    for link in REQUIRED_LINKS:
        if link not in content:
            fail(f"SKILL.md must reference {link}.")

    for phrase in REQUIRED_PHRASES:
        if phrase not in content:
            fail(f"SKILL.md is missing required phrase: {phrase}")


def validate_assets() -> None:
    expected_commands = {
        SKILL_DIR / "assets" / "base-config.json": "write_flash",
        SKILL_DIR / "assets" / "write-flash-config.json": "write_flash",
        SKILL_DIR / "assets" / "read-flash-config.json": "read_flash",
    }

    for path, expected_command in expected_commands.items():
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            fail(f"Invalid JSON in {path.relative_to(REPO_ROOT)}: {exc}")

        command_keys = [key for key in COMMAND_KEYS if key in data]
        if len(command_keys) != 1:
            fail(
                f"{path.relative_to(REPO_ROOT)} must contain exactly one command block."
            )
        if command_keys[0] != expected_command:
            fail(
                f"{path.relative_to(REPO_ROOT)} must use '{expected_command}' as its command block."
            )


def run_help_checks() -> None:
    for command, required_strings in HELP_COMMANDS:
        result = subprocess.run(
            command,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        output = f"{result.stdout}\n{result.stderr}"
        if result.returncode != 0:
            fail(
                "Command failed during validation: "
                + " ".join(command)
                + f"\n{output.strip()}"
            )
        for required in required_strings:
            if required not in output:
                fail(
                    f"Command {' '.join(command)} did not include expected text: {required}"
                )


def main() -> None:
    validate_skill_structure()
    validate_frontmatter_and_links()
    validate_assets()
    run_help_checks()
    print("[OK] skill package is valid")


if __name__ == "__main__":
    main()
