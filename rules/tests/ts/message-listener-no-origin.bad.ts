window.addEventListener("message", (e) => {
  document.title = String(e.data);
});
