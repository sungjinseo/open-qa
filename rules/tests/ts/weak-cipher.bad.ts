import crypto from "crypto";
export function enc(key: Buffer, data: string) {
  const c = crypto.createCipheriv("aes-128-ecb", key, null);
  return Buffer.concat([c.update(data), c.final()]);
}
