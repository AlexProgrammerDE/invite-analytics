import { Events, type Interaction, MessageFlags } from "discord.js";
import { errorEmbed } from "../lib/embeds.js";
import { handlePaginationButton } from "../lib/pagination.js";
import { logger } from "../logger.js";

export const name = Events.InteractionCreate;

export async function execute(interaction: Interaction) {
  if (interaction.isButton()) {
    if (interaction.customId.startsWith("page:")) {
      await handlePaginationButton(interaction);
    }
    return;
  }

  if (!interaction.isChatInputCommand()) return;

  const command = interaction.client.commands.get(interaction.commandName);

  if (!command) {
    logger.warn(`No command matching ${interaction.commandName}`);
    return;
  }

  try {
    await command.execute(interaction);
  } catch (error) {
    logger.error(
      { error, command: interaction.commandName },
      "Command execution error",
    );

    const embed = errorEmbed("An error occurred while executing this command.");

    if (interaction.replied || interaction.deferred) {
      await interaction.followUp({
        embeds: [embed],
        flags: MessageFlags.Ephemeral,
      });
    } else {
      await interaction.reply({
        embeds: [embed],
        flags: MessageFlags.Ephemeral,
      });
    }
  }
}
