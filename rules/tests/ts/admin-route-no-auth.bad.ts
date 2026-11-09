import express from "express";
const app = express();
app.delete("/admin/users/:id", (req, res) => {
  res.json({ deleted: req.params.id });
});
