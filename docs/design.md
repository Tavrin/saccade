# Architecture

Saccade has two crates. `saccade-core` owns comparison, evidence contracts,
validation and portable rendering. `saccade` owns CLI/MCP transport and config.
The core defaults to parallel image processing. Optional graphics, AI,
workbench, evaluation, prechecks and schema features isolate computation while
keeping evidence types readable.

Measured reports are immutable references in canonical evidence cases.
Requests bind selected evidence and policy; provider adapters supply attributed
proposals. Human decisions bind displayed content, inputs and scope.
Neither model answers nor calibration establish human authority.

Serve and MCP share canonical containment and the root registry. Provider
transport enforces human authorization, source-root egress, user-owned endpoints
and the shared attempt ledger. These are application boundaries within the local
trust environment, not protection against an unrestricted shell agent.

Task documentation:

- [Captures](captures.md), [identity/performance](identity-and-performance.md)
- [Review](review.md), [agents](agents.md), [CI](ci.md)
- [Contracts](contracts.md), [evaluation](evaluation.md)
- [Generated CLI reference](cli.md), [experiments](experimental.md)
- [Design decisions](design-decisions/)
