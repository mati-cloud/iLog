"use client";

import { AlertCircle, Check, Loader2 } from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

interface Service {
  id: string;
  name: string;
  description?: string;
}

interface ServiceSelectorProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onServiceSelect?: (serviceId: string) => void;
}

export function ServiceSelector({
  open,
  onOpenChange,
  onServiceSelect,
}: ServiceSelectorProps) {
  const router = useRouter();
  const [services, setServices] = useState<Service[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedService, setSelectedService] = useState<string>("");
  const [showNoServicesAlert, setShowNoServicesAlert] = useState(false);
  const [understood, setUnderstood] = useState(false);

  const fetchServices = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/proxy/services", {
        credentials: "include",
      });
      if (response.ok) {
        const data = await response.json();
        setServices(data);

        // If no services exist, show alert
        if (data.length === 0) {
          setShowNoServicesAlert(true);
        }
      }
    } catch (error) {
      console.error("Failed to fetch services:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      fetchServices();
    }
  }, [open, fetchServices]);

  const handleSelect = (serviceId: string) => {
    setSelectedService(serviceId);

    if (onServiceSelect) {
      onServiceSelect(serviceId);
    } else {
      router.push(`/logs?service=${serviceId}`);
    }

    onOpenChange(false);
  };

  const handleNoServicesConfirm = () => {
    if (understood) {
      router.push("/services");
    }
  };

  // No services alert - unskippable with checkbox
  if (showNoServicesAlert) {
    return (
      <AlertDialog open={true} onOpenChange={() => {}}>
        <AlertDialogContent className="max-w-md">
          <AlertDialogHeader>
            <div className="flex items-center gap-3 mb-2">
              <div className="p-3 bg-amber-500/10 rounded-lg">
                <AlertCircle className="h-6 w-6 text-amber-500" />
              </div>
              <AlertDialogTitle>No Services Found</AlertDialogTitle>
            </div>
            <AlertDialogDescription className="text-left space-y-3">
              <p>
                You need to create a service before you can view logs. A service
                helps organize and manage your log streams.
              </p>
              <p className="font-medium">Steps to get started:</p>
              <ol className="list-decimal list-inside space-y-1 text-sm">
                <li>Create a new service</li>
                <li>Generate an access token</li>
                <li>Configure your log agent</li>
                <li>Start streaming logs</li>
              </ol>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex items-center space-x-2 py-4">
            <Checkbox
              id="understood"
              checked={understood}
              onCheckedChange={(checked) => setUnderstood(checked === true)}
            />
            <Label
              htmlFor="understood"
              className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer"
            >
              I understand and want to create a service
            </Label>
          </div>
          <AlertDialogFooter>
            <AlertDialogAction
              onClick={handleNoServicesConfirm}
              disabled={!understood}
            >
              Go to Services
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    );
  }

  // Service selector dialog - unskippable (can't close without selecting)
  return (
    <Dialog open={open} onOpenChange={() => {}}>
      <DialogContent
        className="max-w-md"
        onInteractOutside={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>Select a Service</DialogTitle>
          <DialogDescription>
            Choose a service to view its logs.
          </DialogDescription>
        </DialogHeader>
        <Command className="rounded-lg border shadow-md">
          <CommandInput placeholder="Search services..." />
          <CommandList>
            {loading ? (
              <div className="flex items-center justify-center py-6">
                <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
              </div>
            ) : (
              <>
                <CommandEmpty>No services found.</CommandEmpty>
                <CommandGroup>
                  {services.map((service) => (
                    <CommandItem
                      key={service.id}
                      value={service.id}
                      onSelect={() => handleSelect(service.id)}
                      className="cursor-pointer"
                    >
                      <Check
                        className={cn(
                          "mr-2 h-4 w-4",
                          selectedService === service.id
                            ? "opacity-100"
                            : "opacity-0",
                        )}
                      />
                      <div className="flex flex-col">
                        <span className="font-medium">{service.name}</span>
                        {service.description && (
                          <span className="text-xs text-muted-foreground">
                            {service.description}
                          </span>
                        )}
                      </div>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </>
            )}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
