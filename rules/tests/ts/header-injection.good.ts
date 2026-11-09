import type { Request, Response } from "express";
export function lang(req: Request, res: Response) {
  res.setHeader("X-Lang", encodeURIComponent(req.query.lang as string));
  res.end();
}
