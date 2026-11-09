import type { Request, Response } from "express";
export function panel(req: Request, res: Response) {
  if (req.cookies.isAdmin === "true") {
    res.send("admin panel");
  }
}
