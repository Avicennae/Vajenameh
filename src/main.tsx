import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { getApi } from "./api";
import "./styles.css";
import { DEFAULT_SETTINGS } from "./types";
import type { Bootstrap } from "./types";

async function start() {
  const api = await getApi();
  let boot: Bootstrap;
  try {
    boot = await api.bootstrap();
  } catch (e) {
    boot = { info: null, dbError: String(e), settings: DEFAULT_SETTINGS, history: [], favorites: [], warnings: [] };
  }
  // تم را پیش از اولین رندر اعمال کن تا چشمک (flash) نداشته باشیم
  if (boot.settings.theme !== "system") document.documentElement.setAttribute("data-theme", boot.settings.theme);
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App api={api} boot={boot} />
    </StrictMode>,
  );
}

void start();
