import type { Request, Response } from "express";
export function updateSettings(req: Request, res: Response) {
  const settings: Record<string, any> = {};
  for (const key in req.body) {
    settings[key] = req.body[key];
  }
  res.json(settings);
}
