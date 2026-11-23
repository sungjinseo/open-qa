import express from "express";
import { User } from "./models";

const app = express();
app.use(express.json());

app.put("/users/:id", async (req, res) => {
  const { name, email } = req.body;
  const user = await User.findByIdAndUpdate(req.params.id, { name, email }, { new: true });
  res.json(user);
});

app.post("/users", async (req, res) => {
  const input = UserCreate.parse(req.body);
  res.json(await User.create({ name: input.name, email: input.email }));
});
