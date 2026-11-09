export default {
  props: ["count"],
  methods: {
    inc() {
      this.$emit("update:count", this.$props.count + 1);
    },
  },
};
