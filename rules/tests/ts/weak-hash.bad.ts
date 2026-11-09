import crypto from "crypto";
export const fingerprint = (s: string) => crypto.createHash("md5").update(s).digest("hex");
