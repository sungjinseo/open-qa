import { SignJWT } from "jose";
export const issue = (uid: string) =>
  new SignJWT({ uid }).setProtectedHeader({ alg: "HS256" }).sign(new TextEncoder().encode(process.env.JWT_SECRET as string));
export const digest = (s: string) => crypto.subtle.digest("SHA-256", new TextEncoder().encode("hello"));
