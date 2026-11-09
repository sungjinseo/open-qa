import jwt from "jsonwebtoken";
export const issue = (uid: string) => jwt.sign({ uid }, "my-super-secret", { expiresIn: "1h" });
