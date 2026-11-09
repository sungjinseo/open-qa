import crypto from "crypto";
export const keys = crypto.generateKeyPairSync("rsa", { modulusLength: 3072 });
