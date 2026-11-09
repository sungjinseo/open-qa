export const load = (token: string) => fetch("/api/items", { headers: { Authorization: `Bearer ${token}` } });
