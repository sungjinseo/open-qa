export const List = ({ items }: { items: string[] }) => (
  <ul>{items.map((i) => <li key={i}>{i}</li>)}</ul>
);
