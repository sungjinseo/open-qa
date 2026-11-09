import type { Request, Response } from "express";
declare function safeRedirectTarget(next: string): string;
export function afterLogin(req: Request, res: Response) {
  const next = req.query.next as string;
  res.redirect(safeRedirectTarget(next));
}
