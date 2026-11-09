import dns from "dns";
export function isInternal(ip: string, cb: (ok: boolean) => void) {
  dns.reverse(ip, (_e, names) => cb((names ?? []).some((n) => n.endsWith(".corp.local"))));
}
