import type { Request, Response } from "express";
declare function assertAllowedHost(u: string): string;
export async function proxy(req: Request, res: Response) {
  const target = assertAllowedHost(req.query.url as string);
  const r = await fetch(target);
  res.send(await r.text());
}
