import { type Client, Events } from "discord.js";
import { cacheGuildInvites } from "../lib/inviteCache.js";
import { logger } from "../logger.js";

export const name = Events.ClientReady;
export const once = true;

export async function execute(client: Client<true>) {
  logger.info(
    `Logged in as ${client.user?.tag} — serving ${client.guilds.cache.size} guilds`,
  );

  // Cache invite counts for all guilds
  for (const guild of client.guilds.cache.values()) {
    try {
      await cacheGuildInvites(guild);
    } catch (err) {
      logger.warn(
        { err, guildId: guild.id },
        "Failed to cache invites for guild",
      );
    }
  }

  logger.info("Invite cache initialized");
}
