import {
  ChannelType,
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { count, eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { guilds, inviteRoles, trackedInvites } from "../db/schema.js";
import { logEmbed, sendLogMessage, writeAuditLog } from "../lib/auditLog.js";
import { brandEmbed, errorEmbed } from "../lib/embeds.js";
import { updateCachedInvite } from "../lib/inviteCache.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("create")
    .setDescription("Create a new tracked invite link")
    .setDefaultMemberPermissions(0x8)
    .addStringOption((opt) =>
      opt
        .setName("primary_source")
        .setDescription("Primary source (e.g. Instagram, YouTube, Discord)")
        .setRequired(true),
    )
    .addStringOption((opt) =>
      opt
        .setName("secondary_source")
        .setDescription("Secondary source (e.g. Bio, Help Embed)")
        .setRequired(true),
    )
    .addChannelOption((opt) =>
      opt
        .setName("channel")
        .setDescription(
          "Channel for the invite (defaults to configured default)",
        )
        .addChannelTypes(ChannelType.GuildText),
    )
    .addRoleOption((opt) =>
      opt
        .setName("role")
        .setDescription(
          "Role to auto-assign when someone joins via this invite",
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const primarySource = interaction.options.getString("primary_source", true);
    const secondarySource = interaction.options.getString(
      "secondary_source",
      true,
    );
    const channelOption = interaction.options.getChannel("channel");
    const roleOption = interaction.options.getRole("role");

    // Ensure guild exists
    await db.insert(guilds).values({ id: guildId }).onConflictDoNothing();

    // Get guild config
    const [guildConfig] = await db
      .select()
      .from(guilds)
      .where(eq(guilds.id, guildId))
      .limit(1);

    // Determine channel
    const channelId = channelOption?.id ?? guildConfig?.defaultChannelId;
    if (!channelId) {
      await interaction.reply({
        embeds: [
          errorEmbed(
            "No channel specified and no default channel configured. Use `/set defaultchannel` first.",
          ),
        ],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    // Check invite count against limit
    const [{ value: currentCount }] = await db
      .select({ value: count() })
      .from(trackedInvites)
      .where(eq(trackedInvites.guildId, guildId));

    if (currentCount >= guildConfig.maxLinks) {
      await interaction.reply({
        embeds: [
          errorEmbed(
            `You've reached the maximum of **${guildConfig.maxLinks}** tracked links. Use \`/set maxlinks\` to increase the limit or \`/delete\` unused links.`,
          ),
        ],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    // Create the Discord invite
    const channel = await interaction.guild?.channels.fetch(channelId);
    if (!channel || channel.type !== ChannelType.GuildText) {
      await interaction.reply({
        embeds: [errorEmbed("Could not access the specified channel.")],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    const invite = await channel.createInvite({
      maxAge: 0,
      maxUses: 0,
      unique: true,
    });

    // Insert into DB
    const [inserted] = await db
      .insert(trackedInvites)
      .values({
        guildId,
        inviteCode: invite.code,
        channelId,
        primarySource,
        secondarySource,
        createdBy: interaction.user.id,
      })
      .returning();

    // Add role if specified
    if (roleOption && inserted) {
      await db.insert(inviteRoles).values({
        trackedInviteId: inserted.id,
        roleId: roleOption.id,
      });
    }

    // Cache the new invite
    await updateCachedInvite(guildId, invite.code, 0);

    // Audit log
    await writeAuditLog(guildId, "invite_created", interaction.user.id, {
      inviteCode: invite.code,
      primarySource,
      secondarySource,
      channelId,
      roleId: roleOption?.id,
    });

    // Build response embed
    const embed = brandEmbed()
      .setTitle("Invite Created!")
      .setDescription(
        [
          `\u2192 **Invite Link:** https://discord.gg/${invite.code}`,
          "",
          `\u2022 **Primary Source:** ${primarySource}`,
          `\u2022 **Secondary Source:** ${secondarySource}`,
          `\u2022 **Channel:** <#${channelId}>`,
          roleOption ? `\u2022 **Auto-Role:** <@&${roleOption.id}>` : null,
          "",
          "To make the most of InviteAnalytics, only share this link on the platform you have designated above.",
        ]
          .filter(Boolean)
          .join("\n"),
      )
      .setFooter({
        text: `${currentCount + 1} / ${guildConfig.maxLinks} links`,
      });

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });

    // Log to log channel
    await sendLogMessage(
      interaction.client,
      guildId,
      logEmbed(
        "Invite Created",
        `<@${interaction.user.id}> created invite \`${invite.code}\`\n**${primarySource}** \u2192 **${secondarySource}**`,
      ),
    );
  },
};

export default command;
