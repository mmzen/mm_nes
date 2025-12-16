#!/usr/bin/env python3
# Authorship: Human 0% | Claude 100%
"""
Code Quality Metrics Collector for Rust Workspace

Collects metrics from rust-code-analysis-cli and optionally cargo-tarpaulin,
aggregates to crate level, and appends to historical CSV.

Usage:
    python3 collect_metrics.py                  # Without coverage
    python3 collect_metrics.py --with-coverage  # With coverage (slower)
"""

import argparse
import csv
import glob
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional

# Platform-specific file locking
# Note: File locking is primarily for CI on Linux. Windows locking is best-effort.
if sys.platform == "win32":
    import msvcrt
    _lock_pos = 0  # Track lock position
    def lock_file(f):
        """Lock file on Windows (best-effort, may fail gracefully)."""
        global _lock_pos
        try:
            _lock_pos = f.tell()
            f.seek(0)
            msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
        except (OSError, IOError):
            pass  # Lock failed, continue anyway (single-process case)
    def unlock_file(f):
        """Unlock file on Windows."""
        try:
            f.seek(0)
            msvcrt.locking(f.fileno(), msvcrt.LK_UNLCK, 1)
        except (OSError, IOError):
            pass  # Unlock failed, continue anyway
else:
    import fcntl
    def lock_file(f):
        fcntl.flock(f.fileno(), fcntl.LOCK_EX)
    def unlock_file(f):
        fcntl.flock(f.fileno(), fcntl.LOCK_UN)


@dataclass
class CrateMetrics:
    """Aggregated metrics for a crate."""
    name: str
    loc_total: int = 0
    loc_code: int = 0
    loc_comments: int = 0
    loc_blanks: int = 0
    functions_count: int = 0
    cyclomatic_values: List[float] = field(default_factory=list)
    cognitive_values: List[float] = field(default_factory=list)
    halstead_difficulty_values: List[float] = field(default_factory=list)
    coverage_percent: float = 0.0
    covered_lines: int = 0
    total_lines: int = 0

    @property
    def avg_cyclomatic(self) -> float:
        return sum(self.cyclomatic_values) / len(self.cyclomatic_values) if self.cyclomatic_values else 0.0

    @property
    def max_cyclomatic(self) -> float:
        return max(self.cyclomatic_values) if self.cyclomatic_values else 0.0

    @property
    def avg_cognitive(self) -> float:
        return sum(self.cognitive_values) / len(self.cognitive_values) if self.cognitive_values else 0.0

    @property
    def max_cognitive(self) -> float:
        return max(self.cognitive_values) if self.cognitive_values else 0.0

    @property
    def avg_halstead_difficulty(self) -> float:
        return sum(self.halstead_difficulty_values) / len(self.halstead_difficulty_values) if self.halstead_difficulty_values else 0.0

    @property
    def max_halstead_difficulty(self) -> float:
        return max(self.halstead_difficulty_values) if self.halstead_difficulty_values else 0.0


# Configuration
CRATES = ["mmnes_core", "mmnes_frontend", "mmretrodb"]
METRICS_DIR = Path(__file__).parent
DATA_DIR = METRICS_DIR / "data"
OUTPUT_CSV = DATA_DIR / "crate_metrics.csv"


def file_belongs_to_crate(file_path: str, crate_name: str) -> bool:
    """Check if a file path belongs to a specific crate.

    Handles both forward and backward slashes for cross-platform compatibility.
    """
    # Normalize path separators
    normalized = file_path.replace("\\", "/")

    # Check various patterns
    return (
        f"/{crate_name}/" in normalized or          # Middle of path
        f"/{crate_name}/src" in normalized or       # Explicit src directory
        normalized.startswith(f"{crate_name}/") or  # Relative path from workspace
        normalized.endswith(f"/{crate_name}")       # Edge case: path ends with crate
    )

# CSV column definitions
CSV_FIELDNAMES = [
    "timestamp", "commit_date", "commit_hash", "commit_short", "branch", "crate_name",
    "loc_total", "loc_code", "loc_comments", "loc_blanks",
    "functions_count", "avg_cyclomatic", "max_cyclomatic",
    "avg_cognitive", "max_cognitive", "avg_halstead_diff", "max_halstead_diff",
    "coverage_percent", "covered_lines", "total_lines"
]


