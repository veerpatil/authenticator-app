const { invoke } = window.__TAURI__.core;

let refreshInterval = null;

window.addEventListener("DOMContentLoaded", () => {
  initNavigation();
  initAddForm();
  initAddButtons();
  initTypeToggle();
  initDeleteConfirm();
  startCodeRefresh();
});

/* ── Navigation ── */

function initNavigation() {
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", (e) => {
      e.preventDefault();
      navigateTo(item.dataset.page);
    });
  });
}

function navigateTo(page) {
  document.querySelectorAll(".nav-item").forEach((n) => n.classList.remove("active"));
  const navItem = document.querySelector(`.nav-item[data-page="${page}"]`);
  if (navItem) navItem.classList.add("active");

  document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
  const pageEl = document.getElementById(`page-${page}`);
  if (pageEl) pageEl.classList.add("active");

  if (page === "accounts") refreshCodes();
}

/* ── Add buttons ── */

function initAddButtons() {
  document.getElementById("btn-add-top")?.addEventListener("click", () => navigateTo("add"));
  document.getElementById("btn-add-empty")?.addEventListener("click", () => navigateTo("add"));
  document.getElementById("btn-cancel-manual")?.addEventListener("click", () => navigateTo("accounts"));
  document.getElementById("btn-cancel-uri")?.addEventListener("click", () => navigateTo("accounts"));
}

/* ── Type toggle (TOTP vs HOTP field swap) ── */

function initTypeToggle() {
  const typeSelect = document.getElementById("input-type");
  if (!typeSelect) return;

  typeSelect.addEventListener("change", () => {
    const isHotp = typeSelect.value === "hotp";
    document.getElementById("period-group")?.classList.toggle("hidden", isHotp);
    document.getElementById("counter-group")?.classList.toggle("hidden", !isHotp);
  });
}

/* ── Code refresh loop ── */

function startCodeRefresh() {
  refreshCodes();
  refreshInterval = setInterval(refreshCodes, 1000);
}

async function refreshCodes() {
  const page = document.getElementById("page-accounts");
  if (!page || !page.classList.contains("active")) return;

  try {
    const accounts = await invoke("get_all_codes");
    renderAccounts(accounts);
    updateGlobalTimer(accounts);
  } catch (err) {
    console.error("Failed to refresh codes:", err);
  }
}

/* ── Render accounts ── */

function renderAccounts(accounts) {
  const list = document.getElementById("accounts-list");
  const empty = document.getElementById("empty-state");
  if (!list || !empty) return;

  if (accounts.length === 0) {
    list.style.display = "none";
    empty.classList.add("visible");
    return;
  }

  list.style.display = "";
  empty.classList.remove("visible");

  const existingMap = {};
  list.querySelectorAll(".account-card").forEach((card) => {
    existingMap[card.dataset.id] = card;
  });

  const fragment = document.createDocumentFragment();

  accounts.forEach((acc) => {
    let card = existingMap[acc.id];
    if (!card) {
      card = createAccountCard(acc);
    } else {
      updateAccountCard(card, acc);
      delete existingMap[acc.id];
    }
    fragment.appendChild(card);
  });

  Object.values(existingMap).forEach((card) => card.remove());
  list.innerHTML = "";
  list.appendChild(fragment);
}

function formatCode(code, digits) {
  if (code.startsWith("ERR")) return code;
  if (digits === 8) return code.slice(0, 4) + " " + code.slice(4);
  const mid = Math.ceil(code.length / 2);
  return code.slice(0, mid) + " " + code.slice(mid);
}

