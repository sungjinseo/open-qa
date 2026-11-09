import jwt from "jsonwebtoken";
export const issue = (uid: string) => jwt.sign({ uid }, process.env.JWT_SECRET as string, { expiresIn: "1h" });
