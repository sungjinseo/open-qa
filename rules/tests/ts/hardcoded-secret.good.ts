const dbPassword = process.env.DB_PASSWORD ?? "";
const passwordLabel = "Password";
const envName = { passwordKey: "DB_PASSWORD" };
export const form = { password: "Password", secret: "short", apiKey: "API_KEY_NAME", p2: "auth.password", p3: "password-required", p4: "password_field", p5: "Password:", p6: "비밀번호를입력하세요" };
export const cfg = { host: "db.internal", dbPassword, passwordLabel, envName };
