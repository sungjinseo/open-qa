import type { Request, Response } from "express";
export function lang(req: Request, res: Response) {
  res.setHeader("X-Lang", req.query.lang as string);
  res.end();
}
