import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({ history: createWebHistory(), routes: [] });
router.beforeEach((to, from, next) => {
  next();
});
export default router;
