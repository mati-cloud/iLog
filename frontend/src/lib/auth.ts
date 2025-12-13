import { betterAuth } from "better-auth";
import { nextCookies } from "better-auth/next-js";
import { jwt } from "better-auth/plugins";
import { Pool } from "pg";

const dbUrl =
  process.env.DATABASE_URL ||
  "postgresql://ilog_user:changeme123@localhost:5432/ilog";
console.log(
  "Better Auth initializing with DB:",
  dbUrl.replace(/:[^:@]+@/, ":***@"),
);

export const auth = betterAuth({
  database: new Pool({
    connectionString: dbUrl,
  }),
  emailAndPassword: {
    enabled: true,
  },
  secret:
    process.env.BETTER_AUTH_SECRET ||
    "your-secret-key-change-in-production-min-32-chars",
  baseURL: process.env.BETTER_AUTH_URL || "http://localhost:3000",
  experimental: {
    joins: true, // Enable joins for better performance
  },
  plugins: [
    nextCookies(), // Automatically handle cookies in server actions
    jwt(), // Enable JWT tokens
  ],
});

export type Session = typeof auth.$Infer.Session;
