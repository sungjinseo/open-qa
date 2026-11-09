import { defineComponent } from "vue";

export default defineComponent({
  props: { label: String },
  template: "<span>{{ label }}</span>",
});
