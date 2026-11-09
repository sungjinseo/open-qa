import cors from "cors";
import express from "express";
const app = express();
app.use(cors({ origin: ["https://app.example.com"] }));
