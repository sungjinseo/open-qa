import jwt from "jsonwebtoken";

export function issueToken(user: { id: string; role: string }) {
  return jwt.sign({ sub: user.id, role: user.role }, process.env.JWT_SECRET as string, { expiresIn: "15m" });
}

export function issueShort(userId: string) {
  return jwt.sign({ sub: userId, exp: Math.floor(Date.now() / 1000) + 900 }, process.env.JWT_SECRET as string);
}
