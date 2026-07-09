import init, { generate } from "./pkg/pickleball_scheduler.js";

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.from(document.querySelectorAll(sel));

let ready = false;
let rosterMode = "counts"; // "counts" | "names"
let goal = "part1"; // "part1" | "each" | "total"
let seed = (Math.random() * 0xffffffff) >>> 0;

// ---- boot ----
(async () => {
  try {
    await init();
    ready = true;
  } catch (e) {
    showBanner("Couldn't load the scheduling engine (WebAssembly). Try a hard refresh.");
    console.error(e);
  }
})();

// ---- roster tabs ----
$$(".tab").forEach((t) =>
  t.addEventListener("click", () => {
    rosterMode = t.dataset.roster;
    $$(".tab").forEach((x) => x.classList.toggle("active", x === t));
    $$("[data-pane]").forEach((p) => (p.hidden = p.dataset.pane !== rosterMode));
  })
);

// ---- goal segmented ----
$$(".seg").forEach((s) =>
  s.addEventListener("click", () => {
    goal = s.dataset.goal;
    $$(".seg").forEach((x) => x.classList.toggle("active", x === s));
    $$("[data-goal-pane]").forEach((p) => (p.hidden = p.dataset.goalPane !== goal));
  })
);

// ---- live names count ----
const updateNamesCount = () => {
  const m = parseNames($("#menNames").value).length;
  const w = parseNames($("#womenNames").value).length;
  $("#namesCount").textContent = m + w ? `${m} men · ${w} women` : "";
};
$("#menNames").addEventListener("input", updateNamesCount);
$("#womenNames").addEventListener("input", updateNamesCount);

// ---- actions ----
$("#generate").addEventListener("click", () => run());
$("#reshuffle").addEventListener("click", () => {
  seed = (Math.random() * 0xffffffff) >>> 0;
  run();
});
$("#print").addEventListener("click", () => window.print());

