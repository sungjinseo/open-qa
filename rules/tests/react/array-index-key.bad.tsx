export function List({ items }: { items: string[] }) {
  return <ul>{items.map((item, i) => <li key={i}>{item}</li>)}</ul>;
}
