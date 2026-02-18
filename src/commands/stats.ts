import {
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { and, count, desc, eq, gte, sql } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { inviteUses, trackedInvites } from "../db/schema.js";
import { brandEmbed, errorEmbed } from "../lib/embeds.js";
import { requireGuild } from "../lib/permissions.js";

const PERIOD_MAP: Record<string, number> = {
  "7d": 7,
  "30d": 30,
  all: 0,
};

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("stats")
    .setDescription("View server-wide invite analytics")
    .setDefaultMemberPermissions(0x8)
    .addStringOption((opt) =>
      opt
        .setName("period")
        .setDescription("Time period for stats")
        .addChoices(
          { name: "Last 7 days", value: "7d" },
          { name: "Last 30 days", value: "30d" },
          { name: "All time", value: "all" },
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const period = interaction.options.getString("period") ?? "30d";
    const days = PERIOD_MAP[period] ?? 30;

    // Total tracked invites
    const [{ value: totalInvites }] = await db
      .select({ value: count() })
      .from(trackedInvites)
      .where(eq(trackedInvites.guildId, guildId));

    if (totalInvites === 0) {
      await interaction.reply({
        embeds: [
          errorEmbed("No tracked invites yet. Use `/create` to get started."),
        ],
        flags: MessageFlags.Ephemeral,
      });
      return;
    }

    // Build date filter for period
    const dateFilter =
      days > 0
        ? gte(
            inviteUses.joinedAt,
            sql`NOW() - INTERVAL '${sql.raw(String(days))} days'`,
          )
        : undefined;

    const baseWhere = and(eq(inviteUses.guildId, guildId), dateFilter);

    // Total joins in period
    const [{ value: totalJoins }] = await db
      .select({ value: count() })
      .from(inviteUses)
      .where(baseWhere);

    const joinsCount = count(inviteUses.id);

    // Top 5 primary sources
    const topPrimary = await db
      .select({
        source: trackedInvites.primarySource,
        joins: joinsCount,
      })
      .from(inviteUses)
      .innerJoin(
        trackedInvites,
        eq(trackedInvites.id, inviteUses.trackedInviteId),
      )
      .where(baseWhere)
      .groupBy(trackedInvites.primarySource)
      .orderBy(desc(joinsCount))
      .limit(5);

    // Top 5 secondary sources
    const topSecondary = await db
      .select({
        source: trackedInvites.secondarySource,
        joins: joinsCount,
      })
      .from(inviteUses)
      .innerJoin(
        trackedInvites,
        eq(trackedInvites.id, inviteUses.trackedInviteId),
      )
      .where(baseWhere)
      .groupBy(trackedInvites.secondarySource)
      .orderBy(desc(joinsCount))
      .limit(5);

    const periodLabel = days > 0 ? `Last ${days} days` : "All time";

    const primaryText =
      topPrimary.length > 0
        ? topPrimary
            .map((r, i) => `${i + 1}. **${r.source}** \u2014 ${r.joins} joins`)
            .join("\n")
        : "No data yet";

    const secondaryText =
      topSecondary.length > 0
        ? topSecondary
            .map((r, i) => `${i + 1}. **${r.source}** \u2014 ${r.joins} joins`)
            .join("\n")
        : "No data yet";

    const embed = brandEmbed()
      .setTitle(`Invite Stats for ${interaction.guild?.name}`)
      .setDescription(`Showing stats for: **${periodLabel}**`)
      .addFields(
        {
          name: "Total Tracked Invites",
          value: String(totalInvites),
          inline: true,
        },
        { name: "Total Joins", value: String(totalJoins), inline: true },
        { name: "\u200b", value: "\u200b", inline: true },
        { name: "Top Primary Sources", value: primaryText, inline: false },
        { name: "Top Secondary Sources", value: secondaryText, inline: false },
      );

    await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
  },
};

export default command;
