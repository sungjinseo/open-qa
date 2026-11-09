import crypto from "crypto";
export const fingerprint = (s: string) => crypto.createHash("sha256").update(s).digest("hex");
