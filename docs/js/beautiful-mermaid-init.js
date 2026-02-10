(function () {
  "use strict";

  function pickTheme(bm) {
    if (!bm || !bm.THEMES) return null;
    return (
      bm.THEMES["dracula"] ||
      bm.THEMES["tokyo-night"] ||
      bm.THEMES.default ||
      null
    );
  }

  async function renderAll() {
    var bm = window.beautifulMermaid;
    if (!bm || typeof bm.renderMermaid !== "function") return;

    var theme = pickTheme(bm);
    var blocks = document.querySelectorAll(
      "pre.mermaid, pre > code.language-mermaid, code.mermaid"
    );

    for (var i = 0; i < blocks.length; i += 1) {
      var node = blocks[i];
      var codeEl = node.tagName === "CODE" ? node : node.querySelector("code");
      if (!codeEl) continue;

      var source = codeEl.textContent || "";
      if (!source.trim()) continue;

      try {
        var svg = theme
          ? await bm.renderMermaid(source, theme)
          : await bm.renderMermaid(source);
        var wrapper = document.createElement("div");
        wrapper.className = "mermaid";
        wrapper.innerHTML = svg;

        var pre = node.tagName === "PRE" ? node : node.parentElement;
        if (pre && pre.parentElement) {
          pre.parentElement.replaceChild(wrapper, pre);
        } else if (node.parentElement) {
          node.parentElement.replaceChild(wrapper, node);
        }
      } catch (err) {
        // Leave the original code block in place on error.
        console.error("Beautiful Mermaid render failed", err);
      }
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      renderAll();
    });
  } else {
    renderAll();
  }
})();
