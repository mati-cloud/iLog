import { headers } from "next/headers";
import { redirect } from "next/navigation";
import LogsTable from "@/components/LogsTable";
import { auth } from "@/lib/auth";

export default async function LogsPage({
  searchParams,
}: {
  searchParams: { service?: string };
}) {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/login");
  }

  return (
    <div className="flex flex-col h-screen bg-background">
      <LogsTable serviceFilter={searchParams.service} />
    </div>
  );
}
