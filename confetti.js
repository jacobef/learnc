(function (global) {
  const COLORS = [
    "#f94144",
    "#f3722c",
    "#f9c74f",
    "#90be6d",
    "#43aa8b",
    "#577590",
    "#277da1",
  ];

  function makePiece() {
    return {
      x: Math.random(),
      y: -0.2 - Math.random() * 0.8,
      size: 10 + Math.random() * 8,
      fall: 0.6 + Math.random() * 0.9,
      drift: (Math.random() - 0.5) * 0.4,
      rot: Math.random() * Math.PI * 2,
      rotSpeed: (Math.random() - 0.5) * 0.2,
      color: COLORS[Math.floor(Math.random() * COLORS.length)],
    };
  }

  function confettiRain({ count = 120, duration = 2000 } = {}) {
    const canvas = document.createElement("canvas");
    canvas.className = "confetti-canvas";
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
    const ctx = canvas.getContext("2d");
    document.body.appendChild(canvas);

    const pieces = Array.from({ length: count }, makePiece);
    let last = null;
    let rafId = null;

    function resize() {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    }

    function tick(ts) {
      if (!last) last = ts;
      const dt = Math.min(40, ts - last);
      last = ts;
      const elapsed = ts - startTime;
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      let offscreenCount = 0;
      pieces.forEach((p) => {
        p.y += (p.fall * dt) / 1000;
        p.x += (p.drift * dt) / 1000;
        p.rot += p.rotSpeed * dt;
        if (p.y > 1.2) offscreenCount += 1;
        const x = p.x * canvas.width;
        const y = p.y * canvas.height;
        ctx.save();
        ctx.translate(x, y);
        ctx.rotate(p.rot);
        ctx.fillStyle = p.color;
        ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size * 0.6);
        ctx.restore();
      });
      const allGone = offscreenCount === pieces.length;
      if (!allGone && elapsed < duration * 2.5) {
        rafId = requestAnimationFrame(tick);
      } else {
        cleanup();
      }
    }

    function cleanup() {
      if (rafId) cancelAnimationFrame(rafId);
      window.removeEventListener("resize", resize);
      canvas.remove();
    }

    window.addEventListener("resize", resize);
    const startTime = performance.now();
    rafId = requestAnimationFrame(tick);
  }

  global.MB = global.MB || {};
  global.MB.confettiRain = confettiRain;
})(window);
