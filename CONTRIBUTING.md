# Contribute to invite-analytics

Contributions can fix behavior, improve documentation, or add focused tests.
See [README](README.md) for runtime setup.

## Before you start

Read [the support guide](SUPPORT.md) for questions and issue routing.
Search existing issues and pull requests. Discuss larger API, architecture, or dependency changes before implementation.

Work from `main` and target that branch in your pull request.
Keep each change focused. Avoid unrelated formatting and dependency updates.

## Prepare a checkout

Install Rust through Rustup. The toolchain is pinned in `rust-toolchain.toml`. Runtime integration needs PostgreSQL, Redis, and a test Discord guild.

Run the commands below from the repository root unless a command names another directory.
On Windows, use `gradlew.bat` in place of `./gradlew` for Gradle commands.

## Repository layout

- `src/`: commands, invite tracking, event handling, and database code.
- `migrations/`: PostgreSQL migrations.
- `.env.example`: required runtime variables.

## Verify your change

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

Use separate Discord and database resources for integration work. Cover invite attribution, cache refresh, event ordering, and CSV transfer with focused tests. Inspect ignored tests before using the CI command with `--include-ignored`. Run them only with the required test services. Keep Discord tokens, member data, and connection credentials out of reports.

Run the relevant checks before review. State the command and result in the pull request.
If a check cannot run, explain the missing dependency or service. Do not claim it passed.
Keep generated artifacts consistent with their source and review their diff.

## Style and documentation

Follow the existing code conventions and repository formatter. Keep commit hooks enabled.
Add focused tests for changed logic when practical. Avoid tests that only assert source strings.
Update documentation when commands, APIs, configuration, or expected behavior change.
Keep examples small and reproducible. Preserve exact identifiers, commands, and error messages.

## Open a pull request

Explain the problem and resulting behavior. Link related issues without a placeholder issue number.
Explain data migration and attribution behavior. State which service-backed tests ran.
Include commands and results. State any runtime checks that remain necessary.
Respond to review with a correction or concrete evidence.

Use Conventional Commits: `type(scope): description`, for example `docs(contributing): explain local validation`.
Use a meaningful scope, or omit it. Keep the subject concise and imperative.
Add a body when the reason or compatibility impact is not obvious.

For vulnerabilities, follow [the security reporting instructions](SECURITY.md).
Remove credentials and private data from examples, logs, and screenshots.
