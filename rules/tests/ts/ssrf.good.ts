import type { Request, Response } from "express";
declare function assertAllowedHost(u: string): string;
export async function relative(req: Request) {
  const id = new URL(req.url).searchParams.get("id");
  return fetch(`/api/items/${id}`);
}
export async function proxy(req: Request, res: Response) {
  const target = assertAllowedHost(req.query.url as string);
  const r = await fetch(target);
  res.send(await r.text());
}
