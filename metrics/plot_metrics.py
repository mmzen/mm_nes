#!/usr/bin/env python3
# Authorship: Human 0% | Claude 100%
"""
Code Quality Metrics Plotter

Generates trend charts from the metrics CSV file.

Usage:
    python3 plot_metrics.py                    # Generate all plots
    python3 plot_metrics.py --output-dir ./    # Specify output directory
    python3 plot_metrics.py --last 50          # Only plot last 50 commits
"""

import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd


# Configuration
METRICS_DIR = Path(__file__).parent
DATA_DIR = METRICS_DIR / "data"
INPUT_CSV = DATA_DIR / "crate_metrics.csv"

# Color palette for crates
CRATE_COLORS = {
    "mmnes_core": "#1f77b4",      # Blue
    "mmnes_frontend": "#ff7f0e",  # Orange
    "mmretrodb": "#2ca02c",       # Green
}


def load_metrics(csv_path: Path, last_n: int = 0) -> pd.DataFrame:
    """Load metrics from CSV file."""
    if not csv_path.exists():
        raise FileNotFoundError(f"Metrics file not found: {csv_path}")

    df = pd.read_csv(csv_path)
    df["timestamp"] = pd.to_datetime(df["timestamp"])

    # Parse commit_date if available, otherwise use timestamp
    if "commit_date" in df.columns:
        df["commit_date"] = pd.to_datetime(df["commit_date"])
        df = df.sort_values("commit_date")
    else:
        # Fallback: use timestamp
        df["commit_date"] = df["timestamp"]
        df = df.sort_values("timestamp")

    # If last_n specified, get only the last N unique commits
    if last_n > 0:
        unique_commits = df["commit_hash"].unique()
        if len(unique_commits) > last_n:
            commits_to_keep = unique_commits[-last_n:]
            df = df[df["commit_hash"].isin(commits_to_keep)]

    return df


