export function sendToken(frame: Window, token: string) {
  frame.postMessage({ token }, "https://app.example.com");
}
