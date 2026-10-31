# Security and scope

This repository is a research foundation, not a privileged desktop agent. The reference core receives structured test inputs and returns routing/notification decisions. It does not capture audio, call an LLM, authenticate a user, enforce OS permissions, execute tools or establish a production sandbox.

Use a private GitHub security advisory to report a vulnerability in repository code when that feature is available. Do not include credentials, recordings or competition data in a public issue. General architecture questions can use public issues with synthetic examples.

Production adapters require a trusted permission broker, authenticated IPC, least-privilege process boundaries, scoped and revocable grants, exact-preview approvals, secure secret storage, evidence verification and side-effect reconciliation. See [the permission design](docs/security-permissions.md).

Notification events in this reference are trusted inputs. A future runtime must authenticate event producers and derive `verified` from a verifier, not from model-generated claims or retrieved page content. The SQLite delivery ledger stores acknowledgements only; it cannot guarantee exactly-once audible speech across a crash.

Dependency versions, framework APIs, code licenses, model licenses and dataset rights must be checked separately before shipping. A passing finite test suite does not establish absence of prompt injection or unauthorized actions in a deployed product.
