// Lilac Web UI — Flat Native HIG App (Monochrome + Lilac, backed by Rust `chess-core`)

const STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

const state = {
  uciMoves: [],
  fen: STARTING_FEN,
  turn: "white",
  isCheck: false,
  status: { status: "ongoing" },
  legalMoves: [],
  sanHistory: [],
  opening: null,
  phase: "Opening",
  materialBalanceCp: 0,
  capturedByWhite: [],
  capturedByBlack: [],
  requestSeq: 0,
  selectedSq: null,
  dragFromSq: null,
  flipped: false,
  autoFlip: false,
  pieceSet: "spatial",
  soundEnabled: true,
  token: localStorage.getItem("lilac_jwt") || null,
  username: localStorage.getItem("lilac_username") || null,
  authMode: "login",
};

let audioCtx = null;
function playTone(kind) {
  if (!state.soundEnabled) return;
  try {
    audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    const now = audioCtx.currentTime;
    if (kind === "capture") {
      osc.type = "triangle";
      osc.frequency.setValueAtTime(190, now);
      osc.frequency.exponentialRampToValueAtTime(75, now + 0.08);
      gain.gain.setValueAtTime(0.18, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.09);
    } else if (kind === "check") {
      osc.type = "sine";
      osc.frequency.setValueAtTime(540, now);
      osc.frequency.exponentialRampToValueAtTime(380, now + 0.14);
      gain.gain.setValueAtTime(0.14, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.15);
    } else {
      osc.type = "sine";
      osc.frequency.setValueAtTime(240, now);
      osc.frequency.exponentialRampToValueAtTime(120, now + 0.055);
      gain.gain.setValueAtTime(0.12, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.06);
    }
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    osc.start(now);
    osc.stop(now + 0.16);
  } catch {
    // Ignore autoplay restriction before first gesture
  }
}

function coordsToSq(file, rank) {
  return String.fromCharCode(97 + file) + (rank + 1);
}

function parseFenBoard(fen) {
  const placement = fen.split(" ")[0];
  const rows = placement.split("/");
  const map = {};
  for (let r = 0; r < 8; r++) {
    let f = 0;
    for (const ch of rows[r]) {
      if (ch >= "1" && ch <= "8") {
        f += parseInt(ch, 10);
      } else {
        const color = ch === ch.toUpperCase() ? "w" : "b";
        const kind = ch.toUpperCase();
        map[coordsToSq(f, 7 - r)] = color + kind;
        f++;
      }
    }
  }
  return map;
}

async function syncBoardFromRust(uciMoves) {
  const seq = ++state.requestSeq;
  const res = await fetch("/api/sandbox/inspect", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ uci_moves: uciMoves }),
  });
  if (!res.ok || seq !== state.requestSeq) return false;
  const data = await res.json();
  if (seq !== state.requestSeq) return false;
  const prevLen = state.uciMoves.length;
  state.uciMoves = data.uci_history;
  state.fen = data.fen;
  state.turn = data.turn;
  state.isCheck = data.is_check;
  state.status = data.status;
  state.legalMoves = data.legal_moves;
  state.sanHistory = data.san_history;
  state.opening = data.opening;
  state.phase = data.phase;
  state.materialBalanceCp = data.material_balance_cp;
  state.capturedByWhite = data.captured_by_white || [];
  state.capturedByBlack = data.captured_by_black || [];
  state.selectedSq = null;

  if (state.uciMoves.length > prevLen) {
    if (state.isCheck) playTone("check");
    else if (state.sanHistory[state.sanHistory.length - 1]?.includes("x")) playTone("capture");
    else playTone("move");
    if (state.autoFlip) {
      state.flipped = state.turn === "black";
    }
  }
  renderArena();
  return true;
}

function legalTargetsFor(sq) {
  if (!sq) return [];
  return state.legalMoves.filter((u) => u.startsWith(sq)).map((u) => u.slice(2, 4));
}

async function tryMove(fromSq, toSq) {
  const matches = state.legalMoves.filter((u) => u.startsWith(fromSq + toSq));
  if (matches.length === 0) return false;
  if (matches.some((u) => u.length === 5)) {
    openPromotionModal(fromSq, toSq);
    return true;
  }
  return syncBoardFromRust([...state.uciMoves, matches[0]]);
}