function createAccountCard(acc) {
  const card = document.createElement("div");
  card.className = "account-card";
  card.dataset.id = acc.id;
  card.dataset.code = acc.code;

  const initial = (acc.issuer || acc.label || "?").charAt(0).toUpperCase();
  const isHotp = acc.otp_type === "hotp";
  const pct = isHotp ? 100 : (acc.seconds_remaining / acc.period) * 100;
  const expiring = !isHotp && acc.seconds_remaining <= 5;

  const typeLabel = isHotp
    ? `<span class="otp-type-badge hotp">HOTP</span>`
    : `<span class="otp-type-badge totp">TOTP</span>`;

  const counterLabel = isHotp
    ? `<span class="account-counter">counter: ${acc.counter}</span>`
    : "";

  const nextBtn = isHotp
    ? `<button class="btn-icon next-btn" title="Next code">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <polyline points="13 17 18 12 13 7"/>
          <polyline points="6 17 11 12 6 7"/>
        </svg>
      </button>`
    : "";

  card.innerHTML = `
    <div class="account-avatar">${initial}</div>
    <div class="account-info">
      <div class="account-issuer-row">
        <span class="account-issuer">${escapeHtml(acc.issuer || "Unknown")}</span>
        ${typeLabel}
      </div>
      <div class="account-label">${escapeHtml(acc.label)} ${counterLabel}</div>
    </div>
    <div class="account-code ${acc.code.startsWith("ERR") ? "error" : ""} ${expiring ? "expiring" : ""}">${formatCode(acc.code, acc.digits)}</div>
    <div class="account-actions">
      ${nextBtn}
      <button class="btn-icon verify-btn" title="Verify account">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="16" x2="12" y2="12"/>
          <line x1="12" y1="8" x2="12.01" y2="8"/>
        </svg>
      </button>
      <button class="btn-icon copy-btn" title="Copy code">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
          <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
        </svg>
      </button>
      <button class="btn-icon danger delete-btn" title="Delete">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <polyline points="3 6 5 6 21 6"/>
          <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>
        </svg>
      </button>
    </div>
    <div class="copy-hint">Copied!</div>
    <div class="account-progress">
      <div class="account-progress-bar ${expiring ? "expiring" : ""}" style="width: ${pct}%"></div>
    </div>
  `;

  card.querySelector(".next-btn")?.addEventListener("click", (e) => {
    e.stopPropagation();
    advanceHotp(acc.id);
  });

  card.addEventListener("click", (e) => {
    if (e.target.closest(".delete-btn") || e.target.closest(".next-btn")) return;
    copyCode(card, card.dataset.code);
  });

  card.querySelector(".verify-btn")?.addEventListener("click", (e) => {
    e.stopPropagation();
    verifyAccount(acc.id);
  });

  card.querySelector(".copy-btn")?.addEventListener("click", (e) => {
    e.stopPropagation();
    copyCode(card, card.dataset.code);
  });

  card.querySelector(".delete-btn")?.addEventListener("click", (e) => {
    e.stopPropagation();
    deleteAccount(acc.id, acc.issuer);
  });

  return card;
}

function updateAccountCard(card, acc) {
  card.dataset.code = acc.code;
  const codeEl = card.querySelector(".account-code");
  const progressBar = card.querySelector(".account-progress-bar");
  const isHotp = acc.otp_type === "hotp";
  const expiring = !isHotp && acc.seconds_remaining <= 5;
  const pct = isHotp ? 100 : (acc.seconds_remaining / acc.period) * 100;

  if (codeEl) {
    codeEl.textContent = formatCode(acc.code, acc.digits);
    codeEl.classList.toggle("expiring", expiring);
    codeEl.classList.toggle("error", acc.code.startsWith("ERR"));
  }

  if (progressBar) {
    progressBar.style.width = `${pct}%`;
    progressBar.classList.toggle("expiring", expiring);
  }

  const counterEl = card.querySelector(".account-counter");
  if (counterEl && isHotp) {
    counterEl.textContent = `counter: ${acc.counter}`;
  }
}

/* ── Advance HOTP counter ── */

async function advanceHotp(id) {
  try {
    await invoke("next_hotp_code", { id });
    showToast("Counter advanced");
    refreshCodes();
  } catch (err) {
    showToast("Error: " + err);
  }
}

/* ── Global timer ── */

function updateGlobalTimer(accounts) {
  const circle = document.getElementById("timer-circle");
  const text = document.getElementById("timer-text");
  if (!circle || !text) return;

  const totpAccounts = accounts.filter((a) => a.otp_type === "totp");
  const period = totpAccounts.length > 0 ? totpAccounts[0].period : 30;
  const remaining = totpAccounts.length > 0 ? totpAccounts[0].seconds_remaining : 30;
  const circumference = 97.4;
  const offset = circumference - (remaining / period) * circumference;

  circle.style.strokeDashoffset = offset;
  text.textContent = remaining;

  if (remaining <= 5) {
    circle.style.stroke = "var(--danger)";
    text.style.color = "var(--danger)";
  } else {
    circle.style.stroke = "var(--accent)";
    text.style.color = "var(--accent)";
  }
}

/* ── Copy to clipboard ── */

async function copyCode(card, code) {
  try {
    const cleanCode = code.replace(/\s/g, "");
    await invoke("copy_to_clipboard", { text: cleanCode });
    card.classList.add("copied");
    showToast("Code copied to clipboard");
    setTimeout(() => card.classList.remove("copied"), 1500);
  } catch (err) {
    showToast("Failed to copy: " + err);
  }
}

/* ── Delete ── */

let pendingDeleteId = null;

function deleteAccount(id, issuer) {
  pendingDeleteId = id;
  const modal = document.getElementById("confirm-modal");
  const msg = document.getElementById("confirm-msg");
  if (modal && msg) {
    msg.textContent = `Delete "${issuer || "this account"}"?`;
    modal.classList.add("visible");
  }
}

