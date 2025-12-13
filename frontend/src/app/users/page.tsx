
import { Mail, Plus, Shield, Users as UsersIcon } from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { type Column, DataTable } from "@/components/DataTable";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useSession } from "@/lib/auth-client";

interface User {
  id: string;
  name: string;
  email: string;
  role: string;
  created_at: string;
  email_verified: boolean;
}

export default function UsersPage() {
  const router = useRouter();
  const { data: session, isPending } = useSession();
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);

  const userColumns: Column<User>[] = [
    {
      key: "user",
      label: "User",
      width: "w-[35%]",
      render: (user) => (
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/10 rounded-lg">
            <UsersIcon className="h-4 w-4 text-primary" />
          </div>
          <div className="flex flex-col">
            <span className="font-medium">{user.name}</span>
            <span className="text-sm text-muted-foreground">{user.email}</span>
          </div>
        </div>
      ),
    },
    {
      key: "role",
      label: "Role",
      render: (user) => (
        <Badge variant="outline" className="gap-1.5">
          <Shield className="h-3 w-3" />
          {user.role || "User"}
        </Badge>
      ),
    },
    {
      key: "status",
      label: "Status",
      render: (user) => (
        <Badge
          variant={user.email_verified ? "default" : "secondary"}
          className="gap-1.5"
        >
          <Mail className="h-3 w-3" />
          {user.email_verified ? "Verified" : "Unverified"}
        </Badge>
      ),
    },
    {
      key: "joined",
      label: "Joined",
      render: (user) => (
        <span className="text-muted-foreground">
          {new Date(user.created_at).toLocaleDateString("en-US", {
            month: "short",
            day: "numeric",
            year: "numeric",
          })}
        </span>
      ),
    },
  ];

  const fetchUsers = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/proxy/users");
      if (response.ok) {
        const data = await response.json();
        setUsers(data);
      }
    } catch (error) {
      console.error("Failed to fetch users:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isPending && !session) {
      router.push("/login");
      return;
    }
    if (session) {
      fetchUsers();
    }
  }, [session, isPending, router, fetchUsers]);

  if (isPending || loading) {
    return (
      <div className="w-full px-6 py-8">
        <div className="max-w-7xl mx-auto">
          <div className="text-center py-12">
            <p className="text-muted-foreground">Loading...</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full px-6 py-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex justify-between items-start mb-8">
          <div className="space-y-1">
            <h1 className="text-3xl font-bold tracking-tight">Users</h1>
            <p className="text-muted-foreground">
              Manage user accounts and permissions.
            </p>
          </div>
          <Button size="default" className="gap-2">
            <Plus className="h-4 w-4" />
            Invite User
          </Button>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <p className="text-muted-foreground">Loading users...</p>
          </div>
        ) : users.length === 0 ? (
          <Card className="border-dashed">
            <CardContent className="flex flex-col items-center justify-center py-16 text-center">
              <div className="rounded-full bg-muted p-4 mb-4">
                <UsersIcon className="h-8 w-8 text-muted-foreground" />
              </div>
              <h3 className="text-lg font-semibold mb-2">No users yet</h3>
              <p className="text-sm text-muted-foreground mb-6 max-w-sm">
                Invite team members to collaborate on your logging platform
              </p>
              <Button size="lg">
                <Plus className="mr-2 h-4 w-4" />
                Invite Your First User
              </Button>
            </CardContent>
          </Card>
        ) : (
          <DataTable
            columns={userColumns}
            data={users}
            getRowKey={(user) => user.id}
          />
        )}
      </div>
    </div>
  );
}
