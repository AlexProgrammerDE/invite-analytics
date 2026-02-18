import { type Client, EmbedBuilder } from "discord.js";
import { eq } from "drizzle-orm";
import { db } from "../db/index.js";
import { auditLogs, guilds } from "../db/schema.js";
import { logger } from "../logger.js";
import { BRAND_COLOR } from "./embeds.js";

export type AuditAction =
  | "invite_created"
  | "invite_deleted"
  | "invite_imported"
  | "member_joined"
  | "settings_changed"
  | "bulk_import";

export async function writeAuditLog(
  guildId: string,
  action: AuditAction,
  performedBy: string,
  details?: Record<string, unknown>,
) {
  await db.insert(auditLogs).values({
    guildId,
    action,
    performedBy,
    details: details ?? null,
  });
}

export async function sendLogMessage(
  client: Client,
  guildId: string,
  embed: EmbedBuilder,
) {
  try {
    const [guild] = await db
      .select({ logChannelId: guilds.logChannelId })
      .from(guilds)
      .where(eq(guilds.id, guildId))
      .limit(1);

    if (!guild?.logChannelId) return;

    const channel = await client.channels.fetch(guild.logChannelId);
    if (channel?.isSendable()) {
      await channel.send({ embeds: [embed] });
    }
  } catch (err) {
    logger.warn({ err, guildId }, "Failed to send log message");
  }
}

export function logEmbed(title: string, description: string) {
  return new EmbedBuilder()
    .setColor(BRAND_COLOR)
    .setTitle(title)
    .setDescription(description)
    .setTimestamp();
}
