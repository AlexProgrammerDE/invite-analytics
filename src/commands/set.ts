import {
  ChannelType,
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { guilds } from "../db/schema.js";
import { logEmbed, sendLogMessage, writeAuditLog } from "../lib/auditLog.js";
import { successEmbed } from "../lib/embeds.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("set")
    .setDescription("Configure InviteAnalytics settings")
    .setDefaultMemberPermissions(0x8) // Administrator
    .addSubcommand((sub) =>
      sub
        .setName("logchannel")
        .setDescription("Set the channel for invite activity logs")
        .addChannelOption((opt) =>
          opt
            .setName("channel")
            .setDescription("The log channel")
            .setRequired(true)
            .addChannelTypes(ChannelType.GuildText),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("defaultchannel")
        .setDescription("Set the default channel for creating invites")
        .addChannelOption((opt) =>
          opt
            .setName("channel")
            .setDescription("The default invite channel")
            .setRequired(true)
            .addChannelTypes(ChannelType.GuildText),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("maxlinks")
        .setDescription("Set the maximum number of tracked invite links")
        .addIntegerOption((opt) =>
          opt
            .setName("limit")
            .setDescription("Maximum number of links")
            .setRequired(true)
            .setMinValue(1)
            .setMaxValue(1000),
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const subcommand = interaction.options.getSubcommand();

    // Ensure guild exists in DB
    await db.insert(guilds).values({ id: guildId }).onConflictDoNothing();

    if (subcommand === "logchannel") {
      const channel = interaction.options.getChannel("channel", true);

      await db
        .update(guilds)
        .set({ logChannelId: channel.id, updatedAt: new Date() })
        .where(eq(guilds.id, guildId));

      await writeAuditLog(guildId, "settings_changed", interaction.user.id, {
        setting: "logchannel",
        value: channel.id,
      });

      const embed = successEmbed()
        .setTitle("Log Channel Set!")
        .setDescription(`\u2192 Channel: <#${channel.id}>`);

      await interaction.reply({
        embeds: [embed],
        flags: MessageFlags.Ephemeral,
      });
    } else if (subcommand === "defaultchannel") {
      const channel = interaction.options.getChannel("channel", true);

      await db
        .update(guilds)
        .set({ defaultChannelId: channel.id, updatedAt: new Date() })
        .where(eq(guilds.id, guildId));

      await writeAuditLog(guildId, "settings_changed", interaction.user.id, {
        setting: "defaultchannel",
        value: channel.id,
      });

      const embed = successEmbed()
        .setTitle("Default Channel Set!")
        .setDescription(`\u2192 Channel: <#${channel.id}>`);

      await interaction.reply({
        embeds: [embed],
        flags: MessageFlags.Ephemeral,
      });
    } else if (subcommand === "maxlinks") {
      const limit = interaction.options.getInteger("limit", true);

      await db
        .update(guilds)
        .set({ maxLinks: limit, updatedAt: new Date() })
        .where(eq(guilds.id, guildId));

      await writeAuditLog(guildId, "settings_changed", interaction.user.id, {
        setting: "maxlinks",
        value: limit,
      });

      const embed = successEmbed()
        .setTitle("Max Links Updated!")
        .setDescription(`\u2192 Limit: **${limit}** links`);

      await interaction.reply({
        embeds: [embed],
        flags: MessageFlags.Ephemeral,
      });

      await sendLogMessage(
        interaction.client,
        guildId,
        logEmbed(
          "Settings Changed",
          `<@${interaction.user.id}> set max links to **${limit}**`,
        ),
      );
    }
  },
};

export default command;
