import { execFile } from "child_process";
export function ping(host: string) {
  execFile("ping", ["-c", "1", host], (err, out) => console.warn(out));
}