def plot_loc_trends(df: pd.DataFrame, output_path: Path):
    """Plot lines of code trends."""
    fig, ax = plt.subplots(figsize=(12, 6))

    for crate_name in df["crate_name"].unique():
        crate_df = df[df["crate_name"] == crate_name].copy()
        color = CRATE_COLORS.get(crate_name, "#333333")
        ax.plot(
            crate_df["commit_date"],
            crate_df["loc_code"],
            label=crate_name,
            marker="o",
            markersize=4,
            color=color,
            linewidth=1.5
        )

    ax.set_title("Lines of Code Over Time", fontsize=14, fontweight="bold")
    ax.set_xlabel("Commit Date")
    ax.set_ylabel("Lines of Code (SLOC)")
    ax.legend(loc="upper left")
    ax.grid(True, alpha=0.3)
    ax.tick_params(axis="x", rotation=45)
    ax.xaxis.set_major_formatter(plt.matplotlib.dates.DateFormatter('%Y-%m-%d'))

    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def plot_complexity_trends(df: pd.DataFrame, output_path: Path):
    """Plot complexity metrics trends."""
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))

    metrics = [
        ("avg_cyclomatic", "Avg Cyclomatic Complexity", axes[0, 0]),
        ("max_cyclomatic", "Max Cyclomatic Complexity", axes[0, 1]),
        ("avg_cognitive", "Avg Cognitive Complexity", axes[1, 0]),
        ("max_cognitive", "Max Cognitive Complexity", axes[1, 1]),
    ]

    for metric_col, title, ax in metrics:
        for crate_name in df["crate_name"].unique():
            crate_df = df[df["crate_name"] == crate_name].copy()
            color = CRATE_COLORS.get(crate_name, "#333333")
            ax.plot(
                crate_df["commit_date"],
                crate_df[metric_col],
                label=crate_name,
                marker="o",
                markersize=3,
                color=color,
                linewidth=1.5
            )
        ax.set_title(title, fontsize=11, fontweight="bold")
        ax.legend(loc="upper left", fontsize=8)
        ax.grid(True, alpha=0.3)
        ax.tick_params(axis="x", rotation=45, labelsize=8)
        ax.tick_params(axis="y", labelsize=8)

    plt.suptitle("Complexity Metrics Over Time", fontsize=14, fontweight="bold")
    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def plot_halstead_trends(df: pd.DataFrame, output_path: Path):
    """Plot Halstead difficulty trends."""
    fig, axes = plt.subplots(1, 2, figsize=(12, 5))

    metrics = [
        ("avg_halstead_diff", "Avg Halstead Difficulty", axes[0]),
        ("max_halstead_diff", "Max Halstead Difficulty", axes[1]),
    ]

    for metric_col, title, ax in metrics:
        for crate_name in df["crate_name"].unique():
            crate_df = df[df["crate_name"] == crate_name].copy()
            color = CRATE_COLORS.get(crate_name, "#333333")
            ax.plot(
                crate_df["commit_date"],
                crate_df[metric_col],
                label=crate_name,
                marker="o",
                markersize=4,
                color=color,
                linewidth=1.5
            )
        ax.set_title(title, fontsize=11, fontweight="bold")
        ax.legend(loc="upper left")
        ax.grid(True, alpha=0.3)
        ax.tick_params(axis="x", rotation=45)

    plt.suptitle("Halstead Difficulty Over Time", fontsize=14, fontweight="bold")
    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def plot_coverage_trends(df: pd.DataFrame, output_path: Path):
    """Plot coverage trends (if data available)."""
    # Check if we have any coverage data
    if df["coverage_percent"].sum() == 0:
        print("  Skipped coverage plot (no coverage data)")
        return

    fig, ax = plt.subplots(figsize=(12, 6))

    for crate_name in df["crate_name"].unique():
        crate_df = df[df["crate_name"] == crate_name].copy()
        color = CRATE_COLORS.get(crate_name, "#333333")
        # Always plot - 0% coverage is valid data that should be shown
        ax.plot(
            crate_df["commit_date"],
            crate_df["coverage_percent"],
            label=crate_name,
            marker="o",
            markersize=4,
            color=color,
            linewidth=1.5
        )

    ax.set_title("Code Coverage Over Time", fontsize=14, fontweight="bold")
    ax.set_xlabel("Date")
    ax.set_ylabel("Coverage (%)")
    ax.set_ylim(0, 100)
    ax.grid(True, alpha=0.3)
    ax.tick_params(axis="x", rotation=45)

    # Add horizontal line at 80% coverage target (before legend so it's included)
    ax.axhline(y=80, color="green", linestyle="--", alpha=0.5, label="80% target")
    ax.legend(loc="lower right")

    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def plot_functions_trends(df: pd.DataFrame, output_path: Path):
    """Plot function count trends."""
    fig, ax = plt.subplots(figsize=(12, 6))

    for crate_name in df["crate_name"].unique():
        crate_df = df[df["crate_name"] == crate_name].copy()
        color = CRATE_COLORS.get(crate_name, "#333333")
        ax.plot(
            crate_df["commit_date"],
            crate_df["functions_count"],
            label=crate_name,
            marker="o",
            markersize=4,
            color=color,
            linewidth=1.5
        )

    ax.set_title("Number of Functions Over Time", fontsize=14, fontweight="bold")
    ax.set_xlabel("Date")
    ax.set_ylabel("Function Count")
    ax.legend(loc="upper left")
    ax.grid(True, alpha=0.3)
    ax.tick_params(axis="x", rotation=45)

    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def plot_combined_dashboard(df: pd.DataFrame, output_path: Path):
    """Create a combined dashboard with all metrics."""
    fig, axes = plt.subplots(2, 3, figsize=(18, 10))

    plots = [
        ("loc_code", "Lines of Code", axes[0, 0]),
        ("functions_count", "Function Count", axes[0, 1]),
        ("avg_cyclomatic", "Avg Cyclomatic", axes[0, 2]),
        ("avg_cognitive", "Avg Cognitive", axes[1, 0]),
        ("coverage_percent", "Coverage %", axes[1, 1]),
        ("claude_percent", "Claude Authorship %", axes[1, 2]),
    ]

    for metric_col, title, ax in plots:
        for crate_name in df["crate_name"].unique():
            crate_df = df[df["crate_name"] == crate_name].copy()
            color = CRATE_COLORS.get(crate_name, "#333333")

            # Always plot - 0% coverage is valid data that should be shown
            ax.plot(
                crate_df["commit_date"],
                crate_df[metric_col],
                label=crate_name,
                marker="o",
                markersize=3,
                color=color,
                linewidth=1.2
            )

        ax.set_title(title, fontsize=10, fontweight="bold")
        ax.legend(loc="best", fontsize=7)
        ax.grid(True, alpha=0.3)
        ax.tick_params(axis="x", rotation=45, labelsize=7)
        ax.tick_params(axis="y", labelsize=8)

    plt.suptitle("Code Quality Metrics Dashboard", fontsize=16, fontweight="bold")
    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()
    print(f"  Saved: {output_path}")


