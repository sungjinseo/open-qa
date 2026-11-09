import crypto from "crypto";
export function newResetToken() {
  const resetToken = crypto.randomBytes(32).toString("hex");
  return resetToken;
}