def get_git_info() -> Dict[str, str]:
    """Get current git commit info with validation."""
    def run_git(args: List[str], default: str = "unknown") -> str:
        try:
            result = subprocess.run(
                ["git"] + args,
                capture_output=True, text=True, check=True
            )
            return result.stdout.strip()
        except (subprocess.CalledProcessError, FileNotFoundError):
            return default

    commit_hash = run_git(["rev-parse", "HEAD"])

    # Fallback to CI environment variables
    if not commit_hash or commit_hash == "unknown":
        commit_hash = os.environ.get("BITBUCKET_COMMIT", "")
        if not commit_hash:
            commit_hash = os.environ.get("GITHUB_SHA", "unknown")

    # Validate commit hash format (should be 40 hex characters)
    if commit_hash != "unknown" and len(commit_hash) >= 7:
        # Basic validation: should be hex characters
        try:
            int(commit_hash[:7], 16)
            commit_short = commit_hash[:7]
        except ValueError:
            commit_short = "unknown"
    else:
        commit_short = "unknown"

    branch = run_git(["rev-parse", "--abbrev-ref", "HEAD"])
    if not branch or branch == "unknown":
        branch = os.environ.get("BITBUCKET_BRANCH", "")
        if not branch:
            branch = os.environ.get("GITHUB_REF_NAME", "unknown")

    # Get commit date in YYYY-MM-DD format
    commit_date = run_git(["log", "-1", "--format=%cs"])  # %cs = short date YYYY-MM-DD

    # Validate date format (YYYY-MM-DD)
    if commit_date and commit_date != "unknown":
        import re
        if not re.match(r"^\d{4}-\d{2}-\d{2}$", commit_date):
            commit_date = "unknown"

    return {
        "commit_hash": commit_hash if commit_hash else "unknown",
        "commit_short": commit_short,
        "commit_date": commit_date if commit_date else "unknown",
        "branch": branch if branch else "unknown",
    }


def run_rust_code_analysis(crate_path: Path) -> Optional[List[dict]]:
    """Run rust-code-analysis-cli on a crate and return list of JSON results.

    Note: rust-code-analysis-cli outputs NDJSON (one JSON object per line).
    """
    src_path = crate_path / "src"
    if not src_path.exists():
        print(f"  Warning: No src/ directory found in {crate_path}")
        return None

    cmd = [
        "rust-code-analysis-cli",
        "--metrics",
        "--output-format", "json",
        "-p", str(src_path)
    ]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        print(f"  Warning: rust-code-analysis-cli timed out for {crate_path}")
        return None
    except FileNotFoundError:
        print("  Error: rust-code-analysis-cli not found. Install it first.")
        return None

    if result.returncode != 0:
        print(f"  Warning: rust-code-analysis-cli failed: {result.stderr[:200]}")
        return None

    # Parse NDJSON (newline-delimited JSON) - one JSON object per line
    results = []
    for line in result.stdout.strip().split('\n'):
        line = line.strip()
        if not line:
            continue
        try:
            results.append(json.loads(line))
        except json.JSONDecodeError as e:
            print(f"  Warning: Failed to parse JSON line: {e}")
            continue

    return results if results else None


