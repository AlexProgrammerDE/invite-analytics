import "dotenv/config";
import { InviteAnalyticsClient } from "./client.js";
import { deployCommands, registerCommands } from "./commands/index.js";
import { env } from "./env.js";
import * as guildCreateEvent from "./events/guildCreate.js";
import * as guildMemberAddEvent from "./events/guildMemberAdd.js";
import * as interactionCreateEvent from "./events/interactionCreate.js";
import * as inviteDeleteEvent from "./events/inviteDelete.js";
// Events
import * as readyEvent from "./events/ready.js";
import { logger } from "./logger.js";
import { redis } from "./redis/index.js";

async function main() {
  const client = new InviteAnalyticsClient();

  // Register commands in memory
  registerCommands(client);

  // Register event handlers
  client.once(readyEvent.name, readyEvent.execute);
  client.on(interactionCreateEvent.name, interactionCreateEvent.execute);
  client.on(guildMemberAddEvent.name, guildMemberAddEvent.execute);
  client.on(guildCreateEvent.name, guildCreateEvent.execute);
  client.on(inviteDeleteEvent.name, inviteDeleteEvent.execute);

  // Connect to Redis
  await redis.connect();

  // Deploy slash commands on startup
  await deployCommands();

  // Login to Discord
  await client.login(env.DISCORD_TOKEN);

  logger.info("InviteAnalytics bot started");
}

main().catch((err) => {
  logger.fatal({ err }, "Failed to start bot");
  process.exit(1);
});
