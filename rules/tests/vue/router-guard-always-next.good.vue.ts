import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({ history: createWebHistory(), routes: [] });
router.beforeEach((to, from, next) => {
  if (to.meta.requiresAuth && !localStorage.getItem("session")) {
    next("/login");
  } else {
    next();
  }
});
export default router;
