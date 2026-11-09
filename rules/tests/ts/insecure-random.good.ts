import crypto from "crypto";
export function newResetToken() {
  const resetToken = crypto.randomBytes(32).toString("hex");
  const colorCode = Math.random().toString(16);
  const statusCode = Math.floor(Math.random() * 3);
  return [resetToken, colorCode, statusCode];
}
