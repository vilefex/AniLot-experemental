"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState, type ReactNode } from "react";
import { CalendarDays, Clapperboard, Library, LogIn, Search, Settings, UserRound } from "lucide-react";
import { useAuth } from "@/lib/client";

const NAV = [
  { href: "/", label: "Каталог", icon: Clapperboard },
  { href: "/schedule", label: "Расписание", icon: CalendarDays },
  { href: "/library", label: "Библиотека", icon: Library },
];

export default function Shell({ children }: { children: ReactNode }) {
  const path = usePathname();
  const router = useRouter();
  const auth = useAuth();
  const [q, setQ] = useState("");
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const on = () => setScrolled(window.scrollY > 8);
    on();
    window.addEventListener("scroll", on, { passive: true });
    return () => window.removeEventListener("scroll", on);
  }, []);

  // Плеер занимает весь экран и рисует собственный интерфейс.
  if (path.startsWith("/watch/")) return <>{children}</>;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (q.trim()) router.push(`/search?q=${encodeURIComponent(q.trim())}`);
  };

  return (
    <div className="min-h-screen">
      <header
        className={`sticky top-0 z-40 transition-colors ${scrolled ? "border-b border-line bg-bg/90 backdrop-blur-md" : "bg-transparent"}`}
      >
        <div className="mx-auto flex h-14 max-w-[1500px] items-center gap-3 px-[4%]">
          <Link href="/" className="mr-2 flex items-center gap-2 text-xl font-black tracking-tight">
            <span className="grid h-8 w-8 place-items-center rounded-lg bg-accent text-black">A</span>
            <span className="hidden sm:inline">
              Ani<span className="text-accent">Lot</span>
            </span>
          </Link>

          <nav className="flex items-center gap-1">
            {NAV.map((n) => {
              const on = n.href === "/" ? path === "/" : path.startsWith(n.href);
              return (
                <Link
                  key={n.href}
                  href={n.href}
                  className={`flex h-9 items-center gap-2 rounded-lg px-3 text-sm font-medium transition ${on ? "bg-accent-soft text-accent" : "text-white/70 hover:bg-white/5 hover:text-white"}`}
                >
                  <n.icon size={18} />
                  <span className="hidden md:inline">{n.label}</span>
                </Link>
              );
            })}
          </nav>

          <form onSubmit={submit} className="relative ml-auto w-full max-w-md">
            <Search size={17} className="absolute left-3 top-1/2 -translate-y-1/2 text-white/40" />
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Поиск аниме…"
              className="input !h-9 !rounded-full !pl-10"
            />
          </form>

          <Link
            href="/settings"
            title="Настройки"
            className={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition hover:bg-white/10 ${path === "/settings" ? "text-accent" : "text-white/70"}`}
          >
            <Settings size={19} />
          </Link>
          {auth.token ? (
            <Link href="/login" title={auth.login} className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-accent-soft text-accent">
              <UserRound size={19} />
            </Link>
          ) : (
            <Link href="/login" className="btn btn-primary !h-9 !px-4">
              <LogIn size={16} />
              <span className="hidden sm:inline">Войти</span>
            </Link>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-[1500px] px-[4%] pb-16 pt-4">{children}</main>
    </div>
  );
}
