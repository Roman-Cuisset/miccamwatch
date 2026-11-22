"use strict";

(() => {
  const key = "mcw-theme";
  const root = document.documentElement;
  const modes = ["system", "light", "dark"];
  const explicit = (value) => value === "light" || value === "dark";
  const apply = (value) => {
    if (explicit(value)) root.dataset.theme = value;
    else delete root.dataset.theme;
  };

  // This same-origin script runs before the stylesheet and the first paint.
  try { apply(localStorage.getItem(key)); } catch { /* CSS follows the system. */ }

  document.addEventListener("DOMContentLoaded", () => {
    const control = document.querySelector("[data-theme-control]");
    if (!control) return;
    const icons = [...control.querySelectorAll("[data-theme-icon]")];
    const currentMode = () => root.dataset.theme || "system";
    const render = () => {
      const mode = currentMode();
      for (const icon of icons) {
        icon.hidden = icon.dataset.themeIcon !== mode;
        if (!icon.hidden) {
          control.title = icon.dataset.label;
          control.setAttribute("aria-label", icon.dataset.label);
        }
      }
    };
    render();
    control.hidden = false;
    control.addEventListener("click", () => {
      const value = modes[(modes.indexOf(currentMode()) + 1) % modes.length];
      apply(value);
      render();
      try {
        if (explicit(value)) localStorage.setItem(key, value);
        else localStorage.removeItem(key);
      } catch { /* The selection still works on this page. */ }
    });
  }, { once: true });
})();
