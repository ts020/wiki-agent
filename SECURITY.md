# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| 0.1.x | Supported after the first public release |
| < 0.1.0 | Not supported |

## Reporting a Vulnerability

Do not publish vulnerability details in a public issue.

Use GitHub Security Advisories for this repository when available. If private advisory reporting is not available yet, open a minimal public issue that says you need a private security contact, without exploit details, secret values, private paths, or affected user data.

Useful information for a private report:

- md-wiki version or commit
- operating system
- exact command line
- minimal Markdown input needed to reproduce
- impact and expected behavior

md-wiki is intended to run locally and does not use external APIs or network services during wiki generation. Security-sensitive areas are path handling, output directory cleanup protection, symlink handling, and parsing of untrusted Markdown files.

