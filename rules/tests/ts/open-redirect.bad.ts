import type { Request, Response } from "express";
export function afterLogin(req: Request, res: Response) {
  const next = req.query.next as string;
  res.redirect(next);
}
