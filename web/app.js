(() => {
  const root = document.documentElement;
  const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
  let preference;
  try {
    preference = localStorage.getItem("worktree-theme");
  } catch {}
  if (!["light", "dark"].includes(preference)) preference = null;

  function setTheme(theme) {
    root.dataset.theme = theme;
    const button = document.querySelector(".theme-toggle");
    if (button)
      button.setAttribute(
        "aria-label",
        `Switch to ${theme === "dark" ? "light" : "dark"} theme`,
      );
  }
  setTheme(preference || (systemTheme.matches ? "dark" : "light"));
  systemTheme.addEventListener("change", (event) => {
    if (!preference) setTheme(event.matches ? "dark" : "light");
  });

  document.addEventListener("DOMContentLoaded", () => {
    const themeButton = document.querySelector(".theme-toggle");
    themeButton.hidden = false;
    setTheme(root.dataset.theme);
    themeButton.addEventListener("click", () => {
      preference = root.dataset.theme === "dark" ? "light" : "dark";
      setTheme(preference);
      try {
        localStorage.setItem("worktree-theme", preference);
      } catch {}
    });
    if (!navigator.clipboard) return;
    for (const button of document.querySelectorAll("[data-copy]")) {
      button.hidden = false;
      button.addEventListener("click", async () => {
        const status = document.querySelector(".copy-status");
        try {
          await navigator.clipboard.writeText(
            document.getElementById(button.dataset.copy).textContent.trim(),
          );
          status.textContent = "Copied to clipboard.";
        } catch {
          status.textContent =
            "Could not copy. Select and copy the command above.";
        }
      });
    }
  });
})();
