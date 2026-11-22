"use strict";

(() => {
  const key = "mcw-theme";
  const root = document.documentElement;
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
    control.value = root.dataset.theme || "system";
    control.closest(".theme-control").hidden = false;
    control.addEventListener("change", () => {
      const value = control.value;
      apply(value);
      try {
        if (explicit(value)) localStorage.setItem(key, value);
        else localStorage.removeItem(key);
      } catch { /* The selection still works on this page. */ }
    });
  }, { once: true });
})();
