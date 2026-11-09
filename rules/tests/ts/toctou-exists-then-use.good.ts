import fs from "fs";
export function save(p: string, data: string) {
  try {
    fs.writeFileSync(p, data, { flag: "wx" });
  } catch (e) {
    console.warn("already exists", e);
  }
}
