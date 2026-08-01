import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import "@/index.css";
import { AppProvider } from "@/lib/app-context";

const container = document.getElementById("root");

// Light is product default and design-review baseline. Theme controls may swap
// this to `dark`; every component uses shadcn semantic tokens, never raw theme colors.
document.documentElement.classList.remove("dark");
document.documentElement.classList.add("light");

if (!container) {
  throw new Error("#root missing from index.html");
}

createRoot(container).render(
  <StrictMode>
    <AppProvider>
      <App />
    </AppProvider>
  </StrictMode>,
);