function openPromotionModal(fromSq, toSq) {
  const color = state.turn === "white" ? "w" : "b";
  const box = document.getElementById("promo-choices");
  box.innerHTML = "";
  for (const [uciChar, pieceChar] of [["q", "Q"], ["r", "R"], ["b", "B"], ["n", "N"]]) {
    const btn = document.createElement("button");
    btn.className = "promo-btn";
    btn.innerHTML = `<img src="/assets/pieces/${state.pieceSet}/${color}${pieceChar}.svg" alt="${pieceChar}" />`;
    btn.onclick = async () => {
      closePromotionModal();
      await syncBoardFromRust([...state.uciMoves, fromSq + toSq + uciChar]);
    };
    box.appendChild(btn);
  }
  document.getElementById("promo-modal").classList.remove("hidden");
}

function closePromotionModal() {
  document.getElementById("promo-modal").classList.add("hidden");
}

const boardSquareNodes = [];

function ensureBoardGrid(boardEl) {
  if (boardSquareNodes.length === 64) return;
  boardEl.innerHTML = "";
  for (let idx = 0; idx < 64; idx++) {
    const sqEl = document.createElement("div");
    const rLbl = document.createElement("span");
    rLbl.className = "coord-rank hidden";
    const fLbl = document.createElement("span");
    fLbl.className = "coord-file hidden";
    const img = document.createElement("img");
    img.className = "piece hidden";
    sqEl.append(rLbl, fLbl, img);

    sqEl.addEventListener("pointerdown", async (e) => {
      if (e.button !== 0) return;
      const sq = sqEl.dataset.sq;
      if (state.selectedSq && state.selectedSq !== sq && legalTargetsFor(state.selectedSq).includes(sq)) {
        await tryMove(state.selectedSq, sq);
        return;
      }
      if (legalTargetsFor(sq).length > 0) {
        state.selectedSq = state.selectedSq === sq ? null : sq;
        state.dragFromSq = sq;
        renderArena();
      } else {
        state.selectedSq = null;
        renderArena();
      }
    });

    boardEl.appendChild(sqEl);
    boardSquareNodes.push({ sqEl, rLbl, fLbl, img });
  }
}

function renderArena() {
  const boardEl = document.getElementById("chess-board");
  ensureBoardGrid(boardEl);
  const pieceMap = parseFenBoard(state.fen);
  const legalTargets = new Set(legalTargetsFor(state.selectedSq));
  const lastUci = state.uciMoves[state.uciMoves.length - 1];
  const lastFrom = lastUci ? lastUci.slice(0, 2) : null;
  const lastTo = lastUci ? lastUci.slice(2, 4) : null;
  const kingCode = state.turn === "white" ? "wK" : "bK";

  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const idx = row * 8 + col;
      const { sqEl, rLbl, fLbl, img } = boardSquareNodes[idx];
      const file = state.flipped ? 7 - col : col;
      const rank = state.flipped ? row : 7 - row;
      const sq = coordsToSq(file, rank);
      const isLight = (file + rank) % 2 === 1;
      const piece = pieceMap[sq];

      sqEl.dataset.sq = sq;
      sqEl.className = `square ${isLight ? "light" : "dark"}`;
      if (sq === lastFrom || sq === lastTo) sqEl.classList.add("last-move");
      if (sq === state.selectedSq) sqEl.classList.add("selected");
      if (state.isCheck && piece === kingCode) sqEl.classList.add("in-check");
      if (legalTargets.has(sq)) {
        sqEl.classList.add(piece ? "legal-capture" : "legal-empty");
      }

      if (col === 0) {
        rLbl.textContent = String(rank + 1);
        rLbl.classList.remove("hidden");
      } else {
        rLbl.classList.add("hidden");
      }
      if (row === 7) {
        fLbl.textContent = String.fromCharCode(97 + file);
        fLbl.classList.remove("hidden");
      } else {
        fLbl.classList.add("hidden");
      }

      if (piece) {
        const expectedSrc = `/assets/pieces/${state.pieceSet}/${piece}.svg`;
        if (img.getAttribute("src") !== expectedSrc) {
          img.setAttribute("src", expectedSrc);
        }
        img.alt = piece;
        img.classList.remove("hidden");
      } else {
        img.classList.add("hidden");
      }
    }
  }

  const byWhite = state.capturedByWhite;
  const byBlack = state.capturedByBlack;
  const topSide = state.flipped ? "white" : "black";
  const bottomSide = state.flipped ? "black" : "white";
  updatePlayerBar("top-player-bar", "top-captured", topSide, byWhite, byBlack);
  updatePlayerBar("bottom-player-bar", "bottom-captured", bottomSide, byWhite, byBlack);

  const statusEl = document.getElementById("game-status-text");
  statusEl.classList.remove("lilac-text");
  if (state.status.status === "ongoing") {
    const sideLabel = state.turn === "white" ? "White" : "Black";
    statusEl.textContent = state.isCheck ? `${sideLabel} in Check` : `${sideLabel} to move`;
    if (state.isCheck) statusEl.classList.add("lilac-text");
  } else if (state.status.status === "checkmate") {
    statusEl.textContent = `Checkmate — ${state.status.winner.toUpperCase()} wins`;
    statusEl.classList.add("lilac-text");
  } else {
    statusEl.textContent = `Draw (${state.status.status.replace("_", " ")})`;
    statusEl.classList.add("lilac-text");
  }

  const openingLineEl = document.getElementById("opening-line");
  if (state.opening) {
    document.getElementById("opening-eco").textContent = state.opening.eco;
    document.getElementById("opening-name").textContent = state.opening.name;
    openingLineEl.classList.remove("hidden");
  } else {
    openingLineEl.classList.add("hidden");
  }

  const sheet = document.getElementById("scoresheet");
  sheet.innerHTML = "";
  for (let i = 0; i < state.sanHistory.length; i += 2) {
    const row = document.createElement("div");
    row.className = "move-row";
    const wMove = state.sanHistory[i] || "";
    const bMove = state.sanHistory[i + 1] || "";
    const wClass = i === state.sanHistory.length - 1 ? "move-latest" : "";
    const bClass = i + 1 === state.sanHistory.length - 1 ? "move-latest" : "";
    row.innerHTML = `<span class="move-num">${i / 2 + 1}.</span><span class="${wClass}">${wMove}</span><span class="${bClass}">${bMove}</span>`;
    sheet.appendChild(row);
  }
  sheet.scrollTop = sheet.scrollHeight;
}

