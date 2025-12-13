import { Activity, FileText, Server } from "lucide-react";
import { headers } from "next/headers";
import Link from "next/link";
import { redirect } from "next/navigation";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { auth } from "@/lib/auth";

export default async function Dashboard() {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/login");
  }

  return (
    <div className="w-full px-6 py-8">
      <div className="max-w-7xl mx-auto">
        <div className="mb-8">
          <h1 className="text-3xl font-bold">Dashboard</h1>
          <p className="text-muted-foreground mt-2">
            Welcome back, {session.user.name || session.user.email}
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          <Card className="hover:shadow-lg transition-shadow">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="p-3 bg-primary/10 rounded-lg">
                  <Server className="h-6 w-6 text-primary" />
                </div>
                <div>
                  <CardTitle>Services</CardTitle>
                  <CardDescription>Manage your services</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground mb-4">
                Create and manage services to organize your logs
              </p>
              <Link href="/services">
                <Button className="w-full">Go to Services</Button>
              </Link>
            </CardContent>
          </Card>

          <Card className="hover:shadow-lg transition-shadow">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="p-3 bg-blue-500/10 rounded-lg">
                  <FileText className="h-6 w-6 text-blue-500" />
                </div>
                <div>
                  <CardTitle>Logs</CardTitle>
                  <CardDescription>View real-time logs</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground mb-4">
                Stream and analyze logs from your services
              </p>
              <Link href="/logs">
                <Button className="w-full" variant="outline">
                  View Logs
                </Button>
              </Link>
            </CardContent>
          </Card>

          <Card className="hover:shadow-lg transition-shadow">
            <CardHeader>
              <div className="flex items-center gap-3">
                <div className="p-3 bg-green-500/10 rounded-lg">
                  <Activity className="h-6 w-6 text-green-500" />
                </div>
                <div>
                  <CardTitle>Getting Started</CardTitle>
                  <CardDescription>Quick setup guide</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <ol className="text-sm text-muted-foreground space-y-2 mb-4">
                <li>1. Create a service</li>
                <li>2. Generate a token</li>
                <li>3. Configure your agent</li>
                <li>4. View logs</li>
              </ol>
              <Link href="/services">
                <Button className="w-full" variant="secondary">
                  Get Started
                </Button>
              </Link>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
