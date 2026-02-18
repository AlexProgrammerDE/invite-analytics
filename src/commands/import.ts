import {
  type ChatInputCommandInteraction,
  type Invite,
  MessageFlags,
  SlashCommandBuilder,
} from "discord.js";
import { count, eq } from "drizzle-orm";
import type { Command } from "../client.js";
import { db } from "../db/index.js";
import { guilds, inviteRoles, trackedInvites } from "../db/schema.js";
import { logEmbed, sendLogMessage, writeAuditLog } from "../lib/auditLog.js";
import { brandEmbed, errorEmbed, successEmbed } from "../lib/embeds.js";
import { updateCachedInvite } from "../lib/inviteCache.js";
import { requireGuild } from "../lib/permissions.js";
import { logger } from "../logger.js";

interface CsvRow {
  inviteCode: string;
  primarySource: string;
  secondarySource: string;
  creatorId: string;
  createdAt: Date;
  roleIds: string[];
}

function parseCsv(content: string): CsvRow[] {
  const lines = content.trim().split("\n");
  if (lines.length < 2) return [];

  // Skip header line
  return lines.slice(1).map((line) => {
    const parts = line.split(",").map((s) => s.trim());
    const roleIds = parts[5]
      ? parts[5]
          .split(";")
          .map((r) => r.trim())
          .filter(Boolean)
      : [];

    return {
      inviteCode: parts[0],
      primarySource: parts[1],
      secondarySource: parts[2],
      creatorId: parts[3],
      createdAt: parts[4] ? new Date(parts[4]) : new Date(),
      roleIds,
    };
  });
}

