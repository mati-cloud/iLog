"use client";

import {
  FileText,
  Key,
  MoreHorizontal,
  Plus,
  Server,
  Settings,
  Trash2,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { type Column, DataTable } from "@/components/DataTable";
import { EmptyState } from "@/components/EmptyState";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { Textarea } from "@/components/ui/textarea";
import { useSession } from "@/lib/auth-client";

interface Service {
  id: string;
  name: string;
  description?: string;
  owner_id: string;
  created_at: string;
  updated_at: string;
}

export default function ServicesPage() {
  const router = useRouter();
  const { data: session, isPending } = useSession();
  const [services, setServices] = useState<Service[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newService, setNewService] = useState({
    name: "",
    description: "",
  });

  const serviceColumns: Column<Service>[] = [
    {
      key: "service",
      label: "Service",
      width: "w-[40%]",
      render: (service) => (
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/10 rounded-lg">
            <Server className="h-4 w-4 text-primary" />
          </div>
          <div className="flex flex-col">
            <span className="font-medium">{service.name}</span>
            {service.description && (
              <span className="text-sm text-muted-foreground line-clamp-1">
                {service.description}
              </span>
            )}
          </div>
        </div>
      ),
    },
    {
      key: "status",
      label: "Status",
      render: () => (
        <Badge variant="outline" className="gap-1.5">
          <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
          Active
        </Badge>
      ),
    },
    {
      key: "created",
      label: "Created",
      render: (service) => (
        <span className="text-muted-foreground">
          {new Date(service.created_at).toLocaleDateString("en-US", {
            month: "short",
            day: "numeric",
            year: "numeric",
          })}
        </span>
      ),
    },
    {
      key: "actions",
      label: "Actions",
      align: "right",
      render: (service) => (
        <div className="flex items-center justify-end gap-2">
          <Button
            variant="ghost"
            size="sm"
            className="gap-2 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={() => router.push(`/logs?service=${service.id}`)}
          >
            <FileText className="h-4 w-4" />
            Logs
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="gap-2 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={() => router.push(`/services/${service.id}/tokens`)}
          >
            <Key className="h-4 w-4" />
            Tokens
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                onClick={() => router.push(`/services/${service.id}/settings`)}
              >
                <Settings className="mr-2 h-4 w-4" />
                Settings
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={() => deleteService(service.id)}
                className="text-destructive focus:text-destructive"
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Delete Service
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      ),
    },
  ];

  const fetchServices = useCallback(async () => {
    try {
      const response = await fetch("/api/proxy/services");
      if (response.status === 401) {
        console.error("Unauthorized - redirecting to login");
        router.push("/login");
        return;
      }
      if (response.ok) {
        const data = await response.json();
        setServices(data);
      } else {
        console.error(
          "Failed to fetch services:",
          response.status,
          await response.text(),
        );
      }
    } catch (error) {
      console.error("Failed to fetch services:", error);
    } finally {
      setLoading(false);
    }
  }, [router]);

  useEffect(() => {
    if (!isPending && !session) {
      router.push("/login");
      return;
    }
    if (session) {
      fetchServices();
    }
  }, [session, isPending, router, fetchServices]);

  const createService = async () => {
    try {
      const response = await fetch("/api/proxy/services", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(newService),
      });

      if (response.ok) {
        setCreateDialogOpen(false);
        setNewService({ name: "", description: "" });
        fetchServices();
      }
    } catch (error) {
      console.error("Failed to create service:", error);
    }
  };

  const deleteService = async (id: string) => {
    if (!confirm("Are you sure you want to delete this service?")) return;

    try {
      const response = await fetch(`/api/proxy/services/${id}`, {
        method: "DELETE",
      });

      if (response.ok) {
        fetchServices();
      }
    } catch (error) {
      console.error("Failed to delete service:", error);
    }
  };

  if (isPending) {
    return (
      <div className="container mx-auto py-8 px-4">
        <div className="text-center py-12">
          <p className="text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full px-6 py-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex justify-between items-start mb-8">
          <div className="space-y-1">
            <h1 className="text-3xl font-bold tracking-tight">Services</h1>
            <p className="text-muted-foreground">
              Manage your log collection services and access tokens
            </p>
          </div>
          <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
            <DialogTrigger asChild>
              <Button size="default" className="gap-2">
                <Plus className="h-4 w-4" />
                New Service
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Create New Service</DialogTitle>
                <DialogDescription>
                  Create a new service to organize your logs
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-4 py-4">
                <div className="space-y-2">
                  <Label htmlFor="name">Service Name</Label>
                  <Input
                    id="name"
                    placeholder="My Application"
                    value={newService.name}
                    onChange={(e) => {
                      setNewService({
                        ...newService,
                        name: e.target.value,
                      });
                    }}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="description">Description (Optional)</Label>
                  <Textarea
                    id="description"
                    placeholder="Production application logs"
                    value={newService.description}
                    onChange={(e) =>
                      setNewService({
                        ...newService,
                        description: e.target.value,
                      })
                    }
                  />
                </div>
              </div>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() => setCreateDialogOpen(false)}
                >
                  Cancel
                </Button>
                <Button onClick={createService}>Create Service</Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <p className="text-muted-foreground">Loading services...</p>
          </div>
        ) : services.length === 0 ? (
          <EmptyState
            icon={Server}
            title="No services yet"
            description="Get started by creating your first service to organize and manage your log streams"
            actionLabel="Create Your First Service"
            onAction={() => setCreateDialogOpen(true)}
          />
        ) : (
          <DataTable
            columns={serviceColumns}
            data={services}
            getRowKey={(service) => service.id}
          />
        )}
      </div>
    </div>
  );
}
