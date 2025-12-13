import { auth } from "@/lib/auth";

export const { GET, POST } = toNextJsHandler(auth);
