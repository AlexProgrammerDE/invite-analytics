import {
  type ChatInputCommandInteraction,
  Client,
  Collection,
  GatewayIntentBits,
  type SlashCommandBuilder,
  type SlashCommandOptionsOnlyBuilder,
  type SlashCommandSubcommandsOnlyBuilder,
} from "discord.js";

export interface Command {
  data:
    | SlashCommandBuilder
    | SlashCommandSubcommandsOnlyBuilder
    | SlashCommandOptionsOnlyBuilder
    | Omit<SlashCommandBuilder, "addSubcommand" | "addSubcommandGroup">;
  execute: (interaction: ChatInputCommandInteraction) => Promise<void>;
}

declare module "discord.js" {
  interface Client {
    commands: Collection<string, Command>;
  }
}

export class InviteAnalyticsClient extends Client {
  commands = new Collection<string, Command>();

  constructor() {
    super({
      intents: [
        GatewayIntentBits.Guilds,
        GatewayIntentBits.GuildMembers,
        GatewayIntentBits.GuildInvites,
      ],
    });
  }
}
