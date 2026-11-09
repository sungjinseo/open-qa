import libxmljs from "libxmljs";
export function parse(xml: string) {
  return libxmljs.parseXml(xml, { noent: true });
}