const command: Command = {
  data: new SlashCommandBuilder()
    .setName("import")
    .setDescription("Import existing invite links into tracking")
    .setDefaultMemberPermissions(0x8)
    .addSubcommand((sub) =>
      sub
        .setName("single")
        .setDescription("Import a single existing invite")
        .addStringOption((opt) =>
          opt
            .setName("code")
            .setDescription("The invite code")
            .setRequired(true),
        )
        .addStringOption((opt) =>
          opt
            .setName("primary_source")
            .setDescription("Primary source")
            .setRequired(true),
        )
        .addStringOption((opt) =>
          opt
            .setName("secondary_source")
            .setDescription("Secondary source")
            .setRequired(true),
        )
        .addRoleOption((opt) =>
          opt.setName("role").setDescription("Role to auto-assign on join"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("csv")
        .setDescription("Bulk import from a CSV file")
        .addAttachmentOption((opt) =>
          opt
            .setName("file")
            .setDescription("CSV file to import")
            .setRequired(true),
        ),
    ),

  async execute(interaction: ChatInputCommandInteraction) {
    const guildId = await requireGuild(interaction);
    if (!guildId) return;
    const subcommand = interaction.options.getSubcommand();

    // Ensure guild exists
    await db.insert(guilds).values({ id: guildId }).onConflictDoNothing();

    if (subcommand === "single") {
      await handleSingleImport(interaction, guildId);
    } else if (subcommand === "csv") {
      await handleCsvImport(interaction, guildId);
    }
  },
};

async function handleSingleImport(
  interaction: ChatInputCommandInteraction,
  guildId: string,
) {
  const code = interaction.options
    .getString("code", true)
    .replace("https://discord.gg/", "")
    .replace("discord.gg/", "");
  const primarySource = interaction.options.getString("primary_source", true);
  const secondarySource = interaction.options.getString(
    "secondary_source",
    true,
  );
  const roleOption = interaction.options.getRole("role");

  const guild = interaction.guild;
  if (!guild) return;

  // Verify invite exists in guild
  let discordInvite: Invite | undefined;
  try {
    discordInvite = await guild.invites.fetch(code);
  } catch {
    await interaction.reply({
      embeds: [errorEmbed(`Invite \`${code}\` not found in this server.`)],
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  if (!discordInvite) {
    await interaction.reply({
      embeds: [errorEmbed(`Invite \`${code}\` not found in this server.`)],
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  // Check if already tracked
  const existing = await db
    .select()
    .from(trackedInvites)
    .where(eq(trackedInvites.inviteCode, code))
    .limit(1);

  if (existing.length > 0) {
    await interaction.reply({
      embeds: [errorEmbed(`Invite \`${code}\` is already being tracked.`)],
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  const uses = discordInvite.uses ?? 0;

  // Insert
  const [inserted] = await db
    .insert(trackedInvites)
    .values({
      guildId,
      inviteCode: code,
      channelId: discordInvite.channel?.id ?? "",
      primarySource,
      secondarySource,
      createdBy: interaction.user.id,
      uses,
    })
    .returning();

  if (roleOption && inserted) {
    await db.insert(inviteRoles).values({
      trackedInviteId: inserted.id,
      roleId: roleOption.id,
    });
  }

  await updateCachedInvite(guildId, code, uses);

  await writeAuditLog(guildId, "invite_imported", interaction.user.id, {
    inviteCode: code,
    primarySource,
    secondarySource,
  });

  const embed = successEmbed()
    .setTitle("Invite Imported!")
    .setDescription(
      `Imported \`${code}\` with **${uses}** existing uses.\n**${primarySource}** \u2192 **${secondarySource}**`,
    );

  await interaction.reply({ embeds: [embed], flags: MessageFlags.Ephemeral });
}

async function handleCsvImport(
  interaction: ChatInputCommandInteraction,
  guildId: string,
) {
  const guild = interaction.guild;
  if (!guild) return;

  const attachment = interaction.options.getAttachment("file", true);

  if (!attachment.name.endsWith(".csv")) {
    await interaction.reply({
      embeds: [errorEmbed("Please upload a `.csv` file.")],
      flags: MessageFlags.Ephemeral,
    });
    return;
  }

  await interaction.deferReply({ flags: MessageFlags.Ephemeral });

  // Download CSV content
  const response = await fetch(attachment.url);
  const content = await response.text();

  const rows = parseCsv(content);
  if (rows.length === 0) {
    await interaction.editReply({
      embeds: [errorEmbed("CSV file is empty or has invalid format.")],
    });
    return;
  }

  // Check limit
  const [guildConfig] = await db
    .select()
    .from(guilds)
    .where(eq(guilds.id, guildId))
    .limit(1);

  const [{ value: currentCount }] = await db
    .select({ value: count() })
    .from(trackedInvites)
    .where(eq(trackedInvites.guildId, guildId));

  const maxLinks = guildConfig?.maxLinks ?? 130;

  let imported = 0;
  let skipped = 0;
  let errors = 0;
  const skippedReasons: string[] = [];

  for (const row of rows) {
    if (currentCount + imported >= maxLinks) {
      skipped += rows.length - imported - skipped - errors;
      skippedReasons.push("Link limit reached");
      break;
    }

    try {
      // Check if already tracked
      const existing = await db
        .select()
        .from(trackedInvites)
        .where(eq(trackedInvites.inviteCode, row.inviteCode))
        .limit(1);

      if (existing.length > 0) {
        skipped++;
        continue;
      }

      // Verify invite exists
      let discordInvite: Invite | undefined;
      try {
        discordInvite = await guild.invites.fetch(row.inviteCode);
      } catch {
        skipped++;
        skippedReasons.push(`\`${row.inviteCode}\` — not found`);
        continue;
      }

      if (!discordInvite) {
        skipped++;
        skippedReasons.push(`\`${row.inviteCode}\` — not found`);
        continue;
      }

      const uses = discordInvite.uses ?? 0;

      // Insert
      const [inserted] = await db
        .insert(trackedInvites)
        .values({
          guildId,
          inviteCode: row.inviteCode,
          channelId: discordInvite.channel?.id ?? "",
          primarySource: row.primarySource,
          secondarySource: row.secondarySource,
          createdBy: row.creatorId,
          uses,
          createdAt: row.createdAt,
        })
        .returning();

      // Add roles
      if (row.roleIds.length > 0 && inserted) {
        for (const roleId of row.roleIds) {
          await db.insert(inviteRoles).values({
            trackedInviteId: inserted.id,
            roleId,
          });
        }
      }

      await updateCachedInvite(guildId, row.inviteCode, uses);
      imported++;
    } catch (err) {
      errors++;
      logger.warn(
        { err, inviteCode: row.inviteCode },
        "Failed to import invite",
      );
    }
  }

  await writeAuditLog(guildId, "bulk_import", interaction.user.id, {
    totalRows: rows.length,
    imported,
    skipped,
    errors,
  });

  const embed = brandEmbed()
    .setTitle("CSV Import Complete")
    .addFields(
      { name: "Total Rows", value: String(rows.length), inline: true },
      { name: "Imported", value: String(imported), inline: true },
      { name: "Skipped", value: String(skipped), inline: true },
      { name: "Errors", value: String(errors), inline: true },
    );

  if (skippedReasons.length > 0) {
    embed.addFields({
      name: "Skip Details",
      value: skippedReasons.slice(0, 10).join("\n"),
    });
  }

  await interaction.editReply({ embeds: [embed] });

  await sendLogMessage(
    interaction.client,
    guildId,
    logEmbed(
      "Bulk Import",
      `<@${interaction.user.id}> imported **${imported}** invites from CSV (${skipped} skipped, ${errors} errors)`,
    ),
  );
}

export default command;
