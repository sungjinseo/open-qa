import jwt from "jsonwebtoken";
export const userId = (token: string) =>
  (jwt.verify(token, process.env.JWT_SECRET as string, { algorithms: ["HS256"] }) as { uid: string }).uid;
