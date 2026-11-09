import { createApp } from "vue";

const app = createApp({});
app.config.devtools = import.meta.env.DEV;
