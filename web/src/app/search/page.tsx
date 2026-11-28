"use client";

import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import PagedList from "@/components/PagedList";
import { ax } from "@/lib/client";

function Inner() {
  const q = useSearchParams().get("q") || "";
  return (
    <div>
      <h1 className="mb-4 text-xl font-bold">
        Результаты поиска: <span className="text-accent">{q}</span>
      </h1>
      {q ? (
        <PagedList resetKey={q} load={(page) => ax(`search/releases/${page}`, { json: { query: q, searchBy: 0, search_by: 0 } })} />
      ) : (
        <div className="py-16 text-center text-white/45">Введите запрос в строке поиска</div>
      )}
    </div>
  );
}

export default function SearchPage() {
  return (
    <Suspense>
      <Inner />
    </Suspense>
  );
}
