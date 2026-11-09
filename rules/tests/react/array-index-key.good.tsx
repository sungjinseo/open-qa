export function List({ items }: { items: { id: string; name: string }[] }) {
  return <ul>{items.map((item) => <li key={item.id}>{item.name}</li>)}</ul>;
}
