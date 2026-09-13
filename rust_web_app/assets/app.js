const API = "";

function toast(msg) {
  const el = document.getElementById("toast");
  el.textContent = msg;
  el.hidden = false;
  clearTimeout(toast._t);
  toast._t = setTimeout(() => {
    el.hidden = true;
  }, 2800);
}

async function api(path, options = {}) {
  const res = await fetch(`${API}${path}`, {
    headers: { "Content-Type": "application/json", ...options.headers },
    ...options,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || res.statusText);
  }
  if (res.status === 204) return null;
  return res.json();
}

function statusBadge(check) {
  const s = check.last_status;
  if (!s) return '<span class="badge unknown">待检查</span>';
  if (s === "up") return '<span class="badge up">正常</span>';
  const err = check.last_error
    ? `<div class="err-text">${escapeHtml(check.last_error)}</div>`
    : "";
  return `<span class="badge down">异常</span>${err}`;
}

function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

let historyState = { checkId: null, name: "", offset: 0, total: 0, limit: 50 };
let alertHistoryState = { offset: 0, total: 0, limit: 50, kind: "" };
let editingCheckId = null;

const ALERT_KIND_LABELS = {
  down: "故障告警",
  recovery: "恢复通知",
  alive_ping: "每日签到",
  test: "通道测试",
};

const ALERT_CHANNEL_LABELS = {
  feishu: "飞书",
  pushplus: "PushPlus",
};

const CHECKPOINT_KIND_OPTIONS = [
  { value: "contains", label: "包含文本" },
  { value: "equals", label: "完全相等" },
  { value: "not_contains", label: "不包含" },
  { value: "regex", label: "正则匹配" },
  { value: "not_regex", label: "正则不匹配" },
];

function renderCheckpointRows(rows) {
  const tbody = document.getElementById("checkpoint-body");
  if (!rows.length) {
    tbody.innerHTML =
      '<tr><td colspan="4" style="color:var(--muted)">未配置检查点（仅校验 HTTP 状态码）</td></tr>';
    return;
  }
  tbody.innerHTML = rows
    .map(
      (row, i) => `
    <tr data-cp-row="${i}">
      <td>
        <select class="cp-kind" data-i="${i}">
          ${CHECKPOINT_KIND_OPTIONS.map(
            (o) =>
              `<option value="${o.value}" ${row.kind === o.value ? "selected" : ""}>${o.label}</option>`
          ).join("")}
        </select>
      </td>
      <td><input type="text" class="cp-value" data-i="${i}" value="${escapeHtml(row.value)}" placeholder="预期字符串或正则" /></td>
      <td><input type="checkbox" class="cp-enabled" data-i="${i}" ${row.enabled ? "checked" : ""} /></td>
      <td><button type="button" class="btn-link danger cp-remove" data-i="${i}">删除</button></td>
    </tr>`
    )
    .join("");

  tbody.querySelectorAll(".cp-remove").forEach((btn) => {
    btn.addEventListener("click", () => {
      const rowsNow = readCheckpointsFromDom();
      rowsNow.splice(Number(btn.dataset.i), 1);
      renderCheckpointRows(rowsNow);
    });
  });
}

function readCheckpointsFromDom() {
  const tbody = document.getElementById("checkpoint-body");
  const trs = tbody.querySelectorAll("tr[data-cp-row]");
  if (!trs.length) return [];
  return Array.from(trs).map((tr) => {
    const kind = tr.querySelector(".cp-kind")?.value || "contains";
    const value = tr.querySelector(".cp-value")?.value ?? "";
    const enabled = tr.querySelector(".cp-enabled")?.checked ?? true;
    return { kind, value: value.trim(), enabled };
  });
}

async function loadCheckpointsForCheck(checkId) {
  try {
    const data = await api("/api/checks/" + checkId + "/checkpoints");
    const rows = (data.checkpoints || []).map((c) => ({
      kind: c.kind,
      value: c.value,
      enabled: !!c.enabled,
    }));
    renderCheckpointRows(rows);
  } catch {
    renderCheckpointRows([]);
  }
}

function resetCheckForm() {
  editingCheckId = null;
  const form = document.getElementById("check-form");
  form.reset();
  document.getElementById("check-status").value = "200";
  document.getElementById("check-interval").value = "60";
  document.getElementById("check-enabled").checked = true;
  document.getElementById("check-submit").textContent = "添加检查";
  document.getElementById("check-cancel-edit").hidden = true;
  document.getElementById("check-form-hint").textContent =
    "填写下方表单添加新的健康检查。";
  renderCheckpointRows([]);
}

