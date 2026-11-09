import ldap from "ldapjs";
const client = ldap.createClient({ url: "ldaps://ldap.example.com" });
export function find(uid: string) {
  client.search("ou=people,dc=example,dc=com", { filter: `(uid=${uid})`, scope: "sub" }, () => {});
}
