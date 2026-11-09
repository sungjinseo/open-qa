const apiKey = process.env.REACT_APP_API_KEY ?? "";
const label = "인증";
export function headers() {
  return { Authorization: apiKey, "X-Label": label };
}
