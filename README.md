# Invite Analytics

Invite Analytics is a Discord bot that tracks invite usage and provides reports and CSV transfer tools.
It uses PostgreSQL for persistent data and Redis for cached state.

## Prepare a test environment

Install Rust through Rustup. The repository pins its toolchain in `rust-toolchain.toml`.
Create a separate Discord bot and guild for development. Start local PostgreSQL and Redis services.

Copy `.env.example` to an ignored `.env` file. Set these variables:

| Variable | Purpose |
| --- | --- |
| `DISCORD_TOKEN` | Token for the test Discord bot. |
| `DATABASE_URL` | PostgreSQL connection string for the test database. |
| `REDIS_URL` | Redis connection string for the test cache. |
| `RUST_LOG` | Optional log filter. |

Enable the Server Members privileged intent for the test bot in the Discord Developer Portal.
The runtime uses guild, member, and invite gateway events.
Invite the bot with the `bot` and `applications.commands` scopes.
Grant the permissions needed for the features you test, including Manage Server for invite synchronization.
Use `/health` to inspect storage, Discord access, and permissions.
Commands are restricted to guild administrators by default.
The bot applies database migrations during startup. Use a database dedicated to the test instance.

```bash
cargo run --locked
```

## Development

Read [CONTRIBUTING.md](CONTRIBUTING.md) for validation and review instructions.
Use [SUPPORT.md](SUPPORT.md) for questions and reports.
Never share a Discord token, database credential, or real member data in a public issue.
