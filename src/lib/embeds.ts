import { EmbedBuilder } from "discord.js";

/** Brand color — teal/cyan accent matching the dark Discord theme */
export const BRAND_COLOR = 0x2dd4bf;
export const SUCCESS_COLOR = 0x22c55e;
export const ERROR_COLOR = 0xef4444;
export const WARNING_COLOR = 0xf59e0b;

export function brandEmbed() {
  return new EmbedBuilder().setColor(BRAND_COLOR).setTimestamp();
}

export function successEmbed() {
  return new EmbedBuilder().setColor(SUCCESS_COLOR).setTimestamp();
}

export function errorEmbed(message: string) {
  return new EmbedBuilder()
    .setColor(ERROR_COLOR)
    .setDescription(`${message}`)
    .setTimestamp();
}
