import { headers } from "next/headers";
import { redirect } from "next/navigation";
import LogsTable from "@/components/LogsTable";
import { auth } from "@/lib/auth";

export default async function LogsPage() {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/login");
  }

  return (
    // Viewport minus the layout footer (57px), so only the list scrolls.
    <div className="flex h-[calc(100dvh-57px)] flex-col">
      <LogsTable />
    </div>
  );
}