function beginEditCheck(check) {
  editingCheckId = check.id;
  document.getElementById("check-name").value = check.name;
  document.getElementById("check-url").value = check.url;
  document.getElementById("check-method").value = check.method || "GET";
  document.getElementById("check-status").value = check.expected_status;
  document.getElementById("check-interval").value = check.interval_secs;
  document.getElementById("check-enabled").checked = !!check.enabled;
  document.getElementById("check-submit").textContent = "保存修改";
  document.getElementById("check-cancel-edit").hidden = false;
  document.getElementById("check-form-hint").textContent = `正在编辑：${check.name}（ID ${check.id}）`;
  loadCheckpointsForCheck(check.id);
  document.getElementById("check-form").scrollIntoView({ behavior: "smooth", block: "start" });
}

function readCheckFormPayload() {
  const checkpoints = readCheckpointsFromDom().filter((c) => c.value.length > 0);
  return {
    name: document.getElementById("check-name").value.trim(),
    url: document.getElementById("check-url").value.trim(),
    method: document.getElementById("check-method").value,
    expected_status: Number(document.getElementById("check-status").value),
    interval_secs: Number(document.getElementById("check-interval").value),
    enabled: document.getElementById("check-enabled").checked,
    checkpoints,
  };
}

function runStatusBadge(status) {
  if (status === "up") return '<span class="badge up">正常</span>';
  if (status === "down") return '<span class="badge down">异常</span>';
  return `<span class="badge unknown">${escapeHtml(status || "未知")}</span>`;
}

