import type { Request, Response } from "express";
export function panel(req: Request, res: Response) {
  if (req.session.user?.role === "admin") {
    res.send("admin panel");
  }
}
