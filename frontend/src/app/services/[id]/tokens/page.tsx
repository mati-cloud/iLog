"use client";

import {
  ArrowLeft,
  CheckCircle2,
  Copy,
  Eye,
  EyeOff,
  Key,
  MoreHorizontal,
  Plus,
  Trash2,
} from "lucide-react";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { type Column, DataTable } from "@/components/DataTable";
import { EmptyState } from "@/components/EmptyState";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface ServiceToken {
  id: string;
  service_id: string;
  name: string;
  token: string;
  source_type?: string;
  metadata?: Record<string, unknown>;
  expires_at?: string;
  last_used_at?: string;
  created_at: string;
}

interface Service {
  id: string;
  name: string;
  slug: string;
}

export default function TokensPage() {
  const params = useParams();
  const router = useRouter();
  const serviceId = params.id as string;

  const [service, setService] = useState<Service | null>(null);
  const [tokens, setTokens] = useState<ServiceToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [confirmCloseDialog, setConfirmCloseDialog] = useState(false);
  const [visibleTokens, setVisibleTokens] = useState<Set<string>>(new Set());
  const [newToken, setNewToken] = useState({
    name: "",
    source_type: "all",
    expires_in_days: "",
  });
  const [createdToken, setCreatedToken] = useState<ServiceToken | null>(null);

  const hasChanges =
    newToken.name !== "" ||
    newToken.source_type !== "all" ||
    newToken.expires_in_days !== "";

  const handleCloseDialog = () => {
    if (hasChanges) {
      setConfirmCloseDialog(true);
    } else {
      setCreateDialogOpen(false);
    }
  };

  const confirmDiscard = () => {
    setNewToken({ name: "", source_type: "all", expires_in_days: "" });
    setCreateDialogOpen(false);
    setConfirmCloseDialog(false);
  };

  const toggleTokenVisibility = (tokenId: string) => {
    setVisibleTokens((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(tokenId)) {
        newSet.delete(tokenId);
      } else {
        newSet.add(tokenId);
      }
      return newSet;
    });
  };

  const copyToken = (token: string) => {
    navigator.clipboard.writeText(token);
  };

  const tokenColumns: Column<ServiceToken>[] = [
    {
      key: "name",
      label: "Token Name",
      width: "w-[20%]",
      render: (token) => (
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/10 rounded-lg">
            <Key className="h-4 w-4 text-primary" />
          </div>
          <div className="flex flex-col">
            <span className="font-medium">{token.name}</span>
            {token.source_type && token.source_type !== "all" && (
              <span className="text-xs text-muted-foreground">
                {token.source_type}
              </span>
            )}
          </div>
        </div>
      ),
    },
    {
      key: "token",
      label: "Token",
      width: "w-[25%]",
      render: (token) => {
        const isVisible = visibleTokens.has(token.id);
        const displayToken = isVisible ? token.token : "••••••••••••••••";

        return (
          <div className="flex items-center gap-2">
            <code className="text-xs bg-muted px-2 py-1 rounded font-mono">
              {displayToken.substring(0, 20)}
              {displayToken.length > 20 ? "..." : ""}
            </code>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleTokenVisibility(token.id)}
              className="h-7 w-7 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
            >
              {isVisible ? (
                <EyeOff className="h-3.5 w-3.5" />
              ) : (
                <Eye className="h-3.5 w-3.5" />
              )}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => copyToken(token.token)}
              className="h-7 w-7 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
          </div>
        );
      },
    },
    {
      key: "source_type",
      label: "Source Type",
      render: (token) => (
        <Badge variant={token.source_type === "all" ? "secondary" : "default"}>
          {token.source_type || "all"}
        </Badge>
      ),
    },
    {
      key: "last_used",
      label: "Last Used",
      render: (token) => (
        <span className="text-muted-foreground text-sm">
          {token.last_used_at
            ? new Date(token.last_used_at).toLocaleDateString("en-US", {
                month: "short",
                day: "numeric",
                year: "numeric",
              })
            : "Never"}
        </span>
      ),
    },
    {
      key: "expires",
      label: "Expires",
      render: (token) =>
        token.expires_at ? (
          <Badge variant="outline">
            {new Date(token.expires_at).toLocaleDateString("en-US", {
              month: "short",
              day: "numeric",
              year: "numeric",
            })}
          </Badge>
        ) : (
          <Badge>Never</Badge>
        ),
    },
    {
      key: "actions",
      label: "Actions",
      align: "right",
      render: (token) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => copyToken(token.token)}>
              <Copy className="mr-2 h-4 w-4" />
              Copy Token
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onClick={() => revokeToken(token.id)}
              className="text-destructive focus:text-destructive"
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Revoke Token
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ];

  const fetchService = useCallback(async () => {
    try {
      const response = await fetch(`/api/proxy/services/${serviceId}`);
      if (response.ok) {
        const data = await response.json();
        setService(data);
      }
    } catch (error) {
      console.error("Failed to fetch service:", error);
    }
  }, [serviceId]);

  const fetchTokens = useCallback(async () => {
    try {
      const response = await fetch(`/api/proxy/services/${serviceId}/agents`);
      if (response.ok) {
        const data = await response.json();
        setTokens(data);
      }
    } catch (error) {
      console.error("Failed to fetch tokens:", error);
    } finally {
      setLoading(false);
    }
  }, [serviceId]);

  useEffect(() => {
    fetchService();
    fetchTokens();
  }, [fetchService, fetchTokens]);

  const createToken = async () => {
    try {
      const payload: {
        name: string;
        source_type: string;
        expires_in_days?: number;
      } = {
        name: newToken.name,
        source_type: newToken.source_type,
      };
      if (newToken.expires_in_days) {
        payload.expires_in_days = parseInt(newToken.expires_in_days, 10);
      }

      const response = await fetch(`/api/proxy/services/${serviceId}/agents`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(payload),
      });

      if (response.ok) {
        const token = await response.json();
        setCreatedToken(token);
        setNewToken({ name: "", source_type: "all", expires_in_days: "" });
        setCreateDialogOpen(false);
        await fetchTokens();
      } else {
        const errorText = await response.text();
        console.error("Failed to create token:", response.status, errorText);
        alert(`Failed to create token: ${response.status} ${errorText}`);
      }
    } catch (error) {
      console.error("Failed to create token:", error);
      alert(`Failed to create token: ${error}`);
    }
  };

  const revokeToken = async (tokenId: string) => {
    if (!confirm("Are you sure you want to revoke this token?")) return;

    try {
      const response = await fetch(
        `/api/proxy/services/${serviceId}/agents/${tokenId}`,
        {
          method: "DELETE",
        },
      );

      if (response.ok) {
        fetchTokens();
      }
    } catch (error) {
      console.error("Failed to revoke token:", error);
    }
  };

  const _formatDate = (dateString?: string) => {
    if (!dateString) return "Never";
    return new Date(dateString).toLocaleString();
  };

  return (
    <div className="w-full px-6 py-8">
      <div className="max-w-7xl mx-auto">
        <div className="mb-6">
          <Button variant="ghost" onClick={() => router.push("/services")}>
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Services
          </Button>
        </div>

        <div className="flex justify-between items-start mb-8">
          <div className="space-y-1">
            <h1 className="text-3xl font-bold tracking-tight">
              {service?.name || "Service"} Tokens
            </h1>
            <p className="text-muted-foreground">
              Manage API tokens for log ingestion.
            </p>
          </div>
          <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
            <DialogTrigger asChild>
              <Button>
                <Plus className="mr-2 h-4 w-4" />
                New Token
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Create New Token</DialogTitle>
                <DialogDescription>
                  Generate a new token for the iLog agent
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-4 py-4">
                <div className="space-y-2">
                  <Label htmlFor="token-name">Token Name</Label>
                  <Input
                    id="token-name"
                    placeholder="production-server-1"
                    value={newToken.name}
                    onChange={(e) =>
                      setNewToken({ ...newToken, name: e.target.value })
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="source-type">Source Type</Label>
                  <select
                    id="source-type"
                    className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    value={newToken.source_type}
                    onChange={(e) =>
                      setNewToken({ ...newToken, source_type: e.target.value })
                    }
                  >
                    <option value="all">All Sources</option>
                    <option value="docker">Docker</option>
                    <option value="file">File</option>
                    <option value="journald">Journald/Systemd</option>
                    <option value="http">HTTP</option>
                  </select>
                  <p className="text-xs text-muted-foreground">
                    Restrict this token to a specific log source type
                  </p>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="expires">Expires In (Days, Optional)</Label>
                  <Input
                    id="expires"
                    type="number"
                    placeholder="365"
                    value={newToken.expires_in_days}
                    onChange={(e) =>
                      setNewToken({
                        ...newToken,
                        expires_in_days: e.target.value,
                      })
                    }
                  />
                  <p className="text-xs text-muted-foreground">
                    Leave empty for no expiration
                  </p>
                </div>
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={handleCloseDialog}>
                  Cancel
                </Button>
                <Button onClick={createToken}>Generate Token</Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        {createdToken && (
          <Card className="mb-6 border-green-500 py-6">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <CheckCircle2 className="h-5 w-5 text-green-500" />
                Token Created Successfully
              </CardTitle>
              <CardDescription>
                Copy this token now - you won&apos;t be able to see it again!
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="relative">
                <div className="bg-muted p-4 rounded-md font-mono text-sm break-all pr-12">
                  {createdToken.token}
                </div>
                <div className="absolute top-2 right-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => copyToken(createdToken.token)}
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>
              </div>
              <div className="mt-4 p-4 bg-muted rounded-md">
                <p className="text-sm font-medium mb-2">Agent Configuration:</p>
                <pre className="text-xs bg-background p-3 rounded border overflow-x-auto">
                  {`[agent]
server = "your-ilog-server.com:8080"
token = "${createdToken.token}"`}
                </pre>
              </div>
            </CardContent>
          </Card>
        )}

        {loading ? (
          <div className="text-center py-12">
            <p className="text-muted-foreground">Loading tokens...</p>
          </div>
        ) : tokens.length === 0 ? (
          <EmptyState
            icon={Key}
            title="No tokens yet"
            description="Create access tokens to authenticate your log agents and applications"
            actionLabel="Create Your First Token"
            onAction={() => setCreateDialogOpen(true)}
          />
        ) : (
          <DataTable
            columns={tokenColumns}
            data={tokens}
            getRowKey={(token) => token.id}
          />
        )}

        <ConfirmDialog
          open={confirmCloseDialog}
          onOpenChange={setConfirmCloseDialog}
          title="Discard changes?"
          description="You have unsaved changes. Are you sure you want to discard them?"
          confirmLabel="Discard Changes"
          cancelLabel="Continue Editing"
          onConfirm={confirmDiscard}
          destructive
        />
      </div>
    </div>
  );
}
