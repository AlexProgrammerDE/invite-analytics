import { Events, type Invite } from "discord.js";
import { removeCachedInvite } from "../lib/inviteCache.js";
import { logger } from "../logger.js";

export const name = Events.InviteDelete;

export async function execute(invite: Invite) {
  if (!invite.guild) return;

  logger.debug(
    { guildId: invite.guild.id, code: invite.code },
    "Invite deleted",
  );
  await removeCachedInvite(invite.guild.id, invite.code);
}