function updatePlayerBar(barId, trayId, side, byWhite, byBlack) {
  const bar = document.getElementById(barId);
  const ongoing = state.status.status === "ongoing";
  bar.classList.toggle("active-turn", ongoing && state.turn === side);
  bar.querySelector(".player-name").textContent = side === "white" ? "White" : "Black";

  const tray = document.getElementById(trayId);
  tray.innerHTML = "";
  const list = side === "white" ? byWhite : byBlack;
  for (const code of list) {
    const img = document.createElement("img");
    img.src = `/assets/pieces/${state.pieceSet}/${code}.svg`;
    img.alt = code;
    tray.appendChild(img);
  }
  const adv = side === "white" ? state.materialBalanceCp : -state.materialBalanceCp;
  if (adv > 0) {
    const span = document.createElement("span");
    span.className = "mat-adv";
    span.textContent = `+${Math.round(adv / 100)}`;
    tray.appendChild(span);
  }
}

async function loadLeaderboards() {
  const gRes = await fetch("/api/leaderboard/global?time_control=blitz&limit=20");
  const gData = gRes.ok ? await gRes.json() : { entries: [] };
  const gEl = document.getElementById("global-lb-list");
  gEl.innerHTML = gData.entries.length
    ? gData.entries.map((e) => renderLbRow(e, true)).join("")
    : `<div class="list-row"><span class="meta-right">No Gold+ qualified players yet.</span></div>`;

  const cRes = await fetch("/api/cohorts");
  const cohorts = cRes.ok ? await cRes.json() : [];
  const cEl = document.getElementById("cohort-lb-list");
  if (cohorts.length > 0) {
    const c = cohorts[0];
    document.getElementById("cohort-lb-title").textContent = c.name;
    const clbRes = await fetch(`/api/leaderboard/cohort/${c.id}?time_control=blitz&limit=20`);
    const clb = clbRes.ok ? await clbRes.json() : { entries: [] };
    cEl.innerHTML = clb.entries.map((e) => renderLbRow(e, false)).join("");
  } else {
    cEl.innerHTML = `<div class="list-row"><span class="meta-right">No active cohorts yet.</span></div>`;
  }
}