def generate_latest_summary(df: pd.DataFrame, output_path: Path):
    """Generate a markdown summary of the latest metrics."""
    # Handle empty DataFrame
    if df.empty:
        with open(output_path, "w") as f:
            f.write("# Code Quality Metrics Summary\n\n")
            f.write(f"*Generated: {pd.Timestamp.now().strftime('%Y-%m-%d %H:%M:%S')}*\n\n")
            f.write("**No metrics data available.**\n")
        print(f"  Saved: {output_path} (no data)")
        return

    # Get latest metrics for each crate
    latest = df.groupby("crate_name").tail(1).copy()

    with open(output_path, "w") as f:
        f.write("# Code Quality Metrics Summary\n\n")
        f.write(f"*Generated: {pd.Timestamp.now().strftime('%Y-%m-%d %H:%M:%S')}*\n\n")

        commit = latest["commit_short"].iloc[0] if len(latest) > 0 else "unknown"
        branch = latest["branch"].iloc[0] if len(latest) > 0 else "unknown"
        f.write(f"**Latest commit:** `{commit}` on `{branch}`\n\n")

        f.write("## Per-Crate Metrics\n\n")
        f.write("| Crate | LOC | Functions | Avg Cyclo | Coverage | Human % | Claude % |\n")
        f.write("|-------|-----|-----------|-----------|----------|---------|----------|\n")

        for _, row in latest.iterrows():
            coverage_str = f"{row['coverage_percent']:.1f}%" if row['total_lines'] > 0 else "N/A"
            human_pct = row.get('human_percent', 100.0)
            claude_pct = row.get('claude_percent', 0.0)
            f.write(
                f"| {row['crate_name']} | {row['loc_code']} | {row['functions_count']} | "
                f"{row['avg_cyclomatic']:.1f} | {coverage_str} | "
                f"{human_pct:.1f}% | {claude_pct:.1f}% |\n"
            )

        f.write("\n## Definitions\n\n")
        f.write("- **LOC**: Lines of code (excluding comments and blanks)\n")
        f.write("- **Cyclomatic**: Number of independent paths through the code\n")
        f.write("- **Coverage**: Percentage of code lines covered by tests\n")
        f.write("- **Human/Claude %**: Authorship attribution based on file headers\n")

    print(f"  Saved: {output_path}")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Generate plots from code quality metrics")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DATA_DIR,
        help="Directory for output files (default: metrics/data/)"
    )
    parser.add_argument(
        "--csv",
        type=Path,
        default=INPUT_CSV,
        help="Input CSV file (default: metrics/data/crate_metrics.csv)"
    )
    parser.add_argument(
        "--last",
        type=int,
        default=0,
        help="Only plot last N commits (default: all)"
    )
    args = parser.parse_args()

    print(f"Loading metrics from: {args.csv}")
    try:
        df = load_metrics(args.csv, last_n=args.last)
    except FileNotFoundError as e:
        print(f"Error: {e}")
        print("Run collect_metrics.py first to generate data.")
        return 1

    print(f"Loaded {len(df)} data points across {df['commit_hash'].nunique()} commits")

    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    print("\nGenerating plots...")
    plot_loc_trends(df, output_dir / "loc_trends.png")
    plot_complexity_trends(df, output_dir / "complexity_trends.png")
    plot_halstead_trends(df, output_dir / "halstead_trends.png")
    plot_coverage_trends(df, output_dir / "coverage_trends.png")
    plot_functions_trends(df, output_dir / "functions_trends.png")
    plot_combined_dashboard(df, output_dir / "dashboard.png")

    print("\nGenerating summary...")
    generate_latest_summary(df, output_dir / "METRICS_SUMMARY.md")

    print("\nAll plots generated successfully!")
    return 0


if __name__ == "__main__":
    exit(main())