def parse_rca_metrics(rca_data: Optional[List[dict]], crate_name: str) -> CrateMetrics:
    """Parse rust-code-analysis JSON output (list of file results) into CrateMetrics."""
    metrics = CrateMetrics(name=crate_name)

    if rca_data is None:
        return metrics

    def process_space(space: dict):
        """Recursively process a space (file/function/etc)."""
        kind = space.get("kind", "")
        space_metrics = space.get("metrics", {})

        # Accumulate LOC metrics at file level (unit = file)
        if kind == "unit":
            loc_info = space_metrics.get("loc", {})
            metrics.loc_code += int(loc_info.get("sloc", 0))
            metrics.loc_comments += int(loc_info.get("cloc", 0))
            metrics.loc_blanks += int(loc_info.get("blank", 0))
            # lloc = logical lines of code (approximation of total)
            lloc = loc_info.get("lloc", 0)
            if lloc:
                metrics.loc_total += int(lloc)
            else:
                # Fallback: sum of sloc + cloc + blank
                metrics.loc_total += (
                    int(loc_info.get("sloc", 0)) +
                    int(loc_info.get("cloc", 0)) +
                    int(loc_info.get("blank", 0))
                )

        # Extract function-level metrics
        if kind == "function":
            # Cyclomatic complexity (minimum is 1 for any function)
            cyclomatic = space_metrics.get("cyclomatic", {}).get("sum", 0)
            if cyclomatic is not None and cyclomatic > 0:
                metrics.cyclomatic_values.append(float(cyclomatic))

            # Cognitive complexity (0 is valid - simple linear functions)
            cognitive = space_metrics.get("cognitive", {}).get("sum", 0)
            if cognitive is not None and cognitive >= 0:
                metrics.cognitive_values.append(float(cognitive))

            # Halstead difficulty (0 means no operators/operands - skip)
            halstead = space_metrics.get("halstead", {})
            difficulty = halstead.get("difficulty", 0)
            if difficulty is not None and difficulty > 0:
                metrics.halstead_difficulty_values.append(float(difficulty))

            metrics.functions_count += 1

        # Process nested spaces recursively
        for subspace in space.get("spaces", []):
            process_space(subspace)

    # Process all spaces in the analysis result
    if isinstance(rca_data, dict):
        process_space(rca_data)
    elif isinstance(rca_data, list):
        for item in rca_data:
            if isinstance(item, dict):
                process_space(item)

    return metrics


def run_tarpaulin(workspace_root: Path) -> Dict[str, Dict[str, float]]:
    """Run cargo-tarpaulin and return per-crate coverage."""
    coverage_file = workspace_root / "tarpaulin-report.json"

    cmd = [
        "cargo", "tarpaulin",
        "--workspace",
        "--out", "Json",
        "--output-dir", str(workspace_root),
        "--skip-clean",
        "--timeout", "300"
    ]

    print("  Running cargo-tarpaulin (this may take a few minutes)...")

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=workspace_root,
            timeout=400
        )
    except subprocess.TimeoutExpired:
        print("  Warning: cargo-tarpaulin timed out")
        return {}
    except FileNotFoundError:
        print("  Warning: cargo-tarpaulin not found")
        return {}

    if result.returncode != 0:
        print(f"  Warning: cargo-tarpaulin failed: {result.stderr[:200]}")
        return {}

    if not coverage_file.exists():
        print("  Warning: Tarpaulin report not found")
        return {}

    try:
        with open(coverage_file) as f:
            data = json.load(f)
    except (json.JSONDecodeError, IOError) as e:
        print(f"  Warning: Failed to read tarpaulin report: {e}")
        return {}

    # Aggregate coverage by crate
    crate_coverage: Dict[str, Dict[str, float]] = {
        crate: {"covered": 0, "total": 0} for crate in CRATES
    }

    for file_info in data.get("files", []):
        file_path = file_info.get("path", "")

        # Determine which crate this file belongs to
        for crate in CRATES:
            if file_belongs_to_crate(file_path, crate):
                traces = file_info.get("traces", [])
                covered = sum(1 for t in traces if t.get("stats", {}).get("Line", 0) > 0)
                total = len(traces)
                crate_coverage[crate]["covered"] += covered
                crate_coverage[crate]["total"] += total
                break

    return crate_coverage


def _find_sdl2_lib_path() -> Optional[Path]:
    """Find SDL2 library path on Windows using glob patterns.

    Searches common installation locations for any SDL2 version.
    """
    if sys.platform != "win32":
        return None

    # Check SDL2_LIB_DIR environment variable first
    env_path = os.environ.get("SDL2_LIB_DIR", "")
    if env_path and Path(env_path).exists():
        sdl2_lib = Path(env_path) / "SDL2.lib"
        if sdl2_lib.exists():
            return Path(env_path)

    # Search patterns for common SDL2 locations
    search_patterns = [
        os.path.join(os.environ.get("TEMP", ""), "SDL2-*", "lib", "x64"),
        os.path.join(os.environ.get("LOCALAPPDATA", ""), "SDL2-*", "lib", "x64"),
        "C:/SDL2-*/lib/x64",
        "C:/SDL2/lib/x64",
        "C:/Libraries/SDL2-*/lib/x64",
    ]

    for pattern in search_patterns:
        matches = glob.glob(pattern)
        for match in matches:
            sdl2_lib = Path(match) / "SDL2.lib"
            if sdl2_lib.exists():
                return Path(match)

    return None


