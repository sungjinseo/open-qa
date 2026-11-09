import fs from "fs";
import type { Request, Response } from "express";
export function download(req: Request, res: Response) {
  const name = req.query.file as string;
  res.send(fs.readFileSync("/data/uploads/" + name));
}
