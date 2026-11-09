import jwt from "jsonwebtoken";
import crypto from "crypto";
export const issue = (uid: string) => jwt.sign({ uid }, process.env.JWT_SECRET as string, { expiresIn: "1h" });
export const sig = (pem: string) => crypto.createSign("RSA-SHA256").sign(pem, "base64");
export const digest = (s: string) => crypto.subtle.digest("SHA-256", new TextEncoder().encode("hello"));
