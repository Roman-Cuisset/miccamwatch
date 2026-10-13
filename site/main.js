"use strict";

// The immutable HTTPS command in HTML remains usable without JavaScript.
const installerCommand = document.querySelector("[data-installer-command]");
const installerNote = document.getElementById("installer-origin-note");
if (installerCommand && window.location.protocol === "https:") {
  const installerUrl = new URL("/install.sh", window.location.origin);
  installerUrl.search = "";
  installerUrl.hash = "";
  installerCommand.textContent = `curl --proto '=https' --tlsv1.2 -fsSL ${installerUrl.href} | sh`;
  if (installerNote) {
    installerNote.textContent = "Cette commande utilise l’endpoint HTTPS install.sh de ce site, qui sert le script revu. Les exécutables et leurs sommes sont téléchargés depuis GitHub. L’alternative ci-dessous reste épinglée à la source immuable de la release.";
  }
}

// Copy is offered only when the browser exposes the secure Clipboard API.
// A failed permission request is reported; there is no silent fallback copy.
const copyStatus = document.getElementById("copy-status");
if (window.isSecureContext && navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
  document.querySelectorAll("[data-copy]").forEach((button) => {
    const code = document.getElementById(button.dataset.copy);
    if (!code) return;
    button.hidden = false;
    button.addEventListener("click", async () => {
      button.disabled = true;
      if (copyStatus) copyStatus.textContent = "";
      try {
        await navigator.clipboard.writeText(code.textContent.trim());
        if (copyStatus) copyStatus.textContent = "Le contenu de ce bloc a été copié dans le presse-papiers.";
      } catch {
        const selection = window.getSelection();
        if (selection) {
          const range = document.createRange();
          range.selectNodeContents(code);
          selection.removeAllRanges();
          selection.addRange(range);
        }
        if (copyStatus) copyStatus.textContent = "La copie a échoué. Le texte du bloc est sélectionné si le navigateur le permet ; copiez-le manuellement avec Ctrl+C, Cmd+C ou le menu de sélection.";
      } finally {
        button.disabled = false;
      }
    });
  });
}

// Without enhancement all OS sections are visible and the controls are links.
const osNavigation = document.querySelector(".os-tabs");
if (osNavigation) {
  const tabs = [...osNavigation.querySelectorAll("[data-os]")];
  const panels = tabs.map((tab) => document.getElementById(tab.dataset.os));
  if (tabs.length && panels.every(Boolean)) {
    const selectTab = (index, focus = false) => {
      tabs.forEach((tab, tabIndex) => {
        const selected = tabIndex === index;
        tab.setAttribute("aria-selected", String(selected));
        tab.tabIndex = selected ? 0 : -1;
        panels[tabIndex].hidden = !selected;
      });
      if (focus) tabs[index].focus();
    };
    const indexFromHash = () => tabs.findIndex((tab) => `#${tab.dataset.os}` === window.location.hash);
    const platform = navigator.userAgentData?.platform || navigator.platform || "";
    const preferredOs = /mac/i.test(platform) ? "macos" : /linux/i.test(platform) ? "linux" : "windows";
    const hashIndex = indexFromHash();
    const preferredIndex = tabs.findIndex((tab) => tab.dataset.os === preferredOs);
    osNavigation.setAttribute("role", "tablist");
    tabs.forEach((tab, index) => {
      tab.id = `${tab.dataset.os}-tab`;
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-controls", panels[index].id);
      panels[index].setAttribute("role", "tabpanel");
      panels[index].setAttribute("aria-labelledby", tab.id);
      panels[index].tabIndex = 0;
      tab.addEventListener("click", (event) => {
        event.preventDefault();
        selectTab(index);
      });
      tab.addEventListener("keydown", (event) => {
        let nextIndex;
        if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
        else if (event.key === "ArrowLeft") nextIndex = (index + tabs.length - 1) % tabs.length;
        else if (event.key === "Home") nextIndex = 0;
        else if (event.key === "End") nextIndex = tabs.length - 1;
        else if (event.key === " ") nextIndex = index;
        else return;
        event.preventDefault();
        selectTab(nextIndex, true);
      });
    });
    selectTab(hashIndex >= 0 ? hashIndex : Math.max(0, preferredIndex));
    window.addEventListener("hashchange", () => {
      const index = indexFromHash();
      if (index >= 0) selectTab(index);
    });
  }
}

// Browsers may otherwise omit closed disclosure contents when printing.
let printDisclosureStates = [];
window.addEventListener("beforeprint", () => {
  printDisclosureStates = [...document.querySelectorAll("details")].map((element) => ({ element, open: element.open }));
  printDisclosureStates.forEach(({ element }) => { element.open = true; });
});
window.addEventListener("afterprint", () => {
  printDisclosureStates.forEach(({ element, open }) => { element.open = open; });
  printDisclosureStates = [];
});
