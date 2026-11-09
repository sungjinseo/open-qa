import fs from "fs";
import path from "path";
import type { Request, Response } from "express";
export function download(req: Request, res: Response) {
  const name = path.basename(req.query.file as string);
  res.send(fs.readFileSync(path.join("/data/uploads", name)));
}
