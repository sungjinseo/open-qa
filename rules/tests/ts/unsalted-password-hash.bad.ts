import crypto from "crypto";
export const storePw = (password: string) => crypto.createHash("sha256").update(password).digest("hex");
