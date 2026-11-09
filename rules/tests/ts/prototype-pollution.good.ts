import type { Request, Response } from "express";
export function updateSettings(req: Request, res: Response) {
  const settings: Record<string, any> = {};
  for (const key in req.body) {
    if (key === "__proto__" || key === "constructor") continue;
    settings[key] = req.body[key];
  }
  res.json(settings);
}
export function copyLocal(src: Record<string, number>) {
  const out: Record<string, number> = {};
  for (const k in src) {
    out[k] = src[k];
  }
  return out;
}
