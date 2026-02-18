import {
  AttachmentBuilder,
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { inviteRoles, trackedInvites } from "../db/schema.js";
import { errorEmbed } from "../lib/embeds.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("export")
    .setDescription("Export all tracked invites as a CSV file")
    .setDefaultMemberPermissions(0x8),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;

    const invites = await db
      .select()
      .from(trackedInvites)
      .where(eq(trackedInvites.guildId, guildId))
      .orderBy(trackedInvites.createdAt);

    if (invites.length === 0) {
      await interaction.reply({
        embeds: [errorEmbed("No tracked invites to export.")],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    await interaction.deferReply({ flags: MessageFlags.Ephemeral });

    // Build CSV
    const header =
      "Invite Code,Primary Source,Secondary Source,Link Creator ID,Creation Time,Roles to Give on join";
    const rows: string[] = [header];

    for (const invite of invites) {
      // Get roles for this invite
      const roles = await db
        .select()
        .from(inviteRoles)
        .where(eq(inviteRoles.trackedInviteId, invite.id));

      const roleIds = roles.map((r) => r.roleId).join(";");

      const createdAt = invite.createdAt
        .toISOString()
        .replace("T", " ")
        .replace(/\.\d{3}Z$/, "");

      rows.push(
        [
          invite.inviteCode,
          invite.primarySource,
          invite.secondarySource,
          invite.createdBy,
          createdAt,
          roleIds,
        ].join(","),
      );
    }

    const csv = rows.join("\n");
    const buffer = Buffer.from(csv, "utf-8");
    const attachment = new AttachmentBuilder(buffer, {
      name: `invites-${guildId}-${Date.now()}.csv`,
    });

    await interaction.editReply({
      content: `Exported **${invites.length}** tracked invites.`,
      files: [attachment],
    });
  },
};

export default command;