function formatTime(iso) {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

function updateHistoryMeta() {
  const el = document.getElementById("history-meta");
  const shown = Math.min(historyState.offset, historyState.total);
  el.textContent =
    historyState.total === 0
      ? "暂无历史记录（新检测开始后会自动写入）"
      : `共 ${historyState.total} 条，已显示 ${shown} 条`;
}

async function loadHistory(append) {
  if (!historyState.checkId) return;
  const offset = append ? historyState.offset : 0;
  const data = await api(
    `/api/checks/${historyState.checkId}/history?limit=${historyState.limit}&offset=${offset}`
  );
  historyState.total = data.total;
  historyState.offset = offset + data.items.length;

  const tbody = document.getElementById("history-body");
  const rows = data.items
    .map(
      (r) => `
    <tr>
      <td>${formatTime(r.checked_at)}</td>
      <td>${runStatusBadge(r.status)}</td>
      <td>${r.response_ms != null ? r.response_ms + " ms" : "—"}</td>
      <td class="url-cell">${r.error ? escapeHtml(r.error) : "—"}</td>
    </tr>
    <tr class="history-messages-row">
      <td colspan="4">
        <details class="run-messages">
          <summary>请求 / 响应报文</summary>
          <div class="message-pair">
            <div class="message-block">
              <strong>请求报文</strong>
              <pre>${escapeHtml(r.request_message || "—")}</pre>
            </div>
            <div class="message-block">
              <strong>响应报文</strong>
              <pre>${escapeHtml(r.response_message || "（无响应，可能为连接失败）")}</pre>
            </div>
          </div>
        </details>
      </td>
    </tr>`
    )
    .join("");

  if (append) {
    tbody.insertAdjacentHTML("beforeend", rows);
  } else {
    tbody.innerHTML =
      rows || '<tr><td colspan="4" style="color:var(--muted)">暂无记录</td></tr>';
  }

  updateHistoryMeta();
  const moreBtn = document.getElementById("history-more");
  moreBtn.hidden = historyState.offset >= historyState.total;
}

async function openHistory(checkId) {
  const check = await api("/api/checks/" + checkId);
  historyState = { checkId, name: check.name, offset: 0, total: 0, limit: 50 };
  document.getElementById("history-title").textContent = `检测历史 · ${check.name}`;
  document.getElementById("history-panel").hidden = false;
  await loadHistory(false);
  document.getElementById("history-panel").scrollIntoView({ behavior: "smooth", block: "start" });
}

document.getElementById("history-close").addEventListener("click", () => {
  document.getElementById("history-panel").hidden = true;
  historyState.checkId = null;
});

document.getElementById("history-more").addEventListener("click", () => {
  loadHistory(true).catch((e) => toast(e.message));
});

async function loadChecks() {
  const checks = await api("/api/checks");
  const checksById = new Map(checks.map((c) => [String(c.id), c]));
  const tbody = document.getElementById("checks-body");
  if (!checks.length) {
    tbody.innerHTML =
      '<tr><td colspan="6" style="color:var(--muted)">暂无条目，请在上方添加</td></tr>';
    return;
  }
  tbody.innerHTML = checks
    .map(
      (c) => `
    <tr>
      <td>${escapeHtml(c.name)}${c.enabled ? "" : ' <span class="badge unknown">已停用</span>'}</td>
      <td class="url-cell">${escapeHtml(c.url)}</td>
      <td>${statusBadge(c)}</td>
      <td>${c.last_response_ms != null ? c.last_response_ms + " ms" : "—"}</td>
      <td>${formatTime(c.last_checked_at)}</td>
      <td>
        <button type="button" class="btn-link" data-edit="${c.id}">编辑</button>
        <button type="button" class="btn-link" data-history="${c.id}">历史</button>
        <button type="button" class="btn-link" data-run="${c.id}">立即检测</button>
        <button type="button" class="btn-link" data-toggle="${c.id}" data-enabled="${c.enabled}">
          ${c.enabled ? "停用" : "启用"}
        </button>
        <button type="button" class="btn-link danger" data-delete="${c.id}">删除</button>
      </td>
    </tr>`
    )
    .join("");

  tbody.querySelectorAll("[data-edit]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const check = checksById.get(btn.dataset.edit);
      if (check) beginEditCheck(check);
    });
  });

  tbody.querySelectorAll("[data-history]").forEach((btn) => {
    btn.addEventListener("click", () => {
      openHistory(btn.dataset.history).catch((e) => toast(e.message));
    });
  });

  tbody.querySelectorAll("[data-run]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const id = btn.dataset.run;
      btn.disabled = true;
      try {
        const updated = await api("/api/checks/" + id + "/run", { method: "POST" });
        const label =
          updated.last_status === "up"
            ? "正常"
            : updated.last_status === "down"
              ? "异常"
              : updated.last_status || "未知";
        toast(`检测完成：${label}`);
        loadChecks();
        if (historyState.checkId === id) {
          loadHistory(false).catch(() => {});
        }
      } catch (err) {
        toast("检测失败: " + err.message);
      } finally {
        btn.disabled = false;
      }
    });
  });

  tbody.querySelectorAll("[data-delete]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      if (!confirm("确定删除该检查？")) return;
      try {
        const deletedId = btn.dataset.delete;
        await api("/api/checks/" + deletedId, { method: "DELETE" });
        if (String(editingCheckId) === deletedId) {
          resetCheckForm();
        }
        toast("已删除");
        loadChecks();
      } catch (err) {
        toast(err.message);
      }
    });
  });

  tbody.querySelectorAll("[data-toggle]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const id = btn.dataset.toggle;
      const enabled = btn.dataset.enabled !== "true";
      try {
        await api("/api/checks/" + id, {
          method: "PUT",
          body: JSON.stringify({ enabled }),
        });
        loadChecks();
      } catch (err) {
        toast(err.message);
      }
    });
  });
}

document.getElementById("checkpoint-add").addEventListener("click", () => {
  const rows = readCheckpointsFromDom();
  if (!rows.length && document.getElementById("checkpoint-body").querySelector("td[colspan]")) {
    rows.length = 0;
  }
  rows.push({ kind: "contains", value: "", enabled: true });
  renderCheckpointRows(rows);
});

document.getElementById("check-cancel-edit").addEventListener("click", () => {
  resetCheckForm();
  toast("已取消编辑");
});

document.getElementById("check-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const payload = readCheckFormPayload();
  try {
    if (editingCheckId) {
      await api("/api/checks/" + editingCheckId, {
        method: "PUT",
        body: JSON.stringify(payload),
      });
      toast("检查已更新");
    } else {
      await api("/api/checks", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      toast("检查已添加");
    }
    resetCheckForm();
    loadChecks();
  } catch (err) {
    toast((editingCheckId ? "保存失败: " : "添加失败: ") + err.message);
  }
});

document.getElementById("btn-refresh").addEventListener("click", () => {
  loadChecks().then(() => toast("已刷新"));
});

