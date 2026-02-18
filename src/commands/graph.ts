import {
  AttachmentBuilder,
  type ChatInputCommandInteraction,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { and, count, desc, eq } from "drizzle-orm";
import React from "react";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { inviteUses, trackedInvites } from "../db/schema.js";
import { BarChart, type BarChartData } from "../graph/components/BarChart.js";
import { renderToImage } from "../graph/render.js";
import { brandEmbed, errorEmbed } from "../lib/embeds.js";
import { requireGuild } from "../lib/permissions.js";

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("graph")
    .setDescription("Generate an invite analytics chart")
    .setDefaultMemberPermissions(0x8)
    .addStringOption((opt) =>
      opt
        .setName("type")
        .setDescription("Type of graph to generate")
        .setRequired(true)
        .addChoices(
          { name: "Primary Sources", value: "primary" },
          { name: "Secondary Sources", value: "secondary" },
        ),
    )
    .addStringOption((opt) =>
      opt
        .setName("source")
        .setDescription(
          "Filter by primary source (only for secondary graph type)",
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const type = interaction.options.getString("type", true);
    const sourceFilter = interaction.options.getString("source");

    await interaction.deferReply({ flags: MessageFlags.Ephemeral });

    let data: BarChartData[];
    let title: string;

    const joinsCount = count(inviteUses.id);

    if (type === "primary") {
      const results = await db
        .select({
          source: trackedInvites.primarySource,
          joins: joinsCount,
        })
        .from(inviteUses)
        .innerJoin(
          trackedInvites,
          eq(trackedInvites.id, inviteUses.trackedInviteId),
        )
        .where(eq(inviteUses.guildId, guildId))
        .groupBy(trackedInvites.primarySource)
        .orderBy(desc(joinsCount))
        .limit(10);

      data = results.map((r) => ({
        label: r.source,
        value: r.joins,
      }));

      title = `Top Primary Invite Sources for "${interaction.guild?.name}"`;
    } else {
      // Secondary — optionally filtered by primary source
      const whereClause = sourceFilter
        ? and(
            eq(inviteUses.guildId, guildId),
            eq(trackedInvites.primarySource, sourceFilter),
          )
        : eq(inviteUses.guildId, guildId);

      const results = await db
        .select({
          source: trackedInvites.secondarySource,
          joins: joinsCount,
        })
        .from(inviteUses)
        .innerJoin(
          trackedInvites,
          eq(trackedInvites.id, inviteUses.trackedInviteId),
        )
        .where(whereClause)
        .groupBy(trackedInvites.secondarySource)
        .orderBy(desc(joinsCount))
        .limit(10);

      data = results.map((r) => ({
        label: r.source,
        value: r.joins,
      }));

      title = sourceFilter
        ? `Top Secondary Invite Sources from "${sourceFilter}" for "${interaction.guild?.name}"`
        : `Top Secondary Invite Sources for "${interaction.guild?.name}"`;
    }

    if (data.length === 0) {
      await interaction.editReply({
        embeds: [
          errorEmbed(
            "No invite usage data found. Members need to join via tracked invites first.",
          ),
        ],
      });
      return;
    }

    // Render the chart
    const chartElement = React.createElement(BarChart, { title, data });
    const pngBuffer = await renderToImage(chartElement);

    const attachment = new AttachmentBuilder(pngBuffer, {
      name: "chart.png",
    });

    const embed = brandEmbed()
      .setTitle(title)
      .setImage("attachment://chart.png");

    await interaction.editReply({
      embeds: [embed],
      files: [attachment],
    });
  },
};

export default command;
