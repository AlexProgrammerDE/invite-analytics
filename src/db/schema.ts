import { relations } from "drizzle-orm";
import {
  integer,
  jsonb,
  pgTable,
  serial,
  text,
  timestamp,
} from "drizzle-orm/pg-core";

// ─── Guilds ──────────────────────────────────────────────────────────────────

export const guilds = pgTable("guilds", {
  id: text("id").primaryKey(), // Discord guild snowflake
  defaultChannelId: text("default_channel_id"),
  logChannelId: text("log_channel_id"),
  maxLinks: integer("max_links").default(130).notNull(),
  createdAt: timestamp("created_at").defaultNow().notNull(),
  updatedAt: timestamp("updated_at").defaultNow().notNull(),
});

export const guildsRelations = relations(guilds, ({ many }) => ({
  trackedInvites: many(trackedInvites),
  inviteUses: many(inviteUses),
  auditLogs: many(auditLogs),
}));

// ─── Tracked Invites ─────────────────────────────────────────────────────────

export const trackedInvites = pgTable("tracked_invites", {
  id: serial("id").primaryKey(),
  guildId: text("guild_id")
    .notNull()
    .references(() => guilds.id, { onDelete: "cascade" }),
  inviteCode: text("invite_code").notNull().unique(),
  channelId: text("channel_id").notNull(),
  primarySource: text("primary_source").notNull(),
  secondarySource: text("secondary_source").notNull(),
  createdBy: text("created_by").notNull(),
  uses: integer("uses").default(0).notNull(),
  createdAt: timestamp("created_at").defaultNow().notNull(),
});

export const trackedInvitesRelations = relations(
  trackedInvites,
  ({ one, many }) => ({
    guild: one(guilds, {
      fields: [trackedInvites.guildId],
      references: [guilds.id],
    }),
    inviteUses: many(inviteUses),
    inviteRoles: many(inviteRoles),
  }),
);

// ─── Invite Roles ────────────────────────────────────────────────────────────

export const inviteRoles = pgTable("invite_roles", {
  id: serial("id").primaryKey(),
  trackedInviteId: integer("tracked_invite_id")
    .notNull()
    .references(() => trackedInvites.id, { onDelete: "cascade" }),
  roleId: text("role_id").notNull(),
});

export const inviteRolesRelations = relations(inviteRoles, ({ one }) => ({
  trackedInvite: one(trackedInvites, {
    fields: [inviteRoles.trackedInviteId],
    references: [trackedInvites.id],
  }),
}));

// ─── Invite Uses (join events) ───────────────────────────────────────────────

export const inviteUses = pgTable("invite_uses", {
  id: serial("id").primaryKey(),
  trackedInviteId: integer("tracked_invite_id")
    .notNull()
    .references(() => trackedInvites.id, { onDelete: "cascade" }),
  guildId: text("guild_id")
    .notNull()
    .references(() => guilds.id, { onDelete: "cascade" }),
  userId: text("user_id").notNull(),
  joinedAt: timestamp("joined_at").defaultNow().notNull(),
});

export const inviteUsesRelations = relations(inviteUses, ({ one }) => ({
  trackedInvite: one(trackedInvites, {
    fields: [inviteUses.trackedInviteId],
    references: [trackedInvites.id],
  }),
  guild: one(guilds, {
    fields: [inviteUses.guildId],
    references: [guilds.id],
  }),
}));

// ─── Audit Logs ──────────────────────────────────────────────────────────────

export const auditLogs = pgTable("audit_logs", {
  id: serial("id").primaryKey(),
  guildId: text("guild_id")
    .notNull()
    .references(() => guilds.id, { onDelete: "cascade" }),
  action: text("action").notNull(),
  performedBy: text("performed_by").notNull(),
  details: jsonb("details"),
  createdAt: timestamp("created_at").defaultNow().notNull(),
});

export const auditLogsRelations = relations(auditLogs, ({ one }) => ({
  guild: one(guilds, {
    fields: [auditLogs.guildId],
    references: [guilds.id],
  }),
}));
