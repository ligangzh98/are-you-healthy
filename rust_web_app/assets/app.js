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

function formatTime(iso) {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

async function loadFeishu() {
  const cfg = await api("/api/feishu");
  document.getElementById("feishu-webhook").value = cfg.webhook_url || "";
  document.getElementById("feishu-cooldown").value = cfg.alert_cooldown_secs || 300;
  document.getElementById("feishu-enabled").checked = !!cfg.enabled;
}

document.getElementById("feishu-test").addEventListener("click", async () => {
  const btn = document.getElementById("feishu-test");
  btn.disabled = true;
  try {
    await api("/api/feishu/test", {
      method: "POST",
      body: JSON.stringify({
        webhook_url: document.getElementById("feishu-webhook").value.trim(),
      }),
    });
    toast("测试消息已发送，请在飞书群查看");
  } catch (err) {
    toast("测试失败: " + err.message);
  } finally {
    btn.disabled = false;
  }
});

document.getElementById("feishu-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    await api("/api/feishu", {
      method: "PUT",
      body: JSON.stringify({
        webhook_url: document.getElementById("feishu-webhook").value.trim(),
        alert_cooldown_secs: Number(document.getElementById("feishu-cooldown").value),
        enabled: document.getElementById("feishu-enabled").checked,
      }),
    });
    toast("飞书配置已保存");
  } catch (err) {
    toast("保存失败: " + err.message);
  }
});

async function loadChecks() {
  const checks = await api("/api/checks");
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
        <button type="button" class="btn-link" data-toggle="${c.id}" data-enabled="${c.enabled}">
          ${c.enabled ? "停用" : "启用"}
        </button>
        <button type="button" class="btn-link danger" data-delete="${c.id}">删除</button>
      </td>
    </tr>`
    )
    .join("");

  tbody.querySelectorAll("[data-delete]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      if (!confirm("确定删除该检查？")) return;
      try {
        await api("/api/checks/" + btn.dataset.delete, { method: "DELETE" });
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

document.getElementById("check-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    await api("/api/checks", {
      method: "POST",
      body: JSON.stringify({
        name: document.getElementById("check-name").value.trim(),
        url: document.getElementById("check-url").value.trim(),
        method: document.getElementById("check-method").value,
        expected_status: Number(document.getElementById("check-status").value),
        interval_secs: Number(document.getElementById("check-interval").value),
        enabled: document.getElementById("check-enabled").checked,
      }),
    });
    e.target.reset();
    document.getElementById("check-status").value = "200";
    document.getElementById("check-interval").value = "60";
    document.getElementById("check-enabled").checked = true;
    toast("检查已添加");
    loadChecks();
  } catch (err) {
    toast("添加失败: " + err.message);
  }
});

document.getElementById("btn-refresh").addEventListener("click", () => {
  loadChecks().then(() => toast("已刷新"));
});

loadFeishu().catch((e) => toast(e.message));
loadChecks().catch((e) => toast(e.message));
setInterval(() => loadChecks().catch(() => {}), 15000);
