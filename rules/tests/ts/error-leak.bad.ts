import type { Request, Response } from "express";
export function handler(req: Request, res: Response) {
  try {
    JSON.parse(req.body);
  } catch (e) {
    res.status(500).json({ error: (e as Error).stack });
  }
}
