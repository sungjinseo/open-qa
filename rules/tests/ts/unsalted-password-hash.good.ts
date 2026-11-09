import bcrypt from "bcrypt";
export const storePw = (password: string) => bcrypt.hash(password, 12);
