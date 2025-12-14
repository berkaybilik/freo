## freo (For the Reviewers’ Eyes Only)

`freo` helps you leave **temporary, reviewer-only comments** in a pull request branch and then **remove them automatically** (typically when a PR is approved) so they never land on your default branch.

It works by **stripping comment text** that contains a keyword (default: `FREO`) across the files you changed in the PR.

### What it does

- **Removes** `FREO` comments from code you changed (added/modified files in the PR).
- **Keeps everything else** unchanged.
- **Skips unknown file types** (unless you configure a comment token for them).
- **Runs as a GitHub Action** (composite action that downloads a prebuilt `freo` binary from this repo’s Releases, verifies the artifact’s attestation, then executes it).

### What counts as a “FREO comment”?

`freo` looks for a **single-line comment token** for the file type (like `//`, `#`, `--`) and removes text matching:

- optional whitespace
- the comment token
- optional whitespace
- the keyword (case-insensitive, as a whole word)
- optional `:`
- the rest of that line

Examples (default keyword `FREO`):

```text
let x = 1; // FREO: remove debug before merge
# freo this is temporary
SELECT * FROM users; -- FREO don’t commit this query
```

If a line becomes empty after removing the comment, the whole line is removed.

### Quick start (recommended)

Create a workflow that runs when a review is submitted, and only proceeds on approval. Then commit/push the cleanup back to the PR branch.

```yaml
name: freo cleanup

on:
  pull_request_review:
    types:
      - submitted

concurrency:
  group: freo-${{ github.event.pull_request.head.repo.full_name }}-${{ github.event.pull_request.head.ref }}
  cancel-in-progress: true

jobs:
  run-freo:
    if: github.event.review.state == 'approved'
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    steps:
      - name: Checkout PR branch
        uses: actions/checkout@v4
        with:
          repository: ${{ github.event.pull_request.head.repo.full_name }}
          ref: ${{ github.event.pull_request.head.ref }}
          fetch-depth: 0
          persist-credentials: true

      - name: Run freo
        uses: berkaybilik/freo@v0.0.12

      - name: Detect freo changes
        id: git_status
        run: |
          if [[ -z "$(git status --porcelain)" ]]; then
            echo "changed=false" >> "$GITHUB_OUTPUT"
          else
            echo "changed=true" >> "$GITHUB_OUTPUT"
          fi

      - name: Configure git user
        if: steps.git_status.outputs.changed == 'true' && github.event.pull_request.head.repo.full_name == github.repository
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

      - name: Commit freo changes
        if: steps.git_status.outputs.changed == 'true' && github.event.pull_request.head.repo.full_name == github.repository
        run: |
          git add -A
          git commit -m "chore: apply freo cleanup"

      - name: Push freo changes
        if: steps.git_status.outputs.changed == 'true' && github.event.pull_request.head.repo.full_name == github.repository
        run: |
          git push origin HEAD:"${{ github.event.pull_request.head.ref }}"

      - name: Skip push for forked PRs
        if: steps.git_status.outputs.changed == 'true' && github.event.pull_request.head.repo.full_name != github.repository
        run: |
          echo "Detected forked pull request; freo changes must be pushed manually."
```

### Action inputs

- **Important**: this action is meant to be used via a **release tag** (for example `uses: berkaybilik/freo@v1.0.0`). It downloads the `freo` binary from the release that matches the tag you reference.

- **`config`** (optional): Path to a JSON config file.
  - Example: `config: .github/freo.json`

Example:

```yaml
- name: Run freo with custom config
  uses: berkaybilik/freo@v0.0.12
  with:
    config: .github/freo.json
```

### Configuration (`freo.json`)

By default, `freo` looks for `freo.json` in the repository root (or `GITHUB_WORKSPACE` on Actions). If the config file is missing, it falls back to defaults.

Supported keys:

- **`keyword`**: The keyword that marks removable comments (default: `FREO`).
- **`comment_map`**: Map of file extensions to single-line comment tokens. Keys are normalized (leading `.` ignored; case-insensitive).

Example:

```json
{
  "keyword": "FREO",
  "comment_map": {
    "txt": "//",
    ".tf": "#",
    "lua": "--"
  }
}
```

### Supported file types (defaults)

Out of the box, `freo` knows these extensions:

- **`//`**: `c`, `cc`, `cpp`, `cs`, `go`, `h`, `hpp`, `java`, `js`, `jsx`, `kt`, `rs`, `swift`, `ts`, `tsx`
- **`#`**: `py`, `rb`, `sh`, `bash`, `toml`, `yaml`, `yml`, `ini`
- **`--`**: `sql`

Anything else is skipped unless you add it to `comment_map`.

### Use cases (why temporary comments help)

- **AI reviewer hints**: “FREO: This looks scary but is safe because …” to avoid repeated false positives.
- **Review-only context**: “FREO: This is a mechanical refactor; focus on behavior changes in X.”
- **Backwards compatibility notes**: “FREO: Safe to remove `legacyField` from the JSON response — frontend no longer reads it (verified in commit abc123 ).”
- **Follow-ups that should become issues**: “FREO: open ticket to rework this later” without leaving TODO noise in main.
- **Local debugging breadcrumbs**: “FREO: remove this log / guard before merge.”
- **Security/PII reminders**: “FREO: redact sample data before merge” without committing sensitive context.
- **Temporary reviewer instructions for generated code**: “FREO: This file is autogenerated; ignore formatting churn.”

### Notes & limitations

- **Single-line only**: `freo` removes single-line comment matches; it is not a block-comment parser.
- **Only changed files**: the action runs on **added/modified files** in the PR compared to the base branch.
- **Needs a PR context**: the action determines the base branch from PR metadata; it’s intended for `pull_request`, `pull_request_review`, or `pull_request_target`.
- **Fork PRs**: you typically can’t push back to fork branches with the default token; the example workflow detects forks and skips pushing.

### Using FREO with AI editors (Cursor, Copilot, Codex, etc.)

If you use an AI assistant while coding, `FREO` comments are a simple “escape hatch” for temporary guidance that helps reviewers (human or AI) but shouldn’t ship:

- Add a project rule like: “You may add `FREO:` comments for reviewer-only context; these will be removed on PR approval.”
- Use them for things like: rationale, tricky edge-cases, suggested review focus areas, or short-lived debugging notes.

### Run locally (CLI)

If you want to test before wiring the Action, you can run the CLI in this repo:

```bash
cargo run -- -f path/to/file1.rs path/to/file2.py
```

With a config file:

```bash
cargo run -- -c freo.json -f path/to/file1.rs
```
