// Set the current year in the footer.
document.getElementById("year").textContent = new Date().getFullYear();

// Reveal-on-scroll animation for sections.
const revealEls = document.querySelectorAll(".section, .masonry, .hero");

const observer = new IntersectionObserver(
    (entries) => {
        for (const entry of entries) {
            if (entry.isIntersecting) {
                entry.target.classList.add("is-visible");
                observer.unobserve(entry.target);
            }
        }
    },
    { threshold: 0.12 }
);

if ("IntersectionObserver" in window) {
    revealEls.forEach((el) => {
        el.classList.add("will-reveal");
        observer.observe(el);
    });
}
