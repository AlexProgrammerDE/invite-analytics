import {
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { and, desc, eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { inviteRoles, inviteUses, trackedInvites } from "../db/schema.js";
import { brandEmbed, errorEmbed } from "../lib/embeds.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("lookup")
    .setDescription("Look up details for a tracked invite")
    .setDefaultMemberPermissions(0x8)
    .addStringOption((opt) =>
      opt
        .setName("code")
        .setDescription("The invite code to look up")
        .setRequired(true),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const code = interaction.options
      .getString("code", true)
      .replace("https://discord.gg/", "")
      .replace("discord.gg/", "");

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

    // Get recent joins
    const recentJoins = await db
      .select()
      .from(inviteUses)
      .where(eq(inviteUses.trackedInviteId, invite.id))
      .orderBy(desc(inviteUses.joinedAt))
      .limit(10);

    // Get assigned roles
    const roles = await db
      .select()
      .from(inviteRoles)
      .where(eq(inviteRoles.trackedInviteId, invite.id));

    const recentJoinsText =
      recentJoins.length > 0
        ? recentJoins
            .map(
              (j) =>
                `<@${j.userId}> — <t:${Math.floor(j.joinedAt.getTime() / 1000)}:R>`,
            )
            .join("\n")
        : "No joins recorded yet.";

    const rolesText =
      roles.length > 0
        ? roles.map((r) => `<@&${r.roleId}>`).join(", ")
        : "None";

    const embed = brandEmbed()
      .setTitle(`Invite Lookup: ${code}`)
      .addFields(
        {
          name: "Invite Link",
          value: `https://discord.gg/${code}`,
          inline: true,
        },
        {
          name: "Primary Source",
          value: invite.primarySource,
          inline: true,
        },
        {
          name: "Secondary Source",
          value: invite.secondarySource,
          inline: true,
        },
        {
          name: "Total Uses",
          value: String(invite.uses),
          inline: true,
        },
        {
          name: "Channel",
          value: `<#${invite.channelId}>`,
          inline: true,
        },
        {
          name: "Created By",
          value: `<@${invite.createdBy}>`,
          inline: true,
        },
        {
          name: "Auto-Roles",
          value: rolesText,
          inline: false,
        },
        {
          name: "Recent Joins",
          value: recentJoinsText,
          inline: false,
        },
      )
      .setFooter({
        text: `Created ${invite.createdAt.toLocaleDateString()}`,
      });

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
  },
};

export default command;
