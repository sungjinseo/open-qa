const dbPassword = process.env.DB_PASSWORD ?? "";
const passwordLabel = "Password";
const envName = { passwordKey: "DB_PASSWORD" };
export const form = { password: "Password", secret: "short", apiKey: "API_KEY_NAME" };
export const cfg = { host: "db.internal", dbPassword, passwordLabel, envName };
