import express from "express";
import { User } from "./models";

const app = express();
app.use(express.json());

app.put("/users/:id", async (req, res) => {
  const user = await User.findByIdAndUpdate(req.params.id, req.body, { new: true });
  res.json(user);
});
