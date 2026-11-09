import { SignJWT } from "jose";
export const issue = (uid: string) =>
  new SignJWT({ uid }).setProtectedHeader({ alg: "HS256" }).sign(new TextEncoder().encode("my-super-secret"));
