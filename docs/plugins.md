# Agent plugins

Saccade has a Claude Code plugin in `integrations/claude-code` and a portable
skills package in `integrations/codex`. Both require `saccade` on the client
process's PATH and local access to the supplied captures. Install once:

```sh
cargo install saccade --version 0.2.9 --locked
saccade --version
```

The repository catalogs are prepared here. GitHub installation commands below
require the owner to make this commit available in `Tavrin/saccade` first.
No public directory submission or MCP Registry publication has been made by
this change. Package validation does not prove that an agent follows a skill.

## Claude Code

Inside Claude Code:

```text
/plugin marketplace add Tavrin/saccade
/plugin install saccade@saccade
```

The shell equivalents are `claude plugin marketplace add Tavrin/saccade` and
`claude plugin install saccade@saccade`. For local testing, replace the repository
name with the absolute checkout path when adding the marketplace.

Use `/saccade:check-visual-change BASELINE CANDIDATE` or `/saccade:saccade`.

For MCP tools as well, configure the server yourself; it is not bundled in the
plugin, because it launches the separately installed `saccade` binary:

```sh
claude mcp add saccade -- saccade mcp --root /absolute/captures --out-root /absolute/reports
```

Neither root may contain the other. The server runs locally over stdio and makes
no provider calls unless explicitly authorized with a finite budget.

