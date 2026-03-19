# pidx Design

Last updated: 2026-03-19

## Problem

Managing 13+ active repos requires a single dashboard for health, velocity, and progress. Manual tracking does not scale. LLMs can provide richer analysis but need structured input.

## Solution

A hybrid CLI tool where:
- pidx handles structured data (GitHub API -> SQLite -> formatted output)
- LLMs provide qualitative analysis (risks, recommendations, summaries)
- The two are connected via a docs pipeline (export/ingest)

## Key Design Choices

### Allowlist over Auto-Discovery
Repos are explicitly listed in config. This prevents noise from forks, archived repos, or experimental projects.

### Health Score Formula
Composite of three signals:
- **Recency** (40%): How recently was code pushed? 100 if pushed within 3 days, linear decay to 0 at 90 days.
- **Velocity** (40%): Commit count over 30 days. 10+ commits scores 100.
- **Issues** (20%): 100 if 0 open issues, -10 per open issue, minimum 0.

Health labels derived from the composite score:
- **Active** (>=80), **Healthy** (>=60), **Moderate** (>=40), **Stale** (>=20), **Dormant** (<20).

### Docs as LLM Interface
Rather than calling LLM APIs directly, pidx exports structured markdown that any LLM can process. This keeps pidx simple and LLM-agnostic.

### SQLite for Persistence
Local, zero-config, fast. All access wrapped in transactions for consistency.
