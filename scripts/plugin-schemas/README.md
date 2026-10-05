# Plugin manifest validation sources

These are unmodified snapshots of the published
[Agent Plugins 1.0.0 schema](https://agent-plugins.org/schemas/1.0.0/plugin.schema.json)
and [MCP Registry server schema](https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json),
retrieved on 2026-10-05. `sources.json` records the exact URLs and SHA-256 hashes.
They have only internal references; validation never retrieves remote schemas.
To update a snapshot, review the upstream changes, replace its bytes, and update
the source URL, retrieval date and hash together.

The Agent Plugins schema is Apache-2.0 software under its
[upstream licensing policy](https://github.com/agentplugins/agent-plugins-spec/blob/main/LICENSE.md);
the license text is in [LICENSE-APACHE](../../LICENSE-APACHE).
The MCP Registry's upstream license and transition notice are preserved in
[mcp-registry-LICENSE.txt](mcp-registry-LICENSE.txt), from
[the Registry license](https://github.com/modelcontextprotocol/registry/blob/main/LICENSE).

No published standalone schema was identified for the other manifests used here.
The validator checks a deliberately narrow subset of documented fields and the
package's additional constraints, not every configuration those clients accept:

- [Claude manifest and userConfig](https://code.claude.com/docs/en/plugins/manifest-reference)
- [Claude marketplace](https://code.claude.com/docs/en/plugins/marketplace-reference)
- [Claude MCP configuration](https://code.claude.com/docs/en/plugins/components)
- [Codex marketplace metadata](https://developers.openai.com/plugins/build/plugins)
- [OpenAI listing fields and image dimensions](https://developers.openai.com/plugins/deploy/submission)
- [Cargo ownership and runtime rules](https://github.com/modelcontextprotocol/registry/blob/main/docs/modelcontextprotocol-io/package-types.mdx)

Claude's own `plugin validate --strict` remains the authoritative local check.
Local schema checks do not establish client execution or registry acceptance.
