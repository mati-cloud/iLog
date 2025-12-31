"use client";

import { Geist, Geist_Mono } from "next/font/google";
import { usePathname } from "next/navigation";
import "./globals.css";
import { Footer } from "@/components/Footer";
import { LayoutContent } from "@/components/LayoutContent";
import { Sidebar } from "@/components/Sidebar";
import { ThemeProvider } from "@/components/theme-provider";
import { useSession } from "@/lib/auth-client";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
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
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          {showSidebar && <Sidebar />}
          <LayoutContent showSidebar={showSidebar}>
            <main className="min-h-screen">{children}</main>
            <Footer />
          </LayoutContent>
        </ThemeProvider>
      </body>
    </html>
  );
}
