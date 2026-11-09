import type { Request, Response } from "express";
declare function safeRedirectTarget(next: string): string;
export function afterLogin(req: Request, res: Response) {
  const next = req.query.next as string;
  res.redirect(safeRedirectTarget(next));
}
export function fromMap(res: Response) {
  const routes = new Map<string, string>([["home", "/home"]]);
  res.redirect(routes.get("home") as string);
}
