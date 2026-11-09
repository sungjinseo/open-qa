import crypto from "crypto";
export function enc(key: Buffer, iv: Buffer, data: string) {
  const c = crypto.createCipheriv("aes-256-gcm", key, iv);
  return Buffer.concat([c.update(data), c.final()]);
}
