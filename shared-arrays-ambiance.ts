// A deliberately out-of-character haunted-house ambiance for the first arrays lesson.
// It never blocks the page: a storm rolls in with distant lightning and fog, blood
// seeps from the top of the screen, things lurk and creep around the edges, and
// fragments of what's coming drift by.
// After a while the storm passes and the page goes back to normal.

const WHISPERS = [
  "int a[3];",
  "b = a;   /* error */",
  "*(a + i)",
  "2[a]",
  "decay",
  "a[3]",
  "int (*p)[3]",
  "{1, 2, 3}",
  "'\\0'",
  "&a != &a[0]?",
  "char s[3] = \"abc\";",
  "sizeof",
];

const STORM_MS = 9000;
const CLEAR_MS = 2500;
const BLEEDING = false;

function randomBetween(low: number, high: number): number {
  return low + Math.random() * (high - low);
}

// Repeats `action` at random intervals until `isOver()`, skipping while the tab is hidden.
function every(low: number, high: number, isOver: () => boolean, action: () => void): void {
  window.setTimeout(() => {
    if (isOver()) return;
    if (!document.hidden) action();
    every(low, high, isOver, action);
  }, randomBetween(low, high));
}

// Restarts a CSS animation that's keyed off a class.
function replayClass(element: HTMLElement, className: string, duration: number): void {
  element.classList.remove(className);
  void element.offsetWidth;
  element.classList.add(className);
  window.setTimeout(() => element.classList.remove(className), duration);
}

// A spot near an edge of the screen, away from the lesson in the middle.
function edgePosition(): { left: number; top: number } {
  const along = randomBetween(4, 96);
  const inward = randomBetween(3, 14);
  switch (Math.floor(Math.random() * 4)) {
    case 0: return { left: inward, top: along };
    case 1: return { left: 100 - inward, top: along };
    case 2: return { left: along, top: inward };
    default: return { left: along, top: 100 - inward };
  }
}

// Distant lightning: a faint flash, and the page trembles with the thunder.
function strikeLightning(overlay: HTMLElement): void {
  replayClass(overlay, "is-striking", 900);
  replayClass(document.body, "arrays-ambiance-shake", 500);
}

function releaseBat(overlay: HTMLElement): void {
  const bat = document.createElement("span");
  bat.className = "arrays-ambiance-bat";
  bat.textContent = "🦇";
  const leftToRight = Math.random() < 0.5;
  bat.style.setProperty("--from-x", leftToRight ? "-10vw" : "110vw");
  bat.style.setProperty("--to-x", leftToRight ? "110vw" : "-10vw");
  bat.style.setProperty("--from-y", `${randomBetween(5, 60)}vh`);
  bat.style.setProperty("--to-y", `${randomBetween(5, 60)}vh`);
  bat.style.setProperty("--flip", leftToRight ? "-1" : "1");
  bat.style.fontSize = `${randomBetween(22, 42)}px`;
  bat.style.animationDuration = `${randomBetween(3.5, 6)}s, 0.18s`;
  bat.addEventListener("animationend", (event) => {
    if (event.animationName === "arrays-ambiance-fly") bat.remove();
  });
  overlay.appendChild(bat);
}

// Glowing eyes open in the dark near an edge, blink, and are gone.
function lurk(overlay: HTMLElement): void {
  const eyes = document.createElement("div");
  eyes.className = "arrays-ambiance-eyes";
  eyes.innerHTML = "<span></span><span></span>";
  const { left, top } = edgePosition();
  eyes.style.left = `${left}%`;
  eyes.style.top = `${top}%`;
  eyes.style.setProperty("--size", `${randomBetween(7, 13)}px`);
  eyes.style.setProperty("--tilt", `${randomBetween(-12, 12)}deg`);
  eyes.addEventListener("animationend", (event) => {
    if (event.target === eyes) eyes.remove();
  });
  overlay.appendChild(eyes);
}

// A spider lowers itself from the top of the screen, has a look, and climbs back up.
function lowerSpider(overlay: HTMLElement): void {
  const spider = document.createElement("div");
  spider.className = "arrays-ambiance-spider";
  spider.innerHTML = '<span class="arrays-ambiance-spider-body">🕷️</span>';
  spider.style.left = `${randomBetween(8, 92)}%`;
  spider.style.setProperty("--drop", `${randomBetween(18, 38)}vh`);
  spider.addEventListener("animationend", (event) => {
    if (event.target === spider) spider.remove();
  });
  overlay.appendChild(spider);
}