function initDeleteConfirm() {
  document.getElementById("confirm-yes")?.addEventListener("click", async () => {
    const modal = document.getElementById("confirm-modal");
    modal?.classList.remove("visible");
    if (!pendingDeleteId) return;
    try {
      await invoke("delete_account", { id: pendingDeleteId });
      showToast("Account deleted");
      pendingDeleteId = null;
      refreshCodes();
    } catch (err) {
      showToast("Failed to delete: " + err);
    }
  });

  document.getElementById("confirm-no")?.addEventListener("click", () => {
    pendingDeleteId = null;
    document.getElementById("confirm-modal")?.classList.remove("visible");
  });
}

/* ── Add Form ── */

function initAddForm() {
  document.querySelectorAll(".form-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      document.querySelectorAll(".form-tab").forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");
      const target = tab.dataset.tab;
      document.getElementById("form-manual").classList.toggle("hidden", target !== "manual");
      document.getElementById("form-uri").classList.toggle("hidden", target !== "uri");
    });
  });

  // Manual form
  document.getElementById("form-manual")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const issuer = document.getElementById("input-issuer").value.trim();
    const label = document.getElementById("input-label").value.trim();
    const secret = document.getElementById("input-secret").value.trim();
    const otpType = document.getElementById("input-type").value;
    const digits = parseInt(document.getElementById("input-digits").value);
    const period = parseInt(document.getElementById("input-period").value);
    const algorithm = document.getElementById("input-algorithm").value;
    const counter = parseInt(document.getElementById("input-counter").value) || 0;

    if (!issuer || !label || !secret) {
      showToast("Please fill in all required fields");
      return;
    }

    try {
      await invoke("add_account", {
        issuer,
        label,
        secret,
        digits,
        period,
        algorithm,
        otpType,
        counter,
      });
      showToast(`${issuer} account added`);
      e.target.reset();
      navigateTo("accounts");
    } catch (err) {
      showToast("Error: " + err);
    }
  });

  // URI form
  document.getElementById("form-uri")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const uri = document.getElementById("input-uri").value.trim();

    if (!uri) {
      showToast("Please paste an otpauth:// URI");
      return;
    }

    try {
      const parsed = await invoke("parse_otpauth_uri", { uri });
      await invoke("add_account", {
        issuer: parsed.issuer || "Unknown",
        label: parsed.label || "",
        secret: parsed.secret,
        digits: parsed.digits,
        period: parsed.period,
        algorithm: parsed.algorithm,
        otpType: parsed.otp_type || "totp",
        counter: parsed.counter || 0,
      });
      showToast(`${parsed.issuer || "Account"} added`);
      e.target.reset();
      navigateTo("accounts");
    } catch (err) {
      showToast("Invalid URI: " + err);
    }
  });
}

/* ── Verify ── */

async function verifyAccount(id) {
  try {
    const info = await invoke("verify_account", { id });
    const msg = [
      `Stored secret: ${info.stored_secret}`,
      `Secret bytes: ${info.secret_length_bytes}`,
      `Type: ${info.otp_type} | Algo: ${info.algorithm} | Digits: ${info.digits} | Period: ${info.period}s`,
      `Unix time: ${info.unix_time}`,
      `Code: ${info.current_code} (${info.seconds_remaining}s left)`,
    ].join("\n");

    const modal = document.getElementById("confirm-modal");
    const msgEl = document.getElementById("confirm-msg");
    const yesBtn = document.getElementById("confirm-yes");
    const noBtn = document.getElementById("confirm-no");
    if (modal && msgEl) {
      msgEl.style.whiteSpace = "pre-wrap";
      msgEl.style.textAlign = "left";
      msgEl.style.fontSize = "12px";
      msgEl.style.fontFamily = "monospace";
      msgEl.textContent = msg;
      yesBtn.style.display = "none";
      noBtn.textContent = "Close";
      modal.classList.add("visible");

      const handler = () => {
        msgEl.style.whiteSpace = "";
        msgEl.style.textAlign = "";
        msgEl.style.fontSize = "";
        msgEl.style.fontFamily = "";
        yesBtn.style.display = "";
        noBtn.textContent = "Cancel";
        noBtn.removeEventListener("click", handler);
      };
      noBtn.addEventListener("click", handler);
    }
  } catch (err) {
    showToast("Verify failed: " + err);
  }
}

/* ── Toast ── */

let toastTimeout = null;

function showToast(message) {
  const toast = document.getElementById("toast");
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add("show");
  clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => toast.classList.remove("show"), 2500);
}

/* ── Util ── */

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
