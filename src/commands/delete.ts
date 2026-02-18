import {
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { and, eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { trackedInvites } from "../db/schema.js";
import { logEmbed, sendLogMessage, writeAuditLog } from "../lib/auditLog.js";
import { errorEmbed, successEmbed } from "../lib/embeds.js";
import { removeCachedInvite } from "../lib/inviteCache.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("delete")
    .setDescription("Delete a tracked invite link")
    .setDefaultMemberPermissions(0x8)
    .addStringOption((opt) =>
      opt
        .setName("code")
        .setDescription("The invite code to delete")
        .setRequired(true),
    )
    .addBooleanOption((opt) =>
      opt
        .setName("revoke")
        .setDescription("Also revoke the Discord invite (default: true)"),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const code = interaction.options
      .getString("code", true)
      .replace("https://discord.gg/", "")
      .replace("discord.gg/", "");
    const revoke = interaction.options.getBoolean("revoke") ?? true;

    // Find the tracked invite
    const [invite] = await db
      .select()
      .from(trackedInvites)
      .where(
        and(
          eq(trackedInvites.guildId, guildId),
          eq(trackedInvites.inviteCode, code),
        ),
      )
      .limit(1);

    if (!invite) {
      await interaction.reply({
        embeds: [errorEmbed(`No tracked invite found with code \`${code}\`.`)],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    // Delete from DB (cascades to invite_roles and invite_uses)
    await db.delete(trackedInvites).where(eq(trackedInvites.id, invite.id));

    // Remove from Redis cache
    await removeCachedInvite(guildId, code);

    // Optionally revoke the Discord invite
    if (revoke) {
      try {
        const discordInvite = await interaction.guild?.invites.fetch(code);
        if (discordInvite && "delete" in discordInvite) {
          await discordInvite.delete("Deleted via InviteAnalytics");
        }
      } catch {
        // Invite may already be expired/deleted
      }
    }

    // Audit log
    await writeAuditLog(guildId, "invite_deleted", interaction.user.id, {
      inviteCode: code,
      primarySource: invite.primarySource,
      secondarySource: invite.secondarySource,
      revoked: revoke,
    });

    const embed = successEmbed()
      .setTitle("Invite Deleted!")
      .setDescription(
        `Removed invite \`${code}\` (**${invite.primarySource}** \u2192 **${invite.secondarySource}**)${revoke ? "\nThe Discord invite has also been revoked." : ""}`,
      );

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });

    await sendLogMessage(
      interaction.client,
      guildId,
      logEmbed(
        "Invite Deleted",
        `<@${interaction.user.id}> deleted invite \`${code}\` (**${invite.primarySource}** \u2192 **${invite.secondarySource}**)`,
      ),
    );
  },
};

export default command;