def run_llvm_cov(workspace_root: Path) -> Dict[str, Dict[str, float]]:
    """Run cargo-llvm-cov and return per-crate coverage.

    Works on Windows, macOS, and Linux (cross-platform alternative to tarpaulin).
    """
    coverage_file = workspace_root / "llvm-cov-report.json"

    cmd = [
        "cargo", "llvm-cov",
        "--workspace",
        "--json",
        "--output-path", str(coverage_file)
    ]

    # Set up environment with SDL2 library path for Windows
    env = os.environ.copy()
    if sys.platform == "win32":
        sdl2_path = _find_sdl2_lib_path()
        if sdl2_path:
            rustflags = env.get("RUSTFLAGS", "")
            env["RUSTFLAGS"] = f"{rustflags} -L {sdl2_path}".strip()
            print(f"  Using SDL2 from: {sdl2_path}")

    print("  Running cargo-llvm-cov (this may take a few minutes)...")

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=workspace_root,
            timeout=600,
            env=env
        )
    except subprocess.TimeoutExpired:
        print("  Warning: cargo-llvm-cov timed out")
        return {}
    except FileNotFoundError:
        print("  Warning: cargo-llvm-cov not found. Install with: cargo install cargo-llvm-cov")
        return {}

    if result.returncode != 0:
        stderr_preview = result.stderr[:300] if result.stderr else "no error output"
        print(f"  Warning: cargo-llvm-cov failed: {stderr_preview}")
        return {}

    if not coverage_file.exists():
        print("  Warning: LLVM coverage report not found")
        return {}

    try:
        with open(coverage_file) as f:
            data = json.load(f)
    except (json.JSONDecodeError, IOError) as e:
        print(f"  Warning: Failed to read llvm-cov report: {e}")
        return {}

    # Aggregate coverage by crate from llvm-cov JSON format
    crate_coverage: Dict[str, Dict[str, float]] = {
        crate: {"covered": 0, "total": 0} for crate in CRATES
    }

    # llvm-cov JSON structure: data[0].files[].filename, data[0].files[].summary.lines
    for export_data in data.get("data", []):
        for file_info in export_data.get("files", []):
            file_path = file_info.get("filename", "")

            # Determine which crate this file belongs to
            for crate in CRATES:
                if file_belongs_to_crate(file_path, crate):
                    summary = file_info.get("summary", {})
                    lines = summary.get("lines", {})
                    covered = lines.get("covered", 0)
                    total = lines.get("count", 0)
                    crate_coverage[crate]["covered"] += covered
                    crate_coverage[crate]["total"] += total
                    break

    return crate_coverage


def collect_all_metrics(workspace_root: Path, with_coverage: bool = False) -> List[CrateMetrics]:
    """Collect all metrics for all crates."""
    all_metrics = []

    # Run coverage tool once for the entire workspace (if requested)
    coverage_data = {}
    if with_coverage:
        print("Collecting coverage data...")
        # Try cargo-llvm-cov first (cross-platform)
        coverage_data = run_llvm_cov(workspace_root)

        # Fallback to tarpaulin on Linux if llvm-cov failed
        if not coverage_data or all(v["total"] == 0 for v in coverage_data.values()):
            if sys.platform.startswith("linux"):
                print("  Falling back to cargo-tarpaulin...")
                coverage_data = run_tarpaulin(workspace_root)

    for crate_name in CRATES:
        crate_path = workspace_root / crate_name

        if not crate_path.exists():
            print(f"Warning: Crate {crate_name} not found at {crate_path}")
            continue

        print(f"Analyzing {crate_name}...")

        # Run rust-code-analysis
        rca_data = run_rust_code_analysis(crate_path)
        metrics = parse_rca_metrics(rca_data, crate_name)

        # Add coverage data if available
        if crate_name in coverage_data:
            cov = coverage_data[crate_name]
            metrics.covered_lines = int(cov["covered"])
            metrics.total_lines = int(cov["total"])
            metrics.coverage_percent = (
                (cov["covered"] / cov["total"] * 100) if cov["total"] > 0 else 0.0
            )

        all_metrics.append(metrics)

    return all_metrics


