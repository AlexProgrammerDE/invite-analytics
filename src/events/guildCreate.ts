import { Events, type Guild } from "discord.js";
import { db } from "../db/index.js";
import { guilds } from "../db/schema.js";
import { cacheGuildInvites } from "../lib/inviteCache.js";
import { logger } from "../logger.js";

export const name = Events.GuildCreate;

export async function execute(guild: Guild) {
  logger.info({ guildId: guild.id, name: guild.name }, "Joined new guild");

  // Upsert guild into DB
  await db.insert(guilds).values({ id: guild.id }).onConflictDoNothing();

  // Cache invites
  try {
    await cacheGuildInvites(guild);
  } catch (err) {
    logger.warn(
      { err, guildId: guild.id },
      "Failed to cache invites for new guild",
    );
  }
}
