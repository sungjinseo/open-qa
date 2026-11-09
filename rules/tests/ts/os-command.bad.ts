import { exec } from "child_process";
export function ping(host: string) {
  exec("ping -c 1 " + host, (err, out) => console.warn(out));
}
