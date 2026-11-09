import { createApp } from "vue";

const app = createApp({});
app.config.errorHandler = (err) => {
  console.error(err);
};
