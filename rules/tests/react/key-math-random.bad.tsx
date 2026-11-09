export const List = ({ items }: { items: string[] }) => (
  <ul>{items.map((i) => <li key={Math.random()}>{i}</li>)}</ul>
);
