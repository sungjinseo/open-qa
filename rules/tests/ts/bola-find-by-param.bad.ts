import express from "express";
import { Order } from "./models";

const app = express();

app.get("/orders/:id", async (req, res) => {
  const order = await Order.findById(req.params.id);
  res.json(order);
});
