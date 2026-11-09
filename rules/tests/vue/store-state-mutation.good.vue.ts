export default {
  methods: {
    login(name: string) {
      this.$store.commit("setUser", name);
    },
  },
};
