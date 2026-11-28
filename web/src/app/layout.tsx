import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import Shell from "@/components/Shell";
import "./globals.css";

export const metadata: Metadata = {
  title: "AniLot — аниме онлайн",
  description: "AniLot: каталог аниме Anixart со встроенным плеером",
};

export const viewport: Viewport = { themeColor: "#0e0e10" };

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="ru">
      <body className="antialiased">
        <Shell>{children}</Shell>
      </body>
    </html>
  );
}
