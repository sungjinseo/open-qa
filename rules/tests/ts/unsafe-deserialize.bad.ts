import serialize from "node-serialize";
export const load = (payload: string) => serialize.unserialize(payload);
