import {
  ActionRowBuilder,
  ButtonBuilder,
  type ButtonInteraction,
  ButtonStyle,
  type EmbedBuilder,
  MessageFlags,
} from "discord.js";
import { z } from "zod";
import { redis } from "../redis/index.js";

const paginationStateSchema = z.object({
  currentPage: z.number(),
  totalPages: z.number(),
  guildId: z.string(),
  commandName: z.string(),
});

export type PaginationState = z.infer<typeof paginationStateSchema>;

const PAGINATION_TTL = 300; // 5 minutes

function stateKey(messageId: string) {
  return `pagination:${messageId}`;
}

export function buildPaginationRow(currentPage: number, totalPages: number) {
  return new ActionRowBuilder<ButtonBuilder>().addComponents(
    new ButtonBuilder()
      .setCustomId("page:first")
      .setLabel("<<")
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(currentPage <= 1),
    new ButtonBuilder()
      .setCustomId("page:prev")
      .setLabel("<")
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(currentPage <= 1),
    new ButtonBuilder()
      .setCustomId("page:indicator")
      .setLabel(`${currentPage} / ${totalPages}`)
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(true),
    new ButtonBuilder()
      .setCustomId("page:next")
      .setLabel(">")
      .setStyle(ButtonStyle.Primary)
      .setDisabled(currentPage >= totalPages),
    new ButtonBuilder()
      .setCustomId("page:last")
      .setLabel(">>")
      .setStyle(ButtonStyle.Secondary)
      .setDisabled(currentPage >= totalPages),
  );
}

export async function savePaginationState(
  messageId: string,
  state: PaginationState,
) {
  await redis.set(
    stateKey(messageId),
    JSON.stringify(state),
    "EX",
    PAGINATION_TTL,
  );
}

export async function getPaginationState(
  messageId: string,
): Promise<PaginationState | null> {
  const data = await redis.get(stateKey(messageId));
  if (!data) return null;
  return paginationStateSchema.parse(JSON.parse(data));
}

export type PageRenderer = (
  page: number,
  guildId: string,
) => Promise<EmbedBuilder>;

const pageRenderers = new Map<string, PageRenderer>();

export function registerPageRenderer(
  commandName: string,
  renderer: PageRenderer,
) {
  pageRenderers.set(commandName, renderer);
}

export async function handlePaginationButton(interaction: ButtonInteraction) {
  const messageId = interaction.message.id;
  const state = await getPaginationState(messageId);

  if (!state) {
    await interaction.reply({
      content: "This pagination has expired. Please run the command again.",
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const action = interaction.customId.replace("page:", "");
  let newPage = state.currentPage;

  switch (action) {
    case "first":
      newPage = 1;
      break;
    case "prev":
      newPage = Math.max(1, state.currentPage - 1);
      break;
    case "next":
      newPage = Math.min(state.totalPages, state.currentPage + 1);
      break;
    case "last":
      newPage = state.totalPages;
      break;
  }

  if (newPage === state.currentPage) {
    await interaction.deferUpdate();
    return;
  }

  state.currentPage = newPage;
  await savePaginationState(messageId, state);

  const renderer = pageRenderers.get(state.commandName);
  if (!renderer) {
    await interaction.deferUpdate();
    return;
  }

  const embed = await renderer(newPage, state.guildId);
  const row = buildPaginationRow(newPage, state.totalPages);

  await interaction.update({ embeds: [embed], components: [row] });
}
