import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

async function startApplication(): Promise<void> {
  if (import.meta.env.VITE_WDIO === "true") {
    await import("@wdio/tauri-plugin");
  }

  createApp(App).mount("#app");
}

void startApplication();
