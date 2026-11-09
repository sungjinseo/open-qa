import type { Request, Response } from "express";
export async function proxy(req: Request, res: Response) {
  const target = req.query.url as string;
  const r = await fetch(target);
  res.send(await r.text());
}
export async function nextProxy(req: NextRequest) {
  const target = req.nextUrl.searchParams.get("url") as string;
  return fetch(target);
}
