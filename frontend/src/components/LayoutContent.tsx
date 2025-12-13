
import { useSidebar } from "@/components/Sidebar";
import { cn } from "@/lib/utils";

export function LayoutContent({ children }: { children: React.ReactNode }) {
  const { collapsed } = useSidebar();

  return (
    <div
      className={cn(
        "transition-all duration-500 ease-in-out",
        collapsed ? "pl-16" : "pl-64",
      )}
    >
      {children}
    </div>
  );
}
