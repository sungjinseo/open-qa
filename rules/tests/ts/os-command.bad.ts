import { exec } from "child_process";
import { execSync } from "node:child_process";
const { exec: run } = require("node:child_process");
export function ping(host: string) {
  exec("ping -c 1 " + host, (err, out) => console.warn(out));
  execSync("ping -c 1 " + host);
  require("child_process").exec("ping -c 1 " + host);
  require("node:child_process").execSync("ping -c 1 " + host);
}
