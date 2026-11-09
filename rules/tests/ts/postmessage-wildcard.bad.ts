export function sendToken(frame: Window, token: string) {
  frame.postMessage({ token }, "*");
}
