import { defineComponent } from "vue";

export default defineComponent({
  props: { count: { type: Number, required: true } },
  setup(props) {
    const inc = () => {
      props.count = props.count + 1;
    };
    return { inc };
  },
});