function renderLbRow(entry, isGlobal) {
  const right = isGlobal
    ? `<span class="lilac-text" style="font-weight:600">${entry.global_rating}</span>`
    : `<span class="meta-right">${entry.rr_points} RR</span>`;
  return `
    <div class="list-row">
      <div class="list-left">
        <span class="rank-num">#${entry.rank_position}</span>
        <strong>${entry.username}</strong>
        <span class="tier-label">${entry.tier.toUpperCase()} ${entry.division}</span>
      </div>
      <div>${right}</div>
    </div>
  `;
}

async function loadProfile() {
  if (!state.token) return;
  const res = await fetch("/api/profile/me", {
    headers: { Authorization: `Bearer ${state.token}` },
  });
  if (!res.ok) return;
  const p = await res.json();
  document.getElementById("auth-open-btn").textContent = p.username;

  const qual = p.rank.global_rating_qualified
    ? `Global ${p.rank.global_rating}`
    : `Qualifying (${p.rank.gold_qualification_games}/${p.rank.gold_qualification_target})`;

  const openingsHtml = p.top_openings.length
    ? p.top_openings
        .map(
          (o) => `<div class="list-row"><span>${o.eco_family}</span><span class="meta-right">Lv ${o.mastery_level}</span><span class="meta-right">${o.wins}/${o.games_played} (${Math.round(o.win_rate_pct)}%)</span></div>`,
        )
        .join("")
    : `<div class="list-row"><span class="meta-right">No opening games recorded yet.</span></div>`;

  const nemesesHtml = p.nemeses.length
    ? p.nemeses
        .map(
          (n) => `<div class="list-row"><strong>${n.opponent_username}</strong><span class="meta-right">${n.wins_against}-${n.losses_against}-${n.draws_against}</span></div>`,
        )
        .join("")
    : `<div class="list-row"><span class="meta-right">No Nemesis rivals recorded yet.</span></div>`;

  document.getElementById("profile-content").innerHTML = `
    <div class="profile-grid">
      <section class="editorial-section">
        <div class="kicker">VALORANT RANK TIER</div>
        <h2>${p.username} — ${p.rank.tier.toUpperCase()} ${p.rank.division}</h2>
        <div class="rr-bar"><div class="rr-fill" style="width:${Math.min(100, p.rank.rr_points)}%"></div></div>
        <div class="section-head">
          <span>${p.rank.rr_points} / 100 RR</span>
          <span class="lilac-text">${qual}</span>
        </div>
        <div class="inline-actions" style="margin-top:16px">
          <button id="logout-btn" class="text-btn">Sign Out</button>
        </div>
      </section>
      <section class="editorial-section">
        <div class="kicker">OPENING MASTERY</div>
        <div class="flat-table">${openingsHtml}</div>
      </section>
      <section class="editorial-section">
        <div class="kicker">NEMESIS RIVALRIES</div>
        <div class="flat-table">${nemesesHtml}</div>
      </section>
    </div>
  `;
  document.getElementById("logout-btn").onclick = () => {
    state.token = null;
    state.username = null;
    localStorage.removeItem("lilac_jwt");
    localStorage.removeItem("lilac_username");
    location.reload();
  };
}

function connectWebSocketPing() {
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(`${proto}://${location.host}/ws`);
  const lbl = document.getElementById("ws-label");

  ws.onopen = () => {
    if (state.token) {
      ws.send(JSON.stringify({ type: "authenticate", token: state.token }));
    }
    ws.send(JSON.stringify({ type: "ping", client_time_ms: Date.now() }));
  };
  ws.onmessage = (ev) => {
    try {
      const msg = JSON.parse(ev.data);
      if (msg.type === "pong") {
        const rtt = Math.max(0, Date.now() - msg.client_time_ms);
        lbl.textContent = `Connected (${rtt}ms)`;
      }
    } catch {
      // Ignore
    }
  };
  ws.onclose = () => {
    lbl.textContent = "Offline";
  };
}

// Primary Nav Tabs
document.querySelectorAll(".topbar .nav-tab[data-tab]").forEach((btn) => {
  btn.addEventListener("click", () => {
    const tab = btn.dataset.tab;
    document.querySelectorAll(".topbar .nav-tab[data-tab]").forEach((b) => b.classList.toggle("active", b === btn));
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.toggle("active", p.id === `tab-${tab}`));
    if (tab === "ladder") loadLeaderboards();
    if (tab === "profile") loadProfile();
  });
});

// Dock Actions
document.getElementById("btn-undo").onclick = () => {
  if (state.uciMoves.length > 0) syncBoardFromRust(state.uciMoves.slice(0, -1));
};
document.getElementById("btn-flip").onclick = () => {
  state.flipped = !state.flipped;
  renderArena();
};
document.getElementById("btn-reset").onclick = () => syncBoardFromRust([]);

