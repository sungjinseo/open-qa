export default {
  props: ["count"],
  methods: {
    inc() {
      this.$props.count = this.$props.count + 1;
    },
  },
};
