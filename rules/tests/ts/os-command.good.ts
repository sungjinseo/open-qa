import { execFile } from "child_process";
export function ping(host: string) {
  execFile("ping", ["-c", "1", host], (err, out) => console.warn(out));
}
export function firstNumber(line: string) {
  const re = /(\d+)/;
  const m = re.exec(line);
  return m ? /x/.exec(m[0]) : null;
}
