import { defineComponent } from "vue";

export default defineComponent({
  props: { count: { type: Number, required: true } },
  emits: ["update:count"],
  setup(props, { emit }) {
    const inc = () => {
      emit("update:count", props.count + 1);
    };
    return { inc };
  },
});
