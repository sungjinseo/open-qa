import type { Request, Response } from "express";
export function handler(req: Request, res: Response) {
  try {
    JSON.parse(req.body);
  } catch (e) {
    console.error(e);
    res.status(500).json({ error: "internal error" });
  }
}