// ---- helpers ----
function parseNames(text) {
  return text
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function clampInt(v, lo, hi) {
  v = parseInt(v, 10);
  if (isNaN(v)) v = lo;
  return Math.max(lo, Math.min(hi, v));
}

function getRoster() {
  if (rosterMode === "names") {
    const men = parseNames($("#menNames").value);
    const women = parseNames($("#womenNames").value);
    return { men, women };
  }
  const M = clampInt($("#men").value, 0, 30);
  const W = clampInt($("#women").value, 0, 30);
  return {
    men: Array.from({ length: M }, (_, i) => `M${i + 1}`),
    women: Array.from({ length: W }, (_, i) => `W${i + 1}`),
  };
}

function getModeParam() {
  if (goal === "each") return { mode: 3, param: clampInt($("#eachN").value, 1, 60) };
  if (goal === "total") return { mode: 4, param: clampInt($("#totalG").value, 1, 400) };
  return { mode: parseInt($("#emphasis").value, 10), param: 0 };
}

function showBanner(msg) {
  const r = $("#results");
  r.hidden = false;
  $("#stats").innerHTML = `<div class="banner">${msg}</div>`;
  $("#grid").innerHTML = "";
}

// ---- main run ----
function run() {
  if (!ready) return showBanner("Engine still loading — one moment, then try again.");

  const { men, women } = getRoster();
  const courts = clampInt($("#courts").value, 1, 20);

  if (men.length < 2 || women.length < 2) {
    return showBanner("Need at least 2 men and 2 women to form a game.");
  }

  const { mode, param } = getModeParam();
  const btn = $("#generate");
  btn.disabled = true;
  btn.textContent = "Generating…";

  // Let the browser paint the disabled state before the (synchronous) compute.
  requestAnimationFrame(() => {
    let data;
    try {
      const json = generate(men.length, women.length, courts, mode, param, seed);
      data = JSON.parse(json);
    } catch (e) {
      console.error(e);
      showBanner("Something went wrong generating the schedule.");
      resetBtn(btn);
      return;
    }
    if (data.error) {
      showBanner(data.error);
    } else {
      render(data, men, women);
    }
    resetBtn(btn);
  });
}

function resetBtn(btn) {
  btn.disabled = false;
  btn.textContent = "Generate schedule";
}

// ---- rendering ----
function render(data, men, women) {
  $("#results").hidden = false;
  renderStats(data.report);
  renderGrid(data, men, women);
  $("#results").scrollIntoView({ behavior: "smooth", block: "start" });
}

function tile(k, v, s, good, badge) {
  return `<div class="tile ${good ? "good" : ""}">
    <div class="k">${k}</div>
    <div class="v">${v}${badge ? ` <span class="badge">${badge}</span>` : ""}</div>
    <div class="s">${s}</div>
  </div>`;
}

function renderStats(r) {
  const sgExcess = r.manExcess + r.womanExcess;
  const sgFloor = r.manFloor + r.womanFloor;
  const sgAtFloor = r.manExcess === r.manFloor && r.womanExcess === r.womanFloor;

  const gpm = r.gamesPerMan.concat(r.gamesPerWoman);
  const lo = Math.min(...gpm), hi = Math.max(...gpm);

  const tiles = [];
  tiles.push(
    tile("Games", `${r.games}`, `of ${r.maxGames} possible`, r.games === r.maxGames, r.games === r.maxGames ? "max ✓" : "")
  );
  tiles.push(
    tile("Court use", `${Math.round(r.courtUtil * 100)}%`, `${r.rounds} rounds`, r.courtUtil >= 0.999)
  );

  // Partner/opponent repeats only matter once Part 2 pushes past the ceiling.
  if (r.partnerExcess > 0 || r.partnerFloor > 0) {
    const at = r.partnerExcess === r.partnerFloor && r.mixedExcess === r.mixedFloor;
    tiles.push(
      tile("Partner/opp repeats", `${r.partnerExcess + r.mixedExcess}`, `floor ${r.partnerFloor + r.mixedFloor}`, at, at ? "min ✓" : "")
    );
  } else {
    tiles.push(tile("Once-only rule", "Kept", "no repeat partners or opponents", true, "✓"));
  }

  tiles.push(
    tile("Same-gender repeats", `${sgExcess}`, `floor ${sgFloor}`, sgAtFloor, sgAtFloor ? "min ✓" : "")
  );
  tiles.push(
    tile("Games per player", lo === hi ? `${lo}` : `${lo}–${hi}`, lo === hi ? "everyone equal" : `spread ${hi - lo}`, lo === hi)
  );

  $("#stats").innerHTML = tiles.join("");
}

function renderGrid(data, men, women) {
  const cols = Math.max(1, ...data.rounds.map((r) => r.length));

  let head = `<tr><th>Round</th>`;
  for (let c = 0; c < cols; c++) head += `<th>Court ${c + 1}</th>`;
  head += `<th>Sitting out</th></tr>`;

  let body = "";
  data.rounds.forEach((round, ri) => {
    const activeMen = new Set();
    const activeWomen = new Set();
    let cells = "";
    for (let c = 0; c < cols; c++) {
      const g = round[c];
      if (!g) {
        cells += `<td class="empty">—</td>`;
        continue;
      }
      const [ma, wa, mb, wb] = g;
      activeMen.add(ma).add(mb);
      activeWomen.add(wa).add(wb);
      cells += `<td class="game"><span class="a">${esc(men[ma])} &amp; ${esc(women[wa])}</span><span class="vs">vs</span><span class="b">${esc(men[mb])} &amp; ${esc(women[wb])}</span></td>`;
    }
    const byes = [];
    men.forEach((n, i) => { if (!activeMen.has(i)) byes.push(esc(n)); });
    women.forEach((n, i) => { if (!activeWomen.has(i)) byes.push(esc(n)); });
    const byeStr = byes.length ? byes.join(", ") : "—";
    body += `<tr><td class="rnum">${ri + 1}</td>${cells}<td class="byes">${byeStr}</td></tr>`;
  });

  $("#grid").innerHTML = `<table class="sched"><thead>${head}</thead><tbody>${body}</tbody></table>`;
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}
