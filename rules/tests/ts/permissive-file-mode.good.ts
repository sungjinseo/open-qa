import fs from "fs";
export const prepare = (dir: string) => fs.chmodSync(dir, 0o750);
