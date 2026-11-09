export function Delete({ remove }: { remove: (id: number) => void }) {
  return <button onClick={() => remove(1)}>delete</button>;
}
