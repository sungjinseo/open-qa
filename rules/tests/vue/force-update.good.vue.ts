export default {
  data() {
    return { tick: 0 };
  },
  methods: {
    refresh() {
      this.tick += 1;
    },
  },
};