See the [Claude manifest reference](https://code.claude.com/docs/en/plugins/manifest-reference)
and [marketplace reference](https://code.claude.com/docs/en/plugins/marketplace-reference).

## Codex

With a CLI supporting plugin installation:

```sh
codex plugin marketplace add Tavrin/saccade
codex plugin add saccade@saccade
```

For a local checkout, use `codex plugin marketplace add /absolute/path/to/saccade`
then the same `plugin add`. In the ChatGPT desktop app, open the repository,
restart, and choose the Saccade marketplace in the Plugins Directory.
The catalog at `.agents/plugins/marketplace.json` resolves its package path
from the repository root. It packages the `saccade` and `check-visual-change`
skills, without MCP. The [OpenAI package guide](https://developers.openai.com/plugins/build/plugins)
describes these surfaces; availability varies by client version.

For manual use, preserve and append
[AGENTS.saccade.md](../integrations/codex/AGENTS.saccade.md) to the project's
AGENTS.md and use the [prompt](../integrations/codex/check-visual-change.prompt.md).
Local MCP can be configured independently with `saccade mcp --root
/absolute/captures --out-root /absolute/reports`; see [agent integrations](../integrations/README.md).

## Cursor

From a checkout, copy `integrations/codex/` to
`~/.cursor/plugins/local/saccade/` and reload Cursor. Keep `plugin.json` at the
copied directory root. After directory acceptance, select Saccade in Customize
and choose Install at the desired scope. Check that both skills appear and
that the client can invoke `saccade --version` and measure supplied captures.
This package has no Cursor-specific variables or MCP configuration.
See [Cursor plugins](https://prod.cursor.com/docs/plugins) and its
[format and submission reference](https://prod.cursor.com/docs/reference/plugins).

## Copilot CLI and VS Code

Before listing, install the portable package from a checkout:

```sh
copilot plugin install ./integrations/codex
```

VS Code discovers plugins installed by Copilot CLI; check Agent Plugins -
Installed in the Extensions view. After acceptance into `awesome-copilot`:

```sh
copilot plugin install saccade@awesome-copilot
```

In VS Code, search `@agentPlugins`, find Saccade and choose Install. Do not use
`Tavrin/saccade` as a direct single-plugin source: the portable manifest lives
in a subdirectory. The repository's Claude marketplace points to the Claude
package, including its client-specific root configuration, so it is not the
portable Copilot installation route.
See [Copilot local testing](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating),
[marketplace installation](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-finding-installing)
and [VS Code agent plugins](https://code.visualstudio.com/docs/agent-customization/agent-plugins).

## Submission values

Use these values for the packages in this change. Bump plugin versions when
publishing changed packages. The owner must publish the reviewed commit before
submitting a GitHub URL or SHA. Do not use the existing v0.1.0 tag as the plugin
ref: it predates these files. Obtain the full integrated commit with
`git rev-parse HEAD` on the published revision, and paste that 40-character SHA
into each review form that requests an immutable source.

| Field | Value |
| --- | --- |
| Repository | `Tavrin/saccade` |
| Repository URL / homepage | `https://github.com/Tavrin/saccade` |
| Name / marketplace name | `saccade` |
| Display name | `Saccade` |
| Plugin version | `0.2.9` |
| Author / developer | `Tavrin` |
| Author URL | `https://github.com/Tavrin` |
| License | `MIT OR Apache-2.0` |
| Description | `Measure and review visual changes with the installed Saccade CLI.` |
| Keywords | `visual-regression, image-diff, testing` |
| Claude plugin path | `integrations/claude-code` |
| Portable plugin path | `integrations/codex` |
| Category | `Developer Tools` |
| Short description | `Visual regression reviews` |
| Long description | `Measure supplied captures and inspect bounded Saccade reports using the installed CLI.` |
| Primary logo (portable package relative) | `./assets/logo.svg` |
| Composer icon (portable package relative) | `./assets/icon.svg` |

### Anthropic

- [ ] Open the [developer portal](https://claude.ai/directory/manage) with an
  eligible account and connect GitHub with push access to `Tavrin/saccade`.
- [ ] Supply the repository URL, path `integrations/claude-code`, version
  `0.2.9`, and the published review SHA. The manifest and README supply the
  remaining package metadata above.
- [ ] Explain the installed CLI prerequisite, local capture access and report
  writes, and that provider execution is disabled by this MCP declaration.
  Answer the actual data-handling and compliance questions for the deployment.
- [ ] Validate and scan the selected commit, resolve findings, then publish a
  passing version. If the external executable is rejected, revise the package
  and validate again before publication.
- [ ] For `anthropics/claude-plugins-official`, use Anthropic's submission or
  partner route separately. Do not open a PR to the read-only community mirror.

Follow the [submission guide](https://claude.com/docs/plugins/submit) and
[pre-submission checklist](https://claude.com/docs/plugins/pre-submission-checklist).
Directory sync, the community marketplace, and the official marketplace are
separate routes; none is claimed by a local `saccade@saccade` install.

### Cursor

- [ ] Test both skills in Cursor with an installed CLI and supplied captures.
- [ ] Open the [publisher form](https://cursor.com/marketplace/publish), submit
  `https://github.com/Tavrin/saccade`, and identify `integrations/codex` as the
  plugin directory. Include the table's metadata and published review SHA.
- [ ] Confirm the open-source license, accept the publisher terms, and resolve
  the team's review findings. If the form cannot select a subdirectory, request
  that path in the review notes; do not point reviewers at the Claude package.

### awesome-copilot

- [ ] Test the portable package in Copilot CLI and VS Code.
- [ ] Open the [external-plugin issue form](https://github.com/github/awesome-copilot/issues/new?template=external-plugin.yml).
  Paste Plugin name `saccade`, GitHub repository `Tavrin/saccade`, Plugin path
  `integrations/codex`, Version `0.2.9`, License identifier `MIT OR Apache-2.0`,
  Author name `Tavrin`, Author URL `https://github.com/Tavrin`, and the description
  and keywords from the table. Homepage may use the repository URL.
- [ ] Paste the full published Commit SHA to review. Leave Ref to review empty
  unless there is an immutable release tag containing this package.
- [ ] In Additional notes, state: `Skills-only plugin. Requires saccade on PATH
  and local captures. No bundled MCP server or provider execution.`
- [ ] Check the form's policy and public-source acknowledgements after review.
  Let automation lint and smoke-test it, and wait for maintainer approval to
  create the listing PR. Do not edit `plugins/external.json` directly.

### MCP Registry

| Field | Prepared value |
| --- | --- |
| Manifest | `server.json` |
| Name / visible README token | `io.github.Tavrin/saccade` / `mcp-name: io.github.Tavrin/saccade` |
| Title | `Saccade` |
| Server version / Cargo package version | `0.2.9` / `0.2.9` |
| Registry type / URL | `cargo` / `https://crates.io` |
| Package identifier / transport | `saccade` / `stdio` |
| Invocation after installation | `saccade mcp --root /absolute/captures --out-root /absolute/reports` |

- [ ] Release a new crate version containing the visible marker in
  `README.md`, packaged by the `saccade` crate. The already-published 0.1.0
  cannot acquire this source change. Publish and verify the 0.2.9 Cargo package
  before submitting this registry metadata.
- [ ] Update both version fields in `server.json` to that actual published
  release and verify its rendered crates.io README contains the exact marker.
  A hidden HTML comment does not satisfy Cargo ownership verification.
- [ ] Install `mcp-publisher`, then run `mcp-publisher validate server.json`.
- [ ] Authenticate as the GitHub owner of `io.github.Tavrin` with
  `mcp-publisher login github`, then publish with `mcp-publisher publish server.json`.
- [ ] Verify the resulting version through the Registry API. No `runtimeHint`
  is needed: users install the Cargo binary once and clients run it on PATH.

These are owner steps, not commands run by this implementation. See
[Cargo package and ownership rules](https://github.com/modelcontextprotocol/registry/blob/main/docs/modelcontextprotocol-io/package-types.mdx)
and the [publisher commands](https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/cli/commands.md).

### OpenAI public directory, later

- [ ] Demonstrate the skills with an installed CLI and captures in the intended
  runtime. Local files and executables are not automatically available in cloud sessions.
- [ ] Decide whether the first public listing is skills-only. An MCP-backed
  listing requires a hosted HTTPS Streamable HTTP service and domain verification;
  adding MCP to an existing skills-only public listing is currently unsupported.
- [ ] In the [Plugins dashboard](https://platform.openai.com/apps), select the
  owning organization/project and verify the publisher identity. Confirm
  `Tavrin` and `Developer Tools` against the dashboard's accepted values.
- [ ] ZIP the contents of `integrations/codex` with `plugin.json`, `skills/` and
  `assets/` at the archive root. Use the table's listing fields and SVG images.
- [ ] Resolve automated findings, complete policy attestations, submit for
  review and publish only after approval. Do not claim hosted MCP in this ZIP.

See the [OpenAI submission field reference](https://developers.openai.com/plugins/deploy/submission).

## Validation

Run `python3 scripts/validate-plugin-manifests.py` with Python 3.11 or later and
`jsonschema==4.10.3`. Published schemas are vendored under
`scripts/plugin-schemas`; other manifests use checks for documented fields.
The check is offline and rejects missing assets, unresolved source paths,
version mismatches and unintended MCP authorization. The Plugins CI workflow
also checks generated skill drift, including changes limited to Markdown.

Run `claude plugin validate --strict integrations/claude-code` and
`claude plugin validate --strict .claude-plugin/marketplace.json` when available.
See the local validation record below for client coverage and remaining gates.

### Local validation record, 2026-10-05

- Passed: `cargo fmt --check`, `cargo test --workspace` (325 passed, zero failed
  or ignored), the release doc link
  checker, all six JSON manifests, generated-document drift, and both new
  skills' frontmatter validation. Cargo used empty `CARGO_BUILD_RUSTC_WRAPPER`
  and `RUSTC_WRAPPER` with the dedicated `codex-saccade-plugins` target directory,
  which was deleted after testing.
- Claude Code 2.1.289: plugin and marketplace `validate --strict --json` passed
  without warnings. Marketplace add, install with both root settings, and list
  succeeded with temporary configuration. The installed entry was enabled.
- Codex CLI 0.160.0: marketplace add, `plugin add saccade@saccade`, and list
  succeeded with temporary configuration; the portable package was enabled.
  Codex warned that helper PATH aliases cannot be created under `/tmp`; plugin
  installation still returned success. Temporary configurations were removed.
- Direct MCP startup with the manifest arguments and explicit root substitution
  passed initialization and discovery of six bounded tools. Paths containing
  spaces worked; overlapping capture/report roots failed with exit 2. This
  does not establish a Claude-hosted MCP session or interactive configuration UI.
- The validator rejected six deliberately invalid temporary fixtures: a missing
  root setting, an unknown portable field, an escaping marketplace path, a
  registry version mismatch, a provider flag, and a missing listing image.
- SVG artwork was rendered and visually inspected. No Rust behavior changed.

Not tested here: installation from the public GitHub revision (unpublished),
interactive agent execution, ChatGPT desktop discovery, Cursor, Copilot CLI,
VS Code plugin loading, directory review, or hosted MCP. Cursor and Copilot CLI
were unavailable; no desktop client plugin session was exercised. The CI job
has been added but has not run on GitHub. `mcp-publisher` was not run, and no
directory, registry, Git push or crate publication was performed.
