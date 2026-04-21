# Shuozeli -- Git Superproject

This is the Shuoze Li Git superproject containing multiple Rust projects as git submodules.

## Structure

```
Shuozeli/
├── .github/
│   └── workflows/           # CI workflows
├── .claude/
│   └── rules/shared/        # Shared Claude Code rules (git submodule)
├── docs/                    # Per-project documentation
├── openworkspace/           # openworkspace platform
├── pidx/                    # Personal index / project tracker
└── [submodules]/            # Individual project repos
```

## Submodules (via git submodules)

Each project lives in its own repository and is linked here as a submodule in `docs/`.

To initialize all submodules after a fresh clone:
```bash
git clone --recurse-submodules git@github.com:Shuozeli/Shuozeli.git
```

To pull latest for a specific submodule:
```bash
cd docs/protobuf-rs && git pull origin main
```

## Shared Claude Rules

The `.claude/rules/shared/` is a git submodule pointing to `https://github.com/shuozeli/claude-rules`.

To update shared rules in any project that has the submodule:
```bash
git submodule update --remote --merge
git add .claude/rules/shared
git commit -m "Update shared claude-rules"
```

## Pre-commit Hooks

This project uses pre-commit. Install with:
```bash
pip install pre-commit
pre-commit install
```

## CI

GitHub Actions runs on every push. See `.github/workflows/`.