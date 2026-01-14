# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.1.x   | :white_check_mark: |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in proc, please report it responsibly.

### How to Report

**DO NOT** create a public GitHub issue for security vulnerabilities.

Instead, please email security concerns to: **[me@yazeed.com](mailto:me@yazeed.com)**

Include the following information:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### What to Expect

1. **Acknowledgment**: We will acknowledge receipt of your report within 48 hours.

2. **Investigation**: We will investigate the issue and determine its severity.

3. **Communication**: We will keep you informed of our progress.

4. **Resolution**: We will work to resolve the issue as quickly as possible.

5. **Credit**: We will credit you in the release notes (unless you prefer to remain anonymous).

### Response Timeline

- **Critical vulnerabilities**: Patch within 24-48 hours
- **High severity**: Patch within 1 week
- **Medium severity**: Patch within 2 weeks
- **Low severity**: Patch in next regular release

## Security Considerations

### Design Principles

proc is designed with security in mind:

1. **No Network Access**: proc only accesses local system information. It does not make network requests or phone home.

2. **No Data Collection**: proc does not collect, store, or transmit any user data or telemetry.

3. **Minimal Privileges**: proc runs with the privileges of the invoking user. It does not require or request elevated privileges except when explicitly needed (e.g., killing processes owned by other users).

4. **Safe Defaults**: Destructive operations (like `kill`) require confirmation by default.

5. **Input Validation**: All user inputs are validated to prevent injection attacks.

6. **Memory Safety**: Written in Rust, which provides memory safety guarantees.

### Dependencies

We carefully vet all dependencies:

- Minimal dependency tree
- Only well-maintained, widely-used crates
- Regular dependency audits with `cargo audit`
- Pinned versions via `Cargo.lock`

### Known Limitations

1. **Process Information Access**: On some systems, proc may not be able to access information about processes owned by other users without elevated privileges.

2. **Signal Sending**: Killing processes owned by other users requires appropriate permissions (typically root/sudo).

3. **Port Information**: Retrieving port information may require elevated privileges on some systems.

## Security Best Practices for Users

1. **Review before killing**: Always verify the processes you're about to kill, especially when using wildcards or patterns.

2. **Use dry-run mode**: When uncertain, use `--dry-run` to see what would happen without making changes.

3. **Avoid running as root**: Run proc with regular user privileges when possible. Only use sudo when necessary.

4. **Keep proc updated**: Install security updates when available.

## Audit History

| Date | Type | Findings | Resolution |
|------|------|----------|------------|
| 2026-01-12 | v1.0.0 Release | N/A | N/A |

## Contact

For security concerns: [me@yazeed.com](mailto:me@yazeed.com)

For general questions: [GitHub Issues](https://github.com/yazeed/proc/issues)
