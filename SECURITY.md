# Security Policy

## Offline Guarantee & Data Privacy
Sheaf is built from the ground up with a privacy-first guarantee:
- **Zero Network Calls**: Sheaf has no networking dependencies, telemetry, analytics, or automatic update checks. Network crates (`reqwest`, `hyper`, `curl`, etc.) are actively blocked at build-time via `deny.toml`.
- **Zero Secret Retention**: Passwords provided to `sheaf` are ingested into memory using `secrecy::SecretString`. They are never logged to console, written to temporary files outside destination folders, or included in error traces.
- **Atomic Operations**: Files are processed using scratch files and atomic renaming to prevent corruption.

## Reporting a Vulnerability
If you discover a potential security vulnerability in Sheaf, please report it privately:

1. **GitHub Security Advisories**: Use the "Report a vulnerability" button under the repository's Security tab on GitHub.
2. **Email**: If you prefer email, contact the maintainers directly at `security@sheaf.app`.

Please provide:
- A description of the issue and potential impact
- Detailed steps to reproduce or a proof of concept PDF
- The Sheaf version and macOS version tested

We will acknowledge receipt within 48 hours and work on a fix promptly.
