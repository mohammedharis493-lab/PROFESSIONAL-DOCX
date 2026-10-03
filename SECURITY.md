# Security Policy

## Supported versions

Professional DocX is currently pre-1.0 software under active development. Security fixes are made against the current supported development line and the latest public release when releases begin. Older commits and unmaintained builds should not be treated as supported.

## Reporting a vulnerability

Please do not disclose suspected vulnerabilities in a public issue, discussion, pull request, screenshot, or log.

When this repository is public, use **Security → Report a vulnerability** so the report is sent privately to the maintainer through GitHub's private vulnerability reporting flow.

A useful report includes:

- affected version or commit;
- security impact;
- reproducible steps using synthetic or reporter-owned data;
- relevant operating system and deployment context;
- any logs needed to understand the issue, with secrets and private data removed.

Do not include real client or audit documents, production credentials, internal server names, private IP addresses, VPN details, access tokens, signing keys, database contents, or confidential filesystem paths.

## Safe research expectations

Security testing must be limited to systems, repositories, files, and accounts you own or are explicitly authorized to test. Do not test against third-party audit environments, client infrastructure, production servers, or other users' data.

## Sensitive-data incidents

If a credential, client artifact, production configuration, or other confidential information is accidentally committed or disclosed, treat it as compromised. Revoke or rotate the affected credential first, then remove the material from the repository/history as appropriate.

## Disclosure

Please allow the maintainer an opportunity to investigate and prepare a fix before public disclosure. Coordinated disclosure details can be agreed through the private vulnerability report.
