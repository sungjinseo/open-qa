export const load = (token: string) => fetch(`/api/items?token=${token}`);
