document.addEventListener("DOMContentLoaded", () => {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Reveal-on-scroll
  const revealEls = document.querySelectorAll(".reveal");
  if (!("IntersectionObserver" in window) || reduced) {
    revealEls.forEach((el) => el.classList.add("visible"));
  } else {
    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add("visible");
            io.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.12, rootMargin: "0px 0px -40px 0px" }
    );
    revealEls.forEach((el) => io.observe(el));
  }

  // Active nav-link highlighting
  const sections = [...document.querySelectorAll("main section[id]")];
  const navLinks = [...document.querySelectorAll(".nav-links a")];
  const textColor = getComputedStyle(document.body)
    .getPropertyValue("--text")
    .trim();
  const dimColor = getComputedStyle(document.body)
    .getPropertyValue("--text-dim")
    .trim();

  const setActive = (id) => {
    navLinks.forEach((link) => {
      link.style.color =
        link.getAttribute("href") === `#${id}` ? textColor : dimColor;
    });
  };

  if ("IntersectionObserver" in window && sections.length) {
    const navObserver = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) setActive(entry.target.id);
        });
      },
      { rootMargin: "-45% 0px -50% 0px", threshold: 0 }
    );
    sections.forEach((s) => navObserver.observe(s));
  }

  // Elevate the sticky nav once you scroll past the hero
  const nav = document.querySelector("#nav");
  if (nav) {
    const onScroll = () => {
      nav.style.borderBottomColor =
        window.scrollY > 24
          ? "var(--border-strong)"
          : "var(--border)";
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
  }
});