function bindAlertTests() {
  const feishuBtn = document.getElementById("feishu-test");
  const pushplusBtn = document.getElementById("pushplus-test");
  if (feishuBtn) {
    feishuBtn.addEventListener("click", async () => {
      feishuBtn.disabled = true;
      try {
        await api("/api/feishu/test", { method: "POST", body: "{}" });
        toast("飞书测试已发送，请在群内查看");
      } catch (err) {
        toast("飞书测试失败: " + err.message);
      } finally {
        feishuBtn.disabled = false;
      }
    });
  }
  if (pushplusBtn) {
    pushplusBtn.addEventListener("click", async () => {
      pushplusBtn.disabled = true;
      try {
        await api("/api/pushplus/test", { method: "POST", body: "{}" });
        toast("PushPlus 测试已提交，请在微信服务号查看");
      } catch (err) {
        toast("PushPlus 测试失败: " + err.message);
      } finally {
        pushplusBtn.disabled = false;
      }
    });
  }
}

function updateAlertHistoryMeta() {
  const el = document.getElementById("alert-history-meta");
  const shown = Math.min(alertHistoryState.offset, alertHistoryState.total);
  el.textContent =
    alertHistoryState.total === 0
      ? "暂无告警发送记录"
      : `共 ${alertHistoryState.total} 条，已显示 ${shown} 条`;
}

function renderAlertStatus(status) {
  if (status === "ok") {
    return '<span class="status-pill ok">成功</span>';
  }
  return '<span class="status-pill failed">失败</span>';
}

function alertHistoryQueryParams(offset) {
  const params = new URLSearchParams({
    limit: String(alertHistoryState.limit),
    offset: String(offset),
  });
  if (alertHistoryState.kind) {
    params.set("kind", alertHistoryState.kind);
  }
  return params.toString();
}

function setAlertKindFilter(kind) {
  alertHistoryState.kind = kind || "";
  document.querySelectorAll("#alert-kind-filters .chip").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.kind === alertHistoryState.kind);
  });
}

async function loadAlertHistory(append = false) {
  const offset = append ? alertHistoryState.offset : 0;
  const data = await api(`/api/alerts/history?${alertHistoryQueryParams(offset)}`);
  alertHistoryState.total = data.total;
  alertHistoryState.offset = offset + data.items.length;

  const tbody = document.getElementById("alert-history-body");
  if (!append) tbody.innerHTML = "";

  for (const row of data.items) {
    const tr = document.createElement("tr");
    const kind = ALERT_KIND_LABELS[row.kind] || row.kind;
    const channel = ALERT_CHANNEL_LABELS[row.channel] || row.channel;
    const checkLabel = row.check_name || (row.check_id ? `#${row.check_id}` : "—");
    const title = row.title ? `<strong>${escapeHtml(row.title)}</strong><br>` : "";
    const err = row.error
      ? `<div class="alert-err">${escapeHtml(row.error)}</div>`
      : "";
    tr.innerHTML = `
      <td>${escapeHtml(formatTime(row.sent_at))}</td>
      <td>${escapeHtml(kind)}</td>
      <td>${escapeHtml(channel)}</td>
      <td>${renderAlertStatus(row.status)}</td>
      <td>${escapeHtml(checkLabel)}</td>
      <td class="alert-msg-cell">${title}${escapeHtml(row.message)}${err}</td>
    `;
    tbody.appendChild(tr);
  }

  document.getElementById("alert-history-more").hidden =
    alertHistoryState.offset >= alertHistoryState.total;
  updateAlertHistoryMeta();
}

document.getElementById("alert-history-refresh").addEventListener("click", () => {
  loadAlertHistory(false).catch((e) => toast(e.message));
});

document.getElementById("alert-history-more").addEventListener("click", () => {
  loadAlertHistory(true).catch((e) => toast(e.message));
});

document.getElementById("alert-kind-filters").addEventListener("click", (e) => {
  const btn = e.target.closest(".chip");
  if (!btn) return;
  setAlertKindFilter(btn.dataset.kind || "");
  loadAlertHistory(false).catch((err) => toast(err.message));
});

bindAlertTests();
renderCheckpointRows([]);

loadAlertHistory().catch((e) => toast(e.message));
loadChecks().catch((e) => toast(e.message));
setInterval(() => loadChecks().catch(() => {}), 15000);
