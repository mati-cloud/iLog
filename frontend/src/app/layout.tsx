"use client";

import { Geist_Mono, Martian_Mono, Schibsted_Grotesk } from "next/font/google";
import { usePathname } from "next/navigation";
import "./globals.css";
import { Footer } from "@/components/Footer";
import { LayoutContent } from "@/components/LayoutContent";
import { Sidebar } from "@/components/Sidebar";
import { ThemeProvider } from "@/components/theme-provider";
import { useSession } from "@/lib/auth-client";

const ui = Schibsted_Grotesk({
  variable: "--font-ui",
  subsets: ["latin"],
});

// Only the query bar uses this; its wide, even glyphs make syntax legible.
const query = Martian_Mono({
  variable: "--font-query",
  subsets: ["latin"],
  weight: ["400", "500"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});


export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const pathname = usePathname();
  const { data: session } = useSession();
  const isLoginPage = pathname === "/login";
  const showSidebar = !!session && !isLoginPage;

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <title>iLog - OpenTelemetry Logging System</title>
        <meta name="description" content="Real-time log streaming and analysis" />
        <script src="/runtime-config.js" />
      </head>
      <body
        className={`${ui.variable} ${query.variable} ${geistMono.variable} font-sans antialiased`}
      >
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          {showSidebar && <Sidebar />}
          <LayoutContent showSidebar={showSidebar}>
            <main className="min-h-[calc(100dvh-57px)]">{children}</main>
            <Footer />
          </LayoutContent>
        </ThemeProvider>
      </body>
    </html>
  );
}
