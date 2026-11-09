export default {
  methods: {
    login(name: string) {
      this.$store.state.user = name;
    },
  },
};
