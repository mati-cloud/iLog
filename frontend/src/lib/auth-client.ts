import { jwtClient } from "better-auth/client/plugins";
import { createAuthClient } from "better-auth/react";
import { config } from "./runtime-config";

export const authClient = createAuthClient({
  baseURL: config.NEXT_PUBLIC_BETTER_AUTH_URL || config.NEXT_PUBLIC_FRONTEND_URL,
  plugins: [jwtClient()],
});

export const { signIn, signUp, signOut, useSession, token } = authClient;
