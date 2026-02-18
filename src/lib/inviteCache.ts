import type { Guild } from "discord.js";
import { logger } from "../logger.js";
import { redis } from "../redis/index.js";

function inviteCacheKey(guildId: string) {
  return `guild:${guildId}:invites`;
}

/** Snapshot all current invite use counts into Redis */
export async function cacheGuildInvites(guild: Guild) {
  const invites = await guild.invites.fetch();
  const key = inviteCacheKey(guild.id);

  // Clear existing cache for this guild
  await redis.del(key);

  if (invites.size === 0) return;

  const entries: Record<string, string> = {};
  for (const invite of invites.values()) {
    entries[invite.code] = String(invite.uses ?? 0);
  }
  await redis.hset(key, entries);

  logger.debug(
    { guildId: guild.id, count: invites.size },
    "Cached guild invites",
  );
}

/** Get cached invite counts for a guild */
export async function getCachedInvites(
  guildId: string,
): Promise<Map<string, number>> {
  const key = inviteCacheKey(guildId);
  const data = await redis.hgetall(key);
  const map = new Map<string, number>();
  for (const [code, uses] of Object.entries(data)) {
    map.set(code, parseInt(uses, 10));
  }
  return map;
}

/** Update a single invite's cached use count */
export async function updateCachedInvite(
  guildId: string,
  code: string,
  uses: number,
) {
  await redis.hset(inviteCacheKey(guildId), code, String(uses));
}

/** Remove a single invite from cache */
export async function removeCachedInvite(guildId: string, code: string) {
  await redis.hdel(inviteCacheKey(guildId), code);
}