// Options Sheet
const optionsModal = document.getElementById("options-modal");
document.getElementById("btn-options").onclick = () => optionsModal.classList.remove("hidden");
document.getElementById("options-close-btn").onclick = () => optionsModal.classList.add("hidden");

document.querySelectorAll("#piece-set-seg .nav-tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    state.pieceSet = btn.dataset.set;
    document.querySelectorAll("#piece-set-seg .nav-tab").forEach((b) => b.classList.toggle("active", b === btn));
    renderArena();
  });
});

document.getElementById("btn-auto-flip").onclick = (e) => {
  state.autoFlip = !state.autoFlip;
  e.currentTarget.textContent = state.autoFlip ? "On" : "Off";
  e.currentTarget.classList.toggle("lilac-text", state.autoFlip);
};

document.getElementById("sound-toggle").onclick = (e) => {
  state.soundEnabled = !state.soundEnabled;
  e.currentTarget.textContent = state.soundEnabled ? "On" : "Off";
};

document.getElementById("promo-cancel").onclick = closePromotionModal;

// Keyboard shortcuts
document.addEventListener("keydown", (e) => {
  if (e.target.tagName === "INPUT") return;
  if (e.key === "u" && state.uciMoves.length > 0) syncBoardFromRust(state.uciMoves.slice(0, -1));
  else if (e.key === "f") { state.flipped = !state.flipped; renderArena(); }
  else if (e.key === "r") syncBoardFromRust([]);
});

// Account Sheet
const authModal = document.getElementById("auth-modal");
const openAuth = () => authModal.classList.remove("hidden");
document.getElementById("auth-open-btn").onclick = openAuth;
document.getElementById("profile-signin-cta").onclick = openAuth;
document.getElementById("auth-close-btn").onclick = () => authModal.classList.add("hidden");

document.getElementById("mode-login-btn").onclick = () => {
  state.authMode = "login";
  document.getElementById("mode-login-btn").classList.add("active");
  document.getElementById("mode-register-btn").classList.remove("active");
  document.getElementById("auth-email").classList.add("hidden");
};
document.getElementById("mode-register-btn").onclick = () => {
  state.authMode = "register";
  document.getElementById("mode-register-btn").classList.add("active");
  document.getElementById("mode-login-btn").classList.remove("active");
  document.getElementById("auth-email").classList.remove("hidden");
};

document.getElementById("auth-form").onsubmit = async (e) => {
  e.preventDefault();
  const username = document.getElementById("auth-username").value.trim();
  const email = document.getElementById("auth-email").value.trim();
  const password = document.getElementById("auth-password").value;
  const errEl = document.getElementById("auth-error");
  errEl.textContent = "";

  const url = state.authMode === "register" ? "/api/auth/register" : "/api/auth/login";
  const body =
    state.authMode === "register"
      ? { username, email, password }
      : { username_or_email: username, password };

  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) {
    errEl.textContent = data.message || "Authentication failed";
    return;
  }
  state.token = data.token;
  state.username = data.username;
  localStorage.setItem("lilac_jwt", data.token);
  localStorage.setItem("lilac_username", data.username);
  authModal.classList.add("hidden");
  loadProfile();
};

document.querySelectorAll(".demo-login").forEach((btn) => {
  btn.onclick = async () => {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username_or_email: btn.dataset.user, password: "lilacseed123" }),
    });
    if (res.ok) {
      const data = await res.json();
      state.token = data.token;
      state.username = data.username;
      localStorage.setItem("lilac_jwt", data.token);
      localStorage.setItem("lilac_username", data.username);
      loadProfile();
    }
  };
});

// Global pointerup handles drag-and-drop cleanly even if released off-square
window.addEventListener("pointerup", async (e) => {
  if (!state.dragFromSq) return;
  const from = state.dragFromSq;
  state.dragFromSq = null;
  const hit = document.elementFromPoint(e.clientX, e.clientY)?.closest?.(".square");
  const targetSq = hit?.dataset?.sq;
  if (targetSq && targetSq !== from) {
    await tryMove(from, targetSq);
  }
});

syncBoardFromRust([]);
connectWebSocketPing();
if (state.token) {
  document.getElementById("auth-open-btn").textContent = state.username || "Account";
}
