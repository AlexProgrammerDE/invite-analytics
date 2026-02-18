import {
  type ChatInputCommandInteraction,
  MessageFlags,
  PermissionFlagsBits,
} from "discord.js";
import { errorEmbed } from "./embeds.js";

/**
 * Returns the guildId if the interaction is in a guild, otherwise replies with an error.
 * Use as: `const guildId = await requireGuild(interaction); if (!guildId) return;`
 */
export async function requireGuild(
  interaction: ChatInputCommandInteraction,
): Promise<string | null> {
  if (!interaction.guildId) {
    await interaction.reply({
      embeds: [errorEmbed("This command can only be used in a server.")],
      flags: MessageFlags.Ephemeral,
    });
    return null;
  }
  return interaction.guildId;
}

export async function requireAdmin(
  interaction: ChatInputCommandInteraction,
): Promise<boolean> {
  if (!interaction.memberPermissions?.has(PermissionFlagsBits.Administrator)) {
    await interaction.reply({
      embeds: [
        errorEmbed("You need Administrator permissions to use this command."),
      ],
      flags: MessageFlags.Ephemeral,
    });
    return false;
  }
  return true;
}
