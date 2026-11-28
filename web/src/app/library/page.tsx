"use client";

import { useState } from "react";
import Link from "next/link";
import PagedList from "@/components/PagedList";
import { ax, LIST_STATUSES, useAuth } from "@/lib/client";

const TABS = [
  { id: "fav", name: "Избранное" },
  { id: "hist", name: "История" },
  ...LIST_STATUSES.map((s) => ({ id: String(s.id), name: s.name })),
];

export default function LibraryPage() {
  const auth = useAuth();
  const [tab, setTab] = useState("fav");

  if (!auth.token) {
    return (
      <div className="mx-auto mt-16 max-w-md rounded-2xl border border-line bg-surface p-8 text-center">
        <h1 className="text-xl font-bold">Библиотека</h1>
        <p className="mt-2 text-sm text-white/55">Избранное, история и списки доступны после входа в аккаунт Anixart.</p>
        <Link href="/login" className="btn btn-primary mt-5">
          Войти
        </Link>
      </div>
    );
  }

  const load = (page: number) => {
    if (tab === "fav") return ax(`favorite/all/${page}`);
    if (tab === "hist") return ax(`history/${page}`);
    return ax(`profile/list/all/${tab}/${page}`, { query: { sort: 1 } });
  };

  return (
    <div>
      <h1 className="mb-4 text-xl font-bold">Библиотека</h1>
      <div className="mb-5 flex flex-wrap gap-2">
        {TABS.map((t) => (
          <button key={t.id} className={`chip chip-btn ${tab === t.id ? "chip-on" : ""}`} onClick={() => setTab(t.id)}>
            {t.name}
          </button>
        ))}
      </div>
      <PagedList resetKey={tab + auth.token} load={load} empty="Здесь пока пусто" />
    </div>
  );
}
