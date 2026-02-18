import { EmbedBuilder, Events, type GuildMember } from "discord.js";
import { and, eq, sql } from "drizzle-orm";
import { db } from "../db/index.js";
import { inviteRoles, inviteUses, trackedInvites } from "../db/schema.js";
import { sendLogMessage, writeAuditLog } from "../lib/auditLog.js";
import { BRAND_COLOR } from "../lib/embeds.js";
import { cacheGuildInvites, getCachedInvites } from "../lib/inviteCache.js";
import { logger } from "../logger.js";

export const name = Events.GuildMemberAdd;

export async function execute(member: GuildMember) {
  const guild = member.guild;

  try {
    // Get cached invite counts (before the join)
    const cachedInvites = await getCachedInvites(guild.id);

    // Fetch current invite counts (after the join)
    const currentInvites = await guild.invites.fetch();

    // Find the invite whose uses increased
    let usedInviteCode: string | null = null;

    for (const invite of currentInvites.values()) {
      const cachedUses = cachedInvites.get(invite.code) ?? 0;
      if ((invite.uses ?? 0) > cachedUses) {
        usedInviteCode = invite.code;
        break;
      }
    }

    // Update cache with new counts
    await cacheGuildInvites(guild);

    if (!usedInviteCode) {
      logger.debug(
        { guildId: guild.id, userId: member.id },
        "Could not determine which invite was used",
      );
      return;
    }

    // Look up the tracked invite
    const [tracked] = await db
      .select()
      .from(trackedInvites)
      .where(
        and(
          eq(trackedInvites.guildId, guild.id),
          eq(trackedInvites.inviteCode, usedInviteCode),
        ),
      )
      .limit(1);

    if (!tracked) {
      logger.debug(
        { guildId: guild.id, inviteCode: usedInviteCode },
        "Invite not tracked",
      );
      return;
    }

    // Record the join
    await db.insert(inviteUses).values({
      trackedInviteId: tracked.id,
      guildId: guild.id,
      userId: member.id,
    });

    // Increment use count
    await db
      .update(trackedInvites)
      .set({ uses: sql`${trackedInvites.uses} + 1` })
      .where(eq(trackedInvites.id, tracked.id));

    // Auto-role assignment
    const roles = await db
      .select()
      .from(inviteRoles)
      .where(eq(inviteRoles.trackedInviteId, tracked.id));

    const assignedRoles: string[] = [];
    for (const role of roles) {
      try {
        await member.roles.add(role.roleId);
        assignedRoles.push(role.roleId);
      } catch (err) {
        logger.warn(
          { err, roleId: role.roleId, memberId: member.id },
          "Failed to assign auto-role",
        );
      }
    }

    // Audit log
    await writeAuditLog(guild.id, "member_joined", member.id, {
      inviteCode: usedInviteCode,
      primarySource: tracked.primarySource,
      secondarySource: tracked.secondarySource,
      rolesAssigned: assignedRoles,
    });

    // Send to log channel
    const rolesText =
      assignedRoles.length > 0
        ? `\n**Roles Assigned:** ${assignedRoles.map((r) => `<@&${r}>`).join(", ")}`
        : "";

    await sendLogMessage(
      member.client,
      guild.id,
      new EmbedBuilder()
        .setColor(BRAND_COLOR)
        .setTitle("Member Joined")
        .setDescription(
          `<@${member.id}> joined via \`${usedInviteCode}\`\n**Source:** ${tracked.primarySource} \u2192 ${tracked.secondarySource}${rolesText}`,
        )
        .setThumbnail(member.user.displayAvatarURL())
        .setTimestamp(),
    );
  } catch (err) {
    logger.error(
      { err, guildId: guild.id, memberId: member.id },
      "Error tracking member join",
    );
  }
}
