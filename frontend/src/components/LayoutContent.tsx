"use client";

import { useSidebar } from "@/components/Sidebar";
import { cn } from "@/lib/utils";

export function LayoutContent({ 
  children,
  showSidebar = true 
}: { 
  children: React.ReactNode;
  showSidebar?: boolean;
}) {
  const { collapsed } = useSidebar();

  return (
    <div
      className={cn(
        "transition-all duration-500 ease-in-out",
        showSidebar ? (collapsed ? "pl-16" : "pl-64") : "pl-0",
      )}
    >
      {children}
    </div>
  );
}
