from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import traceback
from datetime import datetime
from pathlib import Path
from typing import Iterable


class BuildError(RuntimeError):
    pass


def find_project_root() -> Path:
    return Path(__file__).resolve().parents[1]


def build_environment(root: Path, base_env: dict[str, str] | None = None) -> dict[str, str]:
    env = dict(base_env or os.environ)
    env["SHINDEN_CLIENT_LOG_DIR"] = str(root / "logs")
    env["SHINDEN_BUILD_PROJECT_ROOT"] = str(root)
    return env


def plan_commands(
    root: Path,
    *,
    node_modules_exists: bool | None = None,
    skip_install: bool = False,
    npm_command: str = "npm",
) -> list[list[str]]:
    if node_modules_exists is None:
        node_modules_exists = (root / "node_modules").exists()

    commands: list[list[str]] = []
    if not skip_install and not node_modules_exists:
        commands.append([npm_command, "install"])

    commands.append([npm_command, "run", "tauri", "--", "build"])
    return commands


def ensure_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise BuildError(
            f"Could not find '{name}' in PATH. Install Node.js/npm and Rust/Tauri requirements, "
            "then run this generator again."
        )
    return path


def run_command(command: list[str], *, cwd: Path, env: dict[str, str], log_file) -> None:
    write_log(log_file, f"$ {' '.join(command)}")
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )

    assert process.stdout is not None
    for line in process.stdout:
        print(line, end="")
        log_file.write(line)
        log_file.flush()

    exit_code = process.wait()
    if exit_code != 0:
        raise BuildError(f"Command failed with exit code {exit_code}: {' '.join(command)}")


def collect_exe_artifacts(root: Path) -> list[Path]:
    release_dir = root / "src-tauri" / "target" / "release"
    artifacts: list[Path] = []

    standalone = release_dir / "ShindenClient.exe"
    if standalone.exists():
        artifacts.append(standalone)

    bundle_dir = release_dir / "bundle"
    if bundle_dir.exists():
        artifacts.extend(sorted(bundle_dir.rglob("*.exe")))

    return artifacts


def copy_artifacts(artifacts: Iterable[Path], dist_dir: Path) -> list[Path]:
    dist_dir.mkdir(parents=True, exist_ok=True)
    copied: list[Path] = []

    for artifact in artifacts:
        destination = dist_dir / artifact.name
        counter = 2
        while destination.exists():
            destination = dist_dir / f"{artifact.stem}-{counter}{artifact.suffix}"
            counter += 1
        shutil.copy2(artifact, destination)
        copied.append(destination)

    return copied


def clean_dist(root: Path, dist_dir: Path) -> None:
    resolved_root = root.resolve()
    resolved_dist = dist_dir.resolve()
    if resolved_dist == resolved_root or resolved_root not in resolved_dist.parents:
        raise BuildError(f"Refusing to clean a directory outside the project: {resolved_dist}")
    if dist_dir.exists():
        shutil.rmtree(dist_dir)


def write_log(log_file, message: str) -> None:
    line = f"[{datetime.now().isoformat(timespec='seconds')}] {message}\n"
    print(line, end="")
    log_file.write(line)
    log_file.flush()


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build a local Shinden Client Windows EXE.")
    parser.add_argument("--skip-install", action="store_true", help="Do not run npm install when node_modules is missing.")
    parser.add_argument("--clean", action="store_true", help="Remove dist-exe before copying new artifacts.")
    parser.add_argument("--dry-run", action="store_true", help="Print planned commands without running the build.")
    parser.add_argument("--no-copy", action="store_true", help="Leave artifacts in src-tauri/target/release only.")
    parser.add_argument("--dist", default="dist-exe", help="Output directory for copied EXE artifacts.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root = find_project_root()
    log_dir = root / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / "build-exe.log"
    dist_dir = root / args.dist

    with log_path.open("a", encoding="utf-8") as log_file:
        write_log(log_file, "Starting local EXE build")
        write_log(log_file, f"Project root: {root}")
        write_log(log_file, f"Runtime app logs: {log_dir / 'shinden-client.log'}")

        npm_command = (shutil.which("npm") or "npm") if args.dry_run else ensure_tool("npm")
        env = build_environment(root)
        commands = plan_commands(
            root,
            skip_install=args.skip_install,
            npm_command=npm_command,
        )

        if args.dry_run:
            write_log(log_file, "Dry run only. Planned commands:")
            for command in commands:
                write_log(log_file, f"  {' '.join(command)}")
            return 0

        if args.clean:
            clean_dist(root, dist_dir)

        for command in commands:
            run_command(command, cwd=root, env=env, log_file=log_file)

        artifacts = collect_exe_artifacts(root)
        if not artifacts:
            raise BuildError("Build finished, but no EXE artifacts were found in src-tauri/target/release.")

        if args.no_copy:
            write_log(log_file, "EXE artifacts left in place:")
            for artifact in artifacts:
                write_log(log_file, f"  {artifact}")
            return 0

        copied = copy_artifacts(artifacts, dist_dir)
        write_log(log_file, "Copied EXE artifacts:")
        for artifact in copied:
            write_log(log_file, f"  {artifact}")

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        root = find_project_root()
        log_dir = root / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        with (log_dir / "build-exe.log").open("a", encoding="utf-8") as log_file:
            write_log(log_file, f"Build failed: {error}")
            log_file.write(traceback.format_exc())
            log_file.flush()
        print(f"Build failed. See {log_dir / 'build-exe.log'}", file=sys.stderr)
        raise SystemExit(1)
