// proc website: copy buttons, heading anchors, sidebar scroll tracking, and the target reader on the home page.
(function () {
  "use strict";

  function copyText(text, button) {
    var done = function () {
      var label = button.textContent;
      button.textContent = "Copied";
      button.classList.add("is-copied");
      setTimeout(function () {
        button.textContent = label;
        button.classList.remove("is-copied");
      }, 1600);
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () {});
    }
  }

  // Copy buttons on every code block (and the home page install line)
  function addCopyButtons() {
    document.querySelectorAll(".prose pre, .term pre").forEach(function (pre) {
      var holder = pre.closest(".highlighter-rouge") || pre.parentElement;
      if (holder.querySelector(".copy-button")) return;
      var button = document.createElement("button");
      button.type = "button";
      button.className = "copy-button";
      button.textContent = "Copy";
      button.setAttribute("aria-label", "Copy code");
      button.addEventListener("click", function () {
        copyText(pre.innerText.replace(/\n$/, ""), button);
      });
      holder.style.position = "relative";
      holder.appendChild(button);
    });
    document.querySelectorAll("[data-copy]").forEach(function (button) {
      button.addEventListener("click", function () {
        copyText(button.getAttribute("data-copy"), button);
      });
    });
  }

  // "#" links on headings that have ids, so sections can be shared
  function addHeadingAnchors() {
    document.querySelectorAll(".prose h2[id], .prose h3[id]").forEach(function (h) {
      var a = document.createElement("a");
      a.className = "heading-anchor";
      a.href = "#" + h.id;
      a.textContent = "#";
      a.setAttribute("aria-label", "Link to this section");
      h.appendChild(a);
    });
  }

  // Sidebars with data-spy mark the section being read, and keep that link in view
  function setupScrollSpy() {
    var nav = document.querySelector(".docs-nav[data-spy]");
    if (!nav) return;
    var aside = nav.closest(".page-aside");
    var links = {};
    nav.querySelectorAll('a[href^="#"]').forEach(function (a) {
      links[decodeURIComponent(a.getAttribute("href").slice(1))] = a;
    });
    var headings = Array.prototype.filter.call(document.querySelectorAll(".prose h2[id]"), function (h) {
      return links[h.id];
    });
    if (!headings.length) return;
    var current = null;
    function mark(id) {
      if (id === current) return;
      if (current && links[current]) links[current].removeAttribute("aria-current");
      current = id;
      var a = links[id];
      a.setAttribute("aria-current", "location");
      // scroll the sidebar itself, never the page
      if (aside && (a.offsetTop < aside.scrollTop || a.offsetTop + a.offsetHeight > aside.scrollTop + aside.clientHeight)) {
        aside.scrollTop = a.offsetTop - aside.clientHeight / 3;
      }
    }
    // the current section is the last heading that has scrolled past the top fifth of the screen;
    // at the very bottom it's the last one, since short final sections never reach that line
    function update() {
      var id = headings[0].id;
      var bottom = window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2;
      if (bottom) {
        id = headings[headings.length - 1].id;
      } else {
        var line = window.innerHeight / 5;
        for (var i = 0; i < headings.length; i++) {
          if (headings[i].getBoundingClientRect().top <= line) id = headings[i].id;
          else break;
        }
      }
      mark(id);
    }
    var queued = false;
    window.addEventListener("scroll", function () {
      if (queued) return;
      queued = true;
      requestAnimationFrame(function () {
        queued = false;
        update();
      });
    }, { passive: true });
    window.addEventListener("hashchange", update);
    update();
  }

  // The target reader: shows how proc reads each kind of target
  function setupReader() {
    var reader = document.querySelector("[data-reader]");
    if (!reader) return;
    var verbEl = reader.querySelector("[data-reader-verb]");
    var targetEl = reader.querySelector("[data-reader-target]");
    var explainEl = reader.querySelector("[data-reader-explain]");
    var buttons = Array.prototype.slice.call(reader.querySelectorAll("[data-target]"));
    var current = 0;
    var timer = null;

    function show(i) {
      current = i;
      var b = buttons[i];
      verbEl.textContent = b.getAttribute("data-verb");
      targetEl.textContent = b.getAttribute("data-target");
      explainEl.replaceChildren(b.querySelector("template").content.cloneNode(true));
      buttons.forEach(function (other, j) {
        other.setAttribute("aria-pressed", j === i ? "true" : "false");
      });
    }

    function stop() {
      if (timer) { clearInterval(timer); timer = null; }
    }

    buttons.forEach(function (b, i) {
      b.addEventListener("click", function () { stop(); show(i); });
    });
    reader.addEventListener("keydown", function (e) {
      if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
      stop();
      var next = (current + (e.key === "ArrowRight" ? 1 : buttons.length - 1)) % buttons.length;
      show(next);
      buttons[next].focus();
    });

    show(0);

    // Step through the examples once, slowly, unless motion is reduced
    var calm = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!calm) {
      var steps = 0;
      timer = setInterval(function () {
        steps += 1;
        if (steps >= buttons.length) { stop(); show(0); return; }
        show(steps);
      }, 3600);
      ["pointerdown", "keydown", "focusin"].forEach(function (evt) {
        reader.addEventListener(evt, stop, { once: true });
      });
    }
  }

  document.addEventListener("DOMContentLoaded", function () {
    addCopyButtons();
    addHeadingAnchors();
    setupScrollSpy();
    setupReader();
  });
})();
