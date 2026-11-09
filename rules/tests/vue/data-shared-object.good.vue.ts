export default {
  name: "Profile",
  props: {
    data: { type: Array, default: () => [] },
  },
  data() {
    return { user: "" };
  },
};
