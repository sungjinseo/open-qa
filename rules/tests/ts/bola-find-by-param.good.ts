import express from "express";
import { Order } from "./models";

const app = express();

app.get("/orders/:id", async (req, res) => {
  const order = await Order.findOne({ _id: req.params.id, owner: req.user.id });
  if (!order) return res.sendStatus(404);
  res.json(order);
});

app.delete("/posts/:id", async (req, res) => {
  await prisma.post.delete({ where: { id: req.params.id, authorId: req.user.id } });
  res.sendStatus(204);
});
