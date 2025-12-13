"use client";

import {
  Copy,
  Eye,
  EyeOff,
  Key,
  MoreHorizontal,
  Plus,
  Trash2,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { type Column, DataTable } from "@/components/DataTable";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useSession } from "@/lib/auth-client";

interface Token {
  id: string;
  name: string;
  service_name: string;
  token: string;
  created_at: string;
  last_used?: string;
}

export default function TokensPage() {
  const router = useRouter();
  const { data: session, isPending } = useSession();
  const [tokens, setTokens] = useState<Token[]>([]);
  const [loading, setLoading] = useState(true);
  const [visibleTokens, setVisibleTokens] = useState<Set<string>>(new Set());

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

  const deleteToken = async (tokenId: string) => {
    if (
      !confirm(
        "Are you sure you want to delete this token? This action cannot be undone.",
      )
    ) {
      return;
    }
    try {
      await fetch(`/api/proxy/agents/${tokenId}`, {
        method: "DELETE",
        credentials: "include",
      });
      fetchTokens();
    } catch (error) {
      console.error("Failed to delete token:", error);
    }
  };

  const tokenColumns: Column<Token>[] = [
    {
      key: "name",
      label: "Token Name",
      width: "w-[25%]",
      render: (token) => (
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/10 rounded-lg">
            <Key className="h-4 w-4 text-primary" />
          </div>
          <div className="flex flex-col">
            <span className="font-medium">{token.name}</span>
            <span className="text-sm text-muted-foreground">
              {token.service_name}
            </span>
          </div>
        </div>
      ),
    },
    {
      key: "token",
      label: "Token",
      width: "w-[30%]",
      render: (token) => {
        const isVisible = visibleTokens.has(token.id);
        const displayToken = isVisible ? token.token : "••••••••••••••••";

        return (
          <div className="flex items-center gap-2">
            <code className="text-xs bg-muted px-2 py-1 rounded font-mono">
              {displayToken}
            </code>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleTokenVisibility(token.id)}
              className="h-7 w-7 p-0"
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
              className="h-7 w-7 p-0"
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
          </div>
        );
      },
    },
    {
      key: "created",
      label: "Created",
      render: (token) => (
        <span className="text-muted-foreground">
          {new Date(token.created_at).toLocaleDateString("en-US", {
            month: "short",
            day: "numeric",
            year: "numeric",
          })}
        </span>
      ),
    },
    {
      key: "last_used",
      label: "Last Used",
      render: (token) => (
        <span className="text-muted-foreground">
          {token.last_used
            ? new Date(token.last_used).toLocaleDateString("en-US", {
                month: "short",
                day: "numeric",
                year: "numeric",
              })
            : "Never"}
        </span>
      ),
    },
    {
      key: "actions",
      label: "Actions",
      align: "right",
      render: (token) => (
        <div className="flex items-center justify-end gap-2">
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
                onClick={() => deleteToken(token.id)}
                className="text-destructive focus:text-destructive"
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Delete Token
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      ),
    },
  ];

  const fetchTokens = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/proxy/agents", {
        credentials: "include",
      });
      if (response.ok) {
        const data = await response.json();
        setTokens(data);
      }
    } catch (error) {
      console.error("Failed to fetch tokens:", error);
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
      fetchTokens();
    }
  }, [session, isPending, router, fetchTokens]);

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
            <h1 className="text-3xl font-bold tracking-tight">Access Tokens</h1>
            <p className="text-muted-foreground">
              Manage API tokens for your services
            </p>
          </div>
          <Button size="default" className="gap-2">
            <Plus className="h-4 w-4" />
            New Token
          </Button>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <p className="text-muted-foreground">Loading tokens...</p>
          </div>
        ) : tokens.length === 0 ? (
          <EmptyState
            icon={Key}
            title="No tokens yet"
            description="Create access tokens to authenticate your log agents and applications"
            actionLabel="Create Your First Token"
            onAction={() => {}}
          />
        ) : (
          <DataTable
            columns={tokenColumns}
            data={tokens}
            getRowKey={(token) => token.id}
          />
        )}
      </div>
    </div>
  );
}
