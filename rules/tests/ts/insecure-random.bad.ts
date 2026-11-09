export function newResetToken() {
  const resetToken = Math.random().toString(36).slice(2);
  return resetToken;
}
