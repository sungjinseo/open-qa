import jwt from "jsonwebtoken";
export const userId = (token: string) => (jwt.decode(token) as { uid: string }).uid;
