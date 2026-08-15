## freo (For the Reviewers' Eyes Only)

`freo` lets you embed **PR-scoped reasoning directly in code** — as `// FREO: …` comments — so reviewers see the *why* behind each change inline, without that context ever landing on your main branch.

Comments are stripped automatically on PR approval, keeping your codebase clean.

<img width="558" height="387" alt="Screenshot 2025-12-15 at 23 39 10" src="https://github.com/user-attachments/assets/e4690e6c-b3ff-4430-a761-0d73a670dacf" />

### Why freo exists

When you build with an AI assistant like Claude Code, the reasoning behind implementation decisions lives in the chat. By the time the PR is opened, that context is gone — reviewers see the code, but not why it was written that way.

`freo` closes that gap: instruct your AI to annotate its output with `FREO` comments, and those explanations travel with the code to reviewers, then disappear on approval.

### Using freo with Claude Code

Add a rule to your project's `CLAUDE.md` so Claude automatically includes `FREO` comments when generating or modifying code:

```markdown
## Review comments

When you write or modify code, annotate non-obvious decisions with `FREO:` comments.
Use them to explain *why* — trade-offs made, alternatives ruled out, constraints that aren't
visible in the code itself, or anything a reviewer would otherwise have to ask about.
These comments are stripped automatically on PR approval and will never land on main.

Examples:
- `// FREO: using polling here instead of a webhook because the vendor API doesn't support webhooks yet`
- `// FREO: this cast is safe — the upstream type is wrong, see issue #42`
- `// FREO: skipping validation here because this path is only reachable from internal services`
```

With this rule in place, Claude will embed its reasoning in the code itself — reviewers get full context inline, and `freo` removes it all automatically when the PR is approved.

### What it does

- **Removes** `FREO` comments from code you changed (added/modified files in the PR).
- **Keeps everything else** unchanged.
- **Skips unknown file types** (unless you configure a comment token for them).
- **Runs as a GitHub Action** (composite action that ships prebuilt `freo` binaries in-repo under `dist/`, verifies their build provenance, then executes them — nothing is downloaded at run time).

### What counts as a "FREO comment"?

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
SELECT * FROM users; -- FREO don't commit this query
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
      attestations: read # the action verifies the binary's provenance
    steps:
      - name: Checkout PR branch
        uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 # v5.1.0
        with:
          repository: ${{ github.event.pull_request.head.repo.full_name }}
          ref: ${{ github.event.pull_request.head.ref }}
          fetch-depth: 0
          persist-credentials: true

      - name: Run freo
        # Copy the exact pin line from the release page you want.
        uses: berkaybilik/freo@<commit-sha> # v1.1.0

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

- **Pin by commit SHA.** A git tag can be repointed at any time by anyone with push access, so `@v1.1.0` is a promise that can be rewritten after you've reviewed it. A commit SHA names content, not a label — it cannot be moved. Every release page carries the exact line to copy:

  ```yaml
  - uses: berkaybilik/freo@<commit-sha> # v1.1.0
  ```

  Tags still work if you prefer them, and `dependabot` will keep a SHA pin current for you (see `.github/dependabot.yml` in this repo for the pattern).

- **`config`** (optional): Path to a JSON config file.
  - Example: `config: .github/freo.json`

Example:

```yaml
- name: Run freo with custom config
  uses: berkaybilik/freo@<commit-sha> # v1.1.0
  with:
    config: .github/freo.json
```

### How the binary is trusted

`freo` ships prebuilt binaries inside this repository at `dist/<target>/freo`, so pinning a commit SHA gives you byte-exact content with no download at run time.

Because a committed binary is not something you can read in review, the action additionally verifies its [build provenance](https://docs.github.com/actions/security-guides/using-artifact-attestations) before executing it:

```bash
gh attestation verify dist/<target>/freo \
  -R berkaybilik/freo \
  --signer-workflow berkaybilik/freo/.github/workflows/release.yml
```

That proves the binary was produced by this repo's release workflow from this repo's source, rather than built elsewhere and committed by hand. You can run the same command yourself against any pinned checkout.

> **Upgrading from v1.0.x:** older versions downloaded the binary from the release page and only worked when referenced by tag. They keep working — nothing was deleted — but SHA pinning requires v1.1.0 or later.

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

### Use cases

- **AI implementation rationale**: Claude explains a non-obvious trade-off inline — reviewers see it, main branch doesn't.
- **AI reviewer hints**: "FREO: This looks scary but is safe because …" to avoid repeated false positives.
- **Review-only context**: "FREO: This is a mechanical refactor; focus on behavior changes in X."
- **Backwards compatibility notes**: "FREO: Safe to remove `legacyField` from the JSON response — frontend no longer reads it (verified in commit abc123)."
- **Temporary reviewer instructions for generated code**: "FREO: This file is autogenerated; ignore formatting churn."

### Using freo with other AI editors

For Claude, Cursor, Copilot, Codex, or any AI assistant with a rules/instructions file, add a project-level rule along the same lines as the CLAUDE.md snippet above. The principle is the same: tell the AI to annotate decisions with `FREO:` comments, and let freo clean them up on approval.

### Notes & limitations

- **Single-line only**: `freo` removes single-line comment matches; it is not a block-comment parser.
- **Only changed files**: the action runs on **added/modified files** in the PR compared to the base branch.
- **Needs a PR context**: the action determines the base branch from PR metadata; it's intended for `pull_request`, `pull_request_review`, or `pull_request_target`.
- **Fork PRs**: you typically can't push back to fork branches with the default token; the example workflow detects forks and skips pushing.

### Run locally (CLI)

If you want to test before wiring the Action, you can run the CLI in this repo:

```bash
cargo run -- -f path/to/file1.rs path/to/file2.py
```

With a config file:

```bash
cargo run -- -c freo.json -f path/to/file1.rs
```
