window.addEventListener("message", (e) => {
  if (e.origin !== "https://app.example.com") {
    return;
  }
  document.title = String(e.data);
});
