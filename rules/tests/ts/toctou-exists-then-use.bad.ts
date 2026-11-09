import fs from "fs";
export function save(p: string, data: string) {
  if (!fs.existsSync(p)) {
    fs.writeFileSync(p, data);
  }
}
