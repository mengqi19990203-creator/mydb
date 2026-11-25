import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";
import { initTheme } from "./lib/theme";

initTheme();
createApp(App).mount("#app");
