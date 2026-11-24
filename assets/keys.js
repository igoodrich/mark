// Keyboard handling and the hooks Mark's Rust side calls to update the page
// in place, so live reloads and theme switches keep the scroll position.
(() => {
  const content = document.getElementById("content");
  const theme = document.getElementById("theme");
  const send = (msg) => window.ipc.postMessage(msg);

  window.markUpdate = (html) => { content.innerHTML = html; };
  window.markTheme = (css) => { theme.textContent = css; };

  const keys = [
    ["j / k", "Scroll down / up"],
    ["d / u", "Half page down / up"],
    ["Space / b", "Page down / up"],
    ["g / G", "Top / bottom"],
    ["+ / - / 0", "Zoom in / out / reset"],
    ["r", "Reload"],
    ["?", "Show shortcuts"],
    ["q", "Quit"],
  ];
  const help = document.createElement("div");
  help.id = "mark-help";
  help.innerHTML = "<div><table>" +
    keys.map(([k, d]) => `<tr><td><kbd>${k}</kbd></td><td>${d}</td></tr>`).join("") +
    "</table></div>";
  document.body.appendChild(help);
  help.addEventListener("click", () => help.classList.remove("open"));

  let zoom = 1;
  const setZoom = (z) => {
    zoom = Math.min(3, Math.max(0.5, Math.round(z * 10) / 10));
    document.body.style.setProperty("--zoom", zoom);
  };

  const line = 60;
  const page = () => window.innerHeight * 0.9;
  const by = (y) => window.scrollBy({ top: y, behavior: "instant" });

  document.addEventListener("keydown", (e) => {
    if (e.altKey || e.metaKey) return;
    if (e.ctrlKey) {
      if (e.key === "d") by(page() / 2);
      else if (e.key === "u") by(-page() / 2);
      else if (e.key === "=" || e.key === "+") setZoom(zoom + 0.1);
      else if (e.key === "-") setZoom(zoom - 0.1);
      else if (e.key === "0") setZoom(1);
      else if (e.key === "q" || e.key === "w") send("quit");
      else return;
      e.preventDefault();
      return;
    }
    switch (e.key) {
      case "j": case "ArrowDown": by(line); break;
      case "k": case "ArrowUp": by(-line); break;
      case "d": by(page() / 2); break;
      case "u": by(-page() / 2); break;
      case " ": case "PageDown": case "f": by(e.shiftKey ? -page() : page()); break;
      case "b": case "PageUp": by(-page()); break;
      case "g": case "Home": window.scrollTo(0, 0); break;
      case "G": case "End": window.scrollTo(0, document.body.scrollHeight); break;
      case "+": case "=": setZoom(zoom + 0.1); break;
      case "-": setZoom(zoom - 0.1); break;
      case "0": setZoom(1); break;
      case "r": send("reload"); break;
      case "?": help.classList.toggle("open"); break;
      case "Escape": help.classList.remove("open"); break;
      case "q": send("quit"); break;
      default: return;
    }
    e.preventDefault();
  });
})();
