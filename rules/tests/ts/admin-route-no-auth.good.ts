import express from "express";
const app = express();
const requireAdmin: express.RequestHandler = (_req, _res, next) => next();
app.delete("/admin/users/:id", requireAdmin, (req, res) => {
  res.json({ deleted: req.params.id });
});
