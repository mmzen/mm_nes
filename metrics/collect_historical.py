#!/usr/bin/env python3
# Authorship: Human 0% | Claude 100%
"""
Collect metrics for historical commits.

Usage:
    python3 collect_historical.py --commits 10
    python3 collect_historical.py --commits 10 --with-coverage
"""

import argparse
import atexit
import signal
import subprocess
import sys
from pathlib import Path
from typing import Optional


METRICS_DIR = Path(__file__).parent
WORKSPACE_ROOT = METRICS_DIR.parent

# Global state for cleanup
_original_ref: Optional[str] = None
_stash_created: bool = False


def _cleanup():
    """Cleanup handler - restore original branch and pop stash if needed."""
    global _original_ref, _stash_created

    if _original_ref:
        print(f"\nRestoring to {_original_ref}...")
        subprocess.run(
            ["git", "checkout", _original_ref, "--force", "--quiet"],
            capture_output=True,
            text=True,
            cwd=WORKSPACE_ROOT
        )

    if _stash_created:
        print("Restoring stashed changes...")
        subprocess.run(
            ["git", "stash", "pop", "--quiet"],
            capture_output=True,
            text=True,
            cwd=WORKSPACE_ROOT
        )


def _signal_handler(signum, frame):
    """Handle interrupt signals gracefully."""
    print("\n\nInterrupted! Cleaning up...")
    _cleanup()
    sys.exit(1)


def get_commits(n: int) -> list:
    """Get the last N commits in reverse chronological order."""
    result = subprocess.run(
        ["git", "log", f"-{n}", "--format=%H", "--reverse"],
        capture_output=True,
        text=True,
        cwd=WORKSPACE_ROOT
    )
    if result.returncode != 0:
        print(f"Error getting commits: {result.stderr}")
        return []
    return [c.strip() for c in result.stdout.strip().split("\n") if c.strip()]


def get_current_branch() -> str:
    """Get current branch or commit."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        capture_output=True,
        text=True,
        cwd=WORKSPACE_ROOT
    )
    branch = result.stdout.strip()
    if branch == "HEAD":
        # Detached HEAD, get commit hash
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            cwd=WORKSPACE_ROOT
        )
        return result.stdout.strip()
    return branch


def stash_changes() -> bool:
    """Stash any uncommitted changes. Returns True if stash was created."""
    # Check if there are changes to stash
    status_result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True,
        text=True,
        cwd=WORKSPACE_ROOT
    )

    if not status_result.stdout.strip():
        return False  # No changes to stash

    # Stash the changes
    result = subprocess.run(
        ["git", "stash", "push", "-m", "metrics_collection_auto_stash", "--quiet"],
        capture_output=True,
        text=True,
        cwd=WORKSPACE_ROOT
    )
    return result.returncode == 0


def checkout(ref: str) -> bool:
    """Checkout a commit or branch."""
    result = subprocess.run(
        ["git", "checkout", ref, "--force", "--quiet"],
        capture_output=True,
        text=True,
        cwd=WORKSPACE_ROOT
    )
    return result.returncode == 0


def collect_metrics(with_coverage: bool = False) -> bool:
    """Run the metrics collection script."""
    cmd = [sys.executable, str(METRICS_DIR / "collect_metrics.py")]
    if with_coverage:
        cmd.append("--with-coverage")
    result = subprocess.run(cmd, cwd=WORKSPACE_ROOT)
    return result.returncode == 0


def main():
    global _original_ref, _stash_created

    parser = argparse.ArgumentParser(description="Collect metrics for historical commits")
    parser.add_argument(
        "--commits", "-n",
        type=int,
        default=10,
        help="Number of commits to analyze (default: 10)"
    )
    parser.add_argument(
        "--with-coverage",
        action="store_true",
        help="Include code coverage analysis (slower)"
    )
    args = parser.parse_args()

    # Set up signal handlers for graceful cleanup
    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)
    atexit.register(_cleanup)

    print(f"Collecting metrics for the last {args.commits} commits...")
    print(f"Workspace: {WORKSPACE_ROOT}")
    print(f"Coverage: {'ENABLED' if args.with_coverage else 'DISABLED'}")

    # Save current position
    _original_ref = get_current_branch()
    print(f"Current position: {_original_ref}")

    # Stash any uncommitted changes
    _stash_created = stash_changes()
    if _stash_created:
        print("Stashed uncommitted changes (will restore on completion)")

    # Get commits (oldest first)
    commits = get_commits(args.commits)
    if not commits:
        print("No commits found!")
        return 1

    if len(commits) < args.commits:
        print(f"Warning: Only {len(commits)} commits available (requested {args.commits})")

    print(f"Found {len(commits)} commits to analyze")
    print()

    success_count = 0
    try:
        for i, commit in enumerate(commits, 1):
            short_commit = commit[:7]
            print(f"[{i}/{len(commits)}] Analyzing commit {short_commit}...")

            if not checkout(commit):
                print(f"  Failed to checkout {short_commit}")
                continue

            if collect_metrics(with_coverage=args.with_coverage):
                success_count += 1
                print(f"  Metrics collected for {short_commit}")
            else:
                print(f"  Failed to collect metrics for {short_commit}")

            print()
    finally:
        # Restore original position (also handled by atexit, but be explicit)
        print(f"Restoring to {_original_ref}...")
        checkout(_original_ref)

        if _stash_created:
            print("Restoring stashed changes...")
            subprocess.run(
                ["git", "stash", "pop", "--quiet"],
                capture_output=True,
                text=True,
                cwd=WORKSPACE_ROOT
            )
            _stash_created = False  # Prevent double-pop in atexit

        _original_ref = None  # Prevent atexit from running checkout again

    print(f"\nDone! Collected metrics for {success_count}/{len(commits)} commits.")
    print(f"Results saved to: {METRICS_DIR / 'data' / 'crate_metrics.csv'}")

    return 0 if success_count > 0 else 1


if __name__ == "__main__":
    sys.exit(main())
