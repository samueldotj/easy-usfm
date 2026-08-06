import { mount } from "svelte";

import "./app.css";
import "./styles/codemirror.css";
// Last, so its overrides win on order as well as on specificity (PRODUCT 8).
import "./styles/print.css";
import App from "./App.svelte";

// Applies the stored theme before the first paint, so a dark-theme user does
// not see a white flash on every launch.
import "./lib/theme.svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("#app is missing from index.html");
}

export default mount(App, { target });
