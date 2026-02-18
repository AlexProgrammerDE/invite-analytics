import { REST, Routes } from "discord.js";
import type { InviteAnalyticsClient } from "../client.js";
import { env } from "../env.js";
import { logger } from "../logger.js";

import createCommand from "./create.js";
import deleteCommand from "./delete.js";
import exportCommand from "./export.js";
import graphCommand from "./graph.js";
import importCommand from "./import.js";
import linksCommand from "./links.js";
import lookupCommand from "./lookup.js";
import setCommand from "./set.js";
import statsCommand from "./stats.js";

const commands = [
  createCommand,
  linksCommand,
  lookupCommand,
  graphCommand,
  setCommand,
  importCommand,
  exportCommand,
  statsCommand,
  deleteCommand,
];

export function registerCommands(client: InviteAnalyticsClient) {
  for (const command of commands) {
    client.commands.set(command.data.name, command);
  }
  logger.info(`Registered ${commands.length} commands locally`);
}

export async function deployCommands() {
  const rest = new REST().setToken(env.DISCORD_TOKEN);
  const body = commands.map((c) => c.data.toJSON());

  logger.info(`Deploying ${body.length} slash commands to Discord API...`);

  await rest.put(Routes.applicationCommands(env.DISCORD_CLIENT_ID), { body });

  logger.info("Slash commands deployed successfully");
}

// Allow running directly: npx tsx src/commands/index.ts
if (
  process.argv[1]?.endsWith("commands/index.ts") ||
  process.argv[1]?.endsWith("commands/index.js")
) {
  deployCommands()
    .then(() => process.exit(0))
    .catch((err) => {
      console.error("Failed to deploy commands:", err);
      process.exit(1);
    });
}
