import jwt from "jsonwebtoken";

export function issueToken(user: { id: string; role: string }) {
  return jwt.sign({ sub: user.id, role: user.role }, process.env.JWT_SECRET as string);
}