// Blood seeps along the top edge of the screen and runs down in drips, which stay
// until the storm passes. Now and then a drop falls from the end of one.
function bleed(overlay: HTMLElement, isOver: () => boolean): void {
  const blood = document.createElement("div");
  blood.className = "arrays-ambiance-blood";
  overlay.appendChild(blood);
  const drips: HTMLElement[] = [];
  for (let i = 0; i < 11; i += 1) {
    const drip = document.createElement("span");
    drip.className = "arrays-ambiance-drip";
    drip.style.left = `${randomBetween(1, 99)}%`;
    drip.style.setProperty("--width", `${randomBetween(4, 11)}px`);
    drip.style.setProperty("--length", `${randomBetween(18, 110)}px`);
    drip.style.animationDelay = `${randomBetween(0.3, 2.5)}s`;
    drip.style.animationDuration = `${randomBetween(2.5, 5)}s`;
    blood.appendChild(drip);
    drips.push(drip);
  }
  window.setTimeout(() => {
    every(500, 1300, isOver, () => {
      const source = drips[Math.floor(Math.random() * drips.length)]!;
      if (source.offsetHeight < 12) return;
      const drop = document.createElement("span");
      drop.className = "arrays-ambiance-drop";
      drop.style.left = source.style.left;
      drop.style.top = `${source.offsetHeight}px`;
      drop.style.setProperty("--width", `${source.offsetWidth * 0.9}px`);
      drop.addEventListener("animationend", () => drop.remove());
      blood.appendChild(drop);
    });
  }, 2500);
}

function whisper(overlay: HTMLElement, index: number): void {
  const element = document.createElement("span");
  element.className = "arrays-ambiance-whisper";
  element.textContent = WHISPERS[index]!;
  const { left, top } = edgePosition();
  element.style.left = `${left}%`;
  element.style.top = `${top}%`;
  element.style.animationDuration = `${randomBetween(5, 9)}s`;
  element.style.setProperty("--drift", `${randomBetween(-24, 24)}px`);
  element.addEventListener("animationend", () => element.remove());
  overlay.appendChild(element);
}

export function startArraysAmbiance(): void {
  const calm = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const overlay = document.createElement("div");
  overlay.className = `arrays-ambiance${calm ? " is-calm" : ""}`;
  overlay.setAttribute("aria-hidden", "true");
  overlay.innerHTML = `
    <div class="arrays-ambiance-vignette"></div>
    <div class="arrays-ambiance-fog"></div>
    <div class="arrays-ambiance-noise"></div>
    <div class="arrays-ambiance-flash"></div>
  `;
  document.body.appendChild(overlay);

  // The storm passes on its own, or as soon as the learner gets on with the lesson.
  let over = false;
  const isOver = () => over;
  const clear = () => {
    if (overlay.classList.contains("is-clearing")) return;
    over = true;
    overlay.classList.add("is-clearing");
    window.setTimeout(() => overlay.remove(), CLEAR_MS);
  };
  window.setTimeout(() => (over = true), STORM_MS - 1000);
  window.setTimeout(clear, STORM_MS);
  document.querySelector('button[data-stepper="next"]')?.addEventListener("click", clear, { once: true });
  if (calm) return;

  window.setTimeout(() => !over && strikeLightning(overlay), 1500);
  if (BLEEDING) bleed(overlay, isOver);
  every(4000, 6000, isOver, () => strikeLightning(overlay));
  window.setTimeout(() => {
    if (over) return;
    releaseBat(overlay);
    every(3000, 5000, isOver, () => releaseBat(overlay));
  }, 2000);

  window.setTimeout(() => {
    if (over) return;
    lurk(overlay);
    every(1200, 2500, isOver, () => lurk(overlay));
  }, 1500);
  window.setTimeout(() => !over && lowerSpider(overlay), 2500);

  let lastWhisper = -1;
  window.setTimeout(() => {
    every(1200, 2500, isOver, () => {
      let index = Math.floor(Math.random() * WHISPERS.length);
      if (index === lastWhisper) index = (index + 1) % WHISPERS.length;
      lastWhisper = index;
      whisper(overlay, index);
    });
  }, 1500);
}
