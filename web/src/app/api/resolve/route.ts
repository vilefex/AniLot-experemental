import type { NextRequest } from "next/server";
import { json } from "@/lib/server/net";
import { resolveStream } from "@/lib/server/resolve";

export const dynamic = "force-dynamic";
export const maxDuration = 60;

export async function GET(req: NextRequest): Promise<Response> {
  const url = req.nextUrl.searchParams.get("url") || "";
  try {
    return json(await resolveStream(url));
  } catch (e) {
    return json({ error: e instanceof Error ? e.message : String(e) }, 400);
  }
}
