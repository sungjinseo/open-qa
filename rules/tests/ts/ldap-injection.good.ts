import ldap from "ldapjs";
const client = ldap.createClient({ url: "ldaps://ldap.example.com" });
const escapeLdap = (s: string) => s.replace(/[()*\\]/g, (c) => "\\" + c.charCodeAt(0).toString(16));
export function find(uid: string) {
  const filter = "(uid=" + escapeLdap(uid) + ")";
  client.search("ou=people,dc=example,dc=com", { filter, scope: "sub" }, () => {});
}
