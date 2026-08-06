# Security Policy

## Reporting a vulnerability

We take security seriously. If you believe you have found a security
vulnerability in Harmonia, please report it to us privately before
disclosing it publicly.

**Please do not open a public issue for security problems.**

To report a vulnerability:

1. Use **GitHub private vulnerability reporting** on the repository
   (Security → *Report a vulnerability*), or
2. Open a private issue via [https://github.com/bgh30539-code/harmonia/security](https://github.com/bgh30539-code/harmonia/security).

Please include:

- The affected version(s)
- A description of the vulnerability and its impact
- Steps to reproduce, including any crafted input files if applicable
- (Optional) a suggested fix

You will receive an acknowledgement within 3 business days, and we will
coordinate a fix and disclosure timeline with you.

## Supported versions

| Version | Supported |
| --- | --- |
| Latest release (v0.1.x) | ✅ Security fixes |
| Older releases | ⚠️ Best-effort, upgrade recommended |

We recommend always running the [latest release](https://github.com/bgh30539-code/harmonia/releases/latest).

## Security considerations

- **Untrusted media files.** Harmonia parses audio files (tags, embedded
  artwork, lyrics) using well-tested libraries (`lofty`, `symphonia`). The
  scanner isolates per-file failures, so a malformed or malicious file is
  logged and skipped — it cannot crash the app. That said, treat audio files
  from untrusted sources with the usual caution.
- **Signing.** Android release artifacts are signed with the project's public
  FOSS release key (see [docs/INSTALL.md](docs/INSTALL.md)) so builds are
  reproducible and verifiable; this key is intended to be replaced with a
  private keystore before any store distribution.
- **Updates.** Always verify the SHA-256 checksums published with each
  [release](https://github.com/bgh30539-code/harmonia/releases) before
  installing on systems that require high assurance.
