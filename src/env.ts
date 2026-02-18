import { z } from "zod";

const envSchema = z.object({
  DISCORD_TOKEN: z.string().min(1),
  DISCORD_CLIENT_ID: z.string().min(1),
  DATABASE_URL: z.string().url(),
  REDIS_URL: z.string().min(1),
});

export const env = envSchema.parse(process.env);
