import type { Response } from "express";
export function login(res: Response, sid: string) {
  res.cookie("sid", sid, { httpOnly: false });
}
