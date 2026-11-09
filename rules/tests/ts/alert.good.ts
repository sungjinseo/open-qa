export function notify(msg: string) {
  return { type: "toast", msg };
}
