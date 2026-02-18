import {
  type ChatInputCommandInteraction,
  type EmbedBuilder,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { count, eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { guilds, trackedInvites } from "../db/schema.js";
import { brandEmbed, errorEmbed } from "../lib/embeds.js";
import {
  buildPaginationRow,
  registerPageRenderer,
  savePaginationState,
} from "../lib/pagination.js";
import { requireGuild } from "../lib/permissions.js";

const ITEMS_PER_PAGE = 6;

async function renderLinksPage(
  page: number,
  guildId: string,
): Promise<EmbedBuilder> {
  const allInvites = await db
    .select()
    .from(trackedInvites)
    .where(eq(trackedInvites.guildId, guildId))
    .orderBy(trackedInvites.createdAt);

  const [guildConfig] = await db
    .select()
    .from(guilds)
    .where(eq(guilds.id, guildId))
    .limit(1);

  const _totalPages = Math.max(
    1,
    Math.ceil(allInvites.length / ITEMS_PER_PAGE),
  );
  const start = (page - 1) * ITEMS_PER_PAGE;
  const pageInvites = allInvites.slice(start, start + ITEMS_PER_PAGE);

  const lines = pageInvites.map(
    (inv) =>
      `**${inv.primarySource} \u2192 ${inv.secondarySource}**\n.gg/${inv.inviteCode}`,
  );

  return brandEmbed()
    .setTitle(`Invites for ${guildId}`)
    .setDescription(
      [
        `\u2022 You are using a total of **${allInvites.length} / ${guildConfig?.maxLinks ?? 130}** links`,
        "",
        ...lines,
        "",
        "\ud83d\udca1 Use the `/lookup` command to check invite uses.",
      ].join("\n"),
    );
}

// Register the page renderer for pagination buttons
registerPageRenderer("links", renderLinksPage);

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("links")
    .setDescription("View all tracked invite links")
    .setDefaultMemberPermissions(0x8),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;

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

    const totalPages = Math.ceil(totalInvites / ITEMS_PER_PAGE);
    const embed = await renderLinksPage(1, guildId);

    // Update title with actual guild name
    embed.setTitle(`Invites for ${interaction.guild?.name}`);

    const row = buildPaginationRow(1, totalPages);

    const reply = await interaction.reply({
      embeds: [embed],
      components: totalPages > 1 ? [row] : [],
      flags: MessageFlags.Ephemeral,
    });

    if (totalPages > 1) {
      const message = await reply.fetch();
      await savePaginationState(message.id, {
        currentPage: 1,
        totalPages,
        guildId,
        commandName: "links",
      });
    }
  },
};

export default command;