def write_csv(metrics_list: List[CrateMetrics], git_info: Dict[str, str]):
    """Append metrics to CSV file with file locking to prevent race conditions."""
    DATA_DIR.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now(timezone.utc).isoformat()

    # Use r+ mode if file exists, otherwise create with w mode first
    if not OUTPUT_CSV.exists():
        # Create file with header (no lock needed for new file)
        with open(OUTPUT_CSV, "w", newline="") as f:
            writer = csv.DictWriter(f, fieldnames=CSV_FIELDNAMES)
            writer.writeheader()

    # Now append with lock
    with open(OUTPUT_CSV, "a", newline="") as f:
        try:
            lock_file(f)
            writer = csv.DictWriter(f, fieldnames=CSV_FIELDNAMES)

            for m in metrics_list:
                writer.writerow({
                    "timestamp": timestamp,
                    "commit_date": git_info["commit_date"],
                    "commit_hash": git_info["commit_hash"],
                    "commit_short": git_info["commit_short"],
                    "branch": git_info["branch"],
                    "crate_name": m.name,
                    "loc_total": m.loc_total,
                    "loc_code": m.loc_code,
                    "loc_comments": m.loc_comments,
                    "loc_blanks": m.loc_blanks,
                    "functions_count": m.functions_count,
                    "avg_cyclomatic": round(m.avg_cyclomatic, 2),
                    "max_cyclomatic": round(m.max_cyclomatic, 2),
                    "avg_cognitive": round(m.avg_cognitive, 2),
                    "max_cognitive": round(m.max_cognitive, 2),
                    "avg_halstead_diff": round(m.avg_halstead_difficulty, 2),
                    "max_halstead_diff": round(m.max_halstead_difficulty, 2),
                    "coverage_percent": round(m.coverage_percent, 2),
                    "covered_lines": m.covered_lines,
                    "total_lines": m.total_lines,
                })
        finally:
            unlock_file(f)

    print(f"\nMetrics written to {OUTPUT_CSV}")


def print_summary(metrics_list: List[CrateMetrics]):
    """Print a summary table to stdout."""
    print("\n" + "=" * 70)
    print("CODE QUALITY METRICS SUMMARY")
    print("=" * 70)

    for m in metrics_list:
        print(f"\n{m.name}:")
        print(f"  LOC (code/comments/blank): {m.loc_code}/{m.loc_comments}/{m.loc_blanks}")
        print(f"  Functions: {m.functions_count}")
        print(f"  Cyclomatic (avg/max): {m.avg_cyclomatic:.1f}/{m.max_cyclomatic:.1f}")
        print(f"  Cognitive (avg/max): {m.avg_cognitive:.1f}/{m.max_cognitive:.1f}")
        print(f"  Halstead Difficulty (avg/max): {m.avg_halstead_difficulty:.1f}/{m.max_halstead_difficulty:.1f}")
        if m.total_lines > 0:
            print(f"  Coverage: {m.coverage_percent:.1f}% ({m.covered_lines}/{m.total_lines} lines)")
        else:
            print(f"  Coverage: N/A (not collected)")

    print("\n" + "=" * 70)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Collect code quality metrics for Rust workspace")
    parser.add_argument(
        "--with-coverage",
        action="store_true",
        help="Include code coverage analysis (slower)"
    )
    parser.add_argument(
        "--workspace",
        type=Path,
        default=None,
        help="Path to workspace root (default: parent of metrics directory)"
    )
    args = parser.parse_args()

    # Determine workspace root
    if args.workspace:
        workspace_root = args.workspace.resolve()
    else:
        workspace_root = METRICS_DIR.parent

    print(f"Collecting metrics for workspace: {workspace_root}")

    # Verify workspace
    cargo_toml = workspace_root / "Cargo.toml"
    if not cargo_toml.exists():
        print(f"Error: No Cargo.toml found at {workspace_root}")
        sys.exit(1)

    git_info = get_git_info()
    print(f"Commit: {git_info['commit_short']} on branch {git_info['branch']}")

    if args.with_coverage:
        print("Coverage collection: ENABLED")
    else:
        print("Coverage collection: DISABLED (use --with-coverage to enable)")

    metrics_list = collect_all_metrics(workspace_root, with_coverage=args.with_coverage)

    if not metrics_list:
        print("Error: No metrics collected!")
        sys.exit(1)

    write_csv(metrics_list, git_info)
    print_summary(metrics_list)

    print("\nMetrics collection complete!")


if __name__ == "__main__":
    main()
