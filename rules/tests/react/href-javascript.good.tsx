export function Link({ onOpen }: { onOpen: () => void }) {
  return <button type="button" onClick={onOpen}>열기</button>;
}
