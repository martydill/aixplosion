const READONLY_TOOLS = ["search_in_files", "glob"];

const state = {
  conversations: [],
  activeConversationId: null,
  plans: [],
  activePlanId: null,
  mcpServers: [],
  activeServer: null,
  agents: [],
  activeAgent: null,
  activeAgentEditing: null,
  theme: "dark",
  activeTab: "chats",
};

function setPlanForm(plan) {
  state.activePlanId = plan.id;
  document.getElementById("plan-title").value = plan.title || "";
  document.getElementById("plan-user-request").value = plan.user_request || "";
  document.getElementById("plan-markdown").value = plan.plan_markdown || "";
  renderPlanList();
}

function applyTheme(theme) {
  state.theme = theme;
  if (theme === "light") {
    document.documentElement.classList.add("light");
  } else {
    document.documentElement.classList.remove("light");
  }
  const btn = document.getElementById("mode-toggle");
  if (btn) {
    btn.textContent = theme === "light" ? "☾" : "☀";
    btn.title = theme === "light" ? "Switch to dark mode" : "Switch to light mode";
    btn.setAttribute("aria-label", btn.title);
  }
  localStorage.setItem("aixplosion-theme", theme);
}

function setStatus(text) {
  document.getElementById("conversation-meta").textContent = text;
}

async function api(path, options = {}) {
  const opts = { headers: { "Content-Type": "application/json" }, ...options };
  if (opts.body && typeof opts.body !== "string") {
    opts.body = JSON.stringify(opts.body);
  }

  const res = await fetch(path, opts);
  if (!res.ok) {
    const message = await res.text();
    throw new Error(message || `Request failed: ${res.status}`);
  }
  const contentType = res.headers.get("content-type") || "";
  if (contentType.includes("application/json")) {
    return res.json();
  }
  return res.text();
}

function renderConversationList() {
  const list = document.getElementById("conversation-list");
  list.innerHTML = "";
  state.conversations.forEach((conv) => {
    const item = document.createElement("div");
    item.className = "list-item" + (conv.id === state.activeConversationId ? " active" : "");
    item.innerHTML = `
      <div style="font-weight:600;">${conv.last_message ? conv.last_message.slice(0, 50) : "new chat"}</div>
      <small>${new Date(conv.updated_at).toLocaleString()} • ${conv.model}</small>
    `;
    item.addEventListener("click", () => selectConversation(conv.id));
    list.appendChild(item);
  });
}

function renderMessages(messages) {
  const container = document.getElementById("messages");
  container.innerHTML = "";
  messages.forEach((msg) => {
    const bubble = document.createElement("div");
    bubble.className = `bubble ${msg.role}`;
    bubble.textContent = msg.content;
    container.appendChild(bubble);
  });
  container.scrollTop = container.scrollHeight;
}

function appendMessage(role, content) {
  const container = document.getElementById("messages");
  const bubble = document.createElement("div");
  bubble.className = `bubble ${role}`;
  bubble.textContent = content;
  container.appendChild(bubble);
  container.scrollTop = container.scrollHeight;
}

async function loadConversations() {
  const data = await api("/api/conversations");
  mergeConversations(data);
  renderConversationList();
  if (!state.activeConversationId && data.length > 0) {
    await selectConversation(data[0].id);
  }
}

async function selectConversation(id) {
  state.activeConversationId = id;
  renderConversationList();
  setStatus("Loading conversation...");
  const detail = await api(`/api/conversations/${id}`);
  const meta = detail.conversation;
  setStatus(`${detail.messages.length} messages • ${meta.model}`);
  renderMessages(detail.messages);
}

async function createConversation() {
  setStatus("Creating conversation...");
  const res = await api("/api/conversations", { method: "POST", body: {} });
  const newId = res.id;
  const placeholder = {
    id: newId,
    last_message: null,
    updated_at: new Date().toISOString(),
    model: res.model || state.conversations[0]?.model || "glm-4.6",
    created_at: new Date().toISOString(),
  };
  state.activeConversationId = newId;
  mergeConversations([placeholder]);
  if (newId) {
    await selectConversation(newId);
  }
  const input = document.getElementById("message-input");
  if (input) input.focus();
  await loadConversations();
}

async function sendMessage() {
  const input = document.getElementById("message-input");
  const text = input.value.trim();
  if (!text || !state.activeConversationId) return;

  appendMessage("user", text);
  updateConversationPreview(state.activeConversationId, text);
  input.value = "";
  setStatus("Waiting for response...");
  try {
    const result = await api(`/api/conversations/${state.activeConversationId}/message`, {
      method: "POST",
      body: { message: text },
    });
    appendMessage("assistant", result.response || "(empty response)");
    setStatus("Ready");
    await loadConversations();
  } catch (err) {
    appendMessage("assistant", `Error: ${err.message}`);
    setStatus("Error");
  }
}

// Plans
async function loadPlans() {
  const plans = await api("/api/plans");
  state.plans = plans;
  renderPlanList();
}

function renderPlanList() {
  const list = document.getElementById("plan-list");
  list.innerHTML = "";
  state.plans.forEach((plan) => {
    const item = document.createElement("div");
    item.className = "list-item" + (plan.id === state.activePlanId ? " active" : "");
    item.innerHTML = `
      <div style="font-weight:600;">${plan.title || "Untitled plan"}</div>
      <small>${new Date(plan.created_at).toLocaleString()}</small>
    `;
    item.addEventListener("click", () => {
      setPlanForm(plan);
    });
    list.appendChild(item);
  });
}

function mergeConversations(incoming) {
  const map = new Map();
  incoming.forEach((c) => map.set(c.id, c));
  state.conversations.forEach((c) => {
    if (!map.has(c.id)) map.set(c.id, c);
  });
  const merged = Array.from(map.values());
  merged.sort(
    (a, b) =>
      new Date(b.updated_at || b.created_at || 0) - new Date(a.updated_at || a.created_at || 0),
  );
  state.conversations = merged;
}

function updateConversationPreview(id, lastMessage) {
  if (!id) return;
  const existing = state.conversations.find((c) => c.id === id);
  const now = new Date().toISOString();
  if (existing) {
    existing.last_message = lastMessage;
    existing.updated_at = now;
  } else {
    state.conversations.unshift({
      id,
      last_message: lastMessage,
      updated_at: now,
      created_at: now,
      model: "glm-4.6",
    });
  }
  renderConversationList();
}

function resetPlanForm() {
  state.activePlanId = null;
  document.getElementById("plan-title").value = "";
  document.getElementById("plan-user-request").value = "";
  document.getElementById("plan-markdown").value = "";
}

function resetMcpForm() {
  state.activeServer = null;
  document.getElementById("mcp-name").value = "";
  document.getElementById("mcp-command").value = "";
  document.getElementById("mcp-args").value = "";
  document.getElementById("mcp-url").value = "";
  document.getElementById("mcp-env").value = "";
  document.getElementById("mcp-enabled").value = "true";
  renderMcpList();
  document.getElementById("connect-mcp-detail").style.display = "none";
  document.getElementById("disconnect-mcp-detail").style.display = "none";
  document.getElementById("delete-mcp-detail").style.display = "none";
}

function setMcpForm(server) {
  if (!server) {
    resetMcpForm();
    return;
  }
  state.activeServer = server.name;
  document.getElementById("mcp-name").value = server.name;
  document.getElementById("mcp-command").value = server.config.command || "";
  document.getElementById("mcp-args").value = (server.config.args || []).join(" ");
  document.getElementById("mcp-url").value = server.config.url || "";
  const env = server.config.env || {};
  document.getElementById("mcp-env").value = Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join(", ");
  document.getElementById("mcp-enabled").value = String(server.config.enabled);
  document.getElementById("connect-mcp-detail").style.display = "";
  document.getElementById("disconnect-mcp-detail").style.display = "";
  document.getElementById("delete-mcp-detail").style.display = "";
}
async function savePlan() {
  if (!state.activePlanId) return;
  const payload = {
    title: document.getElementById("plan-title").value,
    user_request: document.getElementById("plan-user-request").value,
    plan_markdown: document.getElementById("plan-markdown").value,
  };
  await api(`/api/plans/${state.activePlanId}`, { method: "PUT", body: payload });
  await loadPlans();
}

async function createPlan() {
  resetPlanForm();
  const payload = {
    title: "",
    user_request: "",
    plan_markdown: "",
    conversation_id: state.activeConversationId,
  };
  const res = await api("/api/plans", { method: "POST", body: payload });
  state.activePlanId = res.id;
  await loadPlans();
}

async function deletePlan() {
  if (!state.activePlanId) return;
  await api(`/api/plans/${state.activePlanId}`, { method: "DELETE" });
  resetPlanForm();
  await loadPlans();
  selectFirstPlan();
}

// MCP
async function loadMcp() {
  state.mcpServers = await api("/api/mcp/servers");
  renderMcpList();
}

function renderMcpList() {
  const list = document.getElementById("mcp-list");
  list.innerHTML = "";
  state.mcpServers.forEach((server) => {
    const item = document.createElement("div");
    item.className = "list-item" + (server.name === state.activeServer ? " active" : "");
    const status = server.connected ? "Connected" : server.config.enabled ? "Ready" : "Disabled";
    item.innerHTML = `
      <div class="flex-between">
        <div>
          <div style="font-weight:700;">${server.name}</div>
          <small class="muted">${status}</small>
        </div>
      </div>
      <div class="muted" style="margin-top:6px;">${server.config.url || server.config.command || "No endpoint configured"}</div>
    `;
    item.addEventListener("click", () => {
      setMcpForm(server);
      renderMcpList();
    });
    list.appendChild(item);
  });
}

async function saveMcpServer() {
  const name = document.getElementById("mcp-name").value.trim();
  if (!name) return;
  const payload = {
    name,
    command: document.getElementById("mcp-command").value.trim() || null,
    args: document
      .getElementById("mcp-args")
      .value.trim()
      .split(" ")
      .filter(Boolean),
    url: document.getElementById("mcp-url").value.trim() || null,
    env: parseEnv(document.getElementById("mcp-env").value),
    enabled: document.getElementById("mcp-enabled").value === "true",
  };
  await api(`/api/mcp/servers/${name}`, { method: "PUT", body: payload });
  await loadMcp();
}

function parseEnv(text) {
  const env = {};
  text
    .split(",")
    .map((p) => p.trim())
    .filter(Boolean)
    .forEach((pair) => {
      const [k, v] = pair.split("=");
      if (k && v !== undefined) env[k.trim()] = v.trim();
    });
  return env;
}

// Agents
async function loadAgents() {
  state.agents = await api("/api/agents");
  const active = await api("/api/agents/active");
  state.activeAgent = active.active;
  renderAgents();
  renderAgentSelector();
}

function renderAgents() {
  const list = document.getElementById("agent-list");
  list.innerHTML = "";
  state.agents.forEach((agent) => {
    const item = document.createElement("div");
    item.className = "list-item" + (agent.name === state.activeAgentEditing ? " active" : "");
    item.innerHTML = `
      <div style="font-weight:700;">${agent.name}</div>
      <small class="muted">${agent.model || "model inherits"} • ${agent.allowed_tools.length} allowed</small>
    `;
    item.addEventListener("click", () => {
      setAgentForm(agent);
      renderAgents();
    });
    list.appendChild(item);
  });
}

function resetAgentForm() {
  state.activeAgentEditing = null;
  document.getElementById("agent-name").value = "";
  document.getElementById("agent-model").value = "";
  document.getElementById("agent-temp").value = "";
  document.getElementById("agent-max-tokens").value = "";
  document.getElementById("agent-allowed").value = READONLY_TOOLS.join(", ");
  document.getElementById("agent-denied").value = "";
  document.getElementById("agent-prompt").value = "";
  document.getElementById("activate-agent").style.display = "none";
  document.getElementById("deactivate-agent").style.display = "none";
  document.getElementById("delete-agent").style.display = "none";
  renderAgents();
}

function setAgentForm(agent) {
  state.activeAgentEditing = agent.name;
  document.getElementById("agent-name").value = agent.name;
  document.getElementById("agent-model").value = agent.model || "";
  document.getElementById("agent-temp").value = agent.temperature ?? "";
  document.getElementById("agent-max-tokens").value = agent.max_tokens ?? "";
  document.getElementById("agent-allowed").value = agent.allowed_tools.join(", ");
  document.getElementById("agent-denied").value = agent.denied_tools.join(", ");
  document.getElementById("agent-prompt").value = agent.system_prompt;
  document.getElementById("activate-agent").style.display = "";
  document.getElementById("deactivate-agent").style.display = "";
  document.getElementById("delete-agent").style.display = "";
}

function selectFirstConversation() {
  if (state.conversations.length > 0) {
    selectConversation(state.conversations[0].id);
  }
}

function selectFirstPlan() {
  if (state.plans.length > 0) {
    setPlanForm(state.plans[0]);
  } else {
    resetPlanForm();
  }
}

function selectFirstMcp() {
  if (state.mcpServers.length > 0) {
    setMcpForm(state.mcpServers[0]);
    renderMcpList();
  } else {
    resetMcpForm();
  }
}

function selectFirstAgent() {
  if (state.agents.length > 0) {
    setAgentForm(state.agents[0]);
    renderAgents();
  } else {
    resetAgentForm();
  }
}

async function saveAgent() {
  const payload = {
    system_prompt: document.getElementById("agent-prompt").value,
    allowed_tools: splitList(document.getElementById("agent-allowed").value),
    denied_tools: splitList(document.getElementById("agent-denied").value),
    max_tokens: numberOrNull(document.getElementById("agent-max-tokens").value),
    temperature: numberOrNull(document.getElementById("agent-temp").value),
    model: document.getElementById("agent-model").value || null,
  };
  const name = document.getElementById("agent-name").value.trim();
  if (!name) return;

  if (state.agents.some((a) => a.name === name)) {
    await api(`/api/agents/${name}`, { method: "PUT", body: payload });
  } else {
    await api("/api/agents", { method: "POST", body: { ...payload, name } });
  }
  state.activeAgentEditing = name;
  await loadAgents();
}

async function activateAgent(name) {
  await api("/api/agents/active", { method: "POST", body: { name } });
  await loadAgents();
}

async function deleteAgent() {
  const name = document.getElementById("agent-name").value.trim();
  if (!name) return;
  const idx = state.agents.findIndex((a) => a.name === name);
  await api(`/api/agents/${name}`, { method: "DELETE" });
  state.activeAgentEditing = null;
  await loadAgents();
  if (state.agents.length > 0) {
    const next = state.agents[Math.min(Math.max(idx, 0), state.agents.length - 1)];
    setAgentForm(next);
    renderAgents();
  } else {
    resetAgentForm();
  }
}

function splitList(text) {
  return text
    .split(",")
    .map((v) => v.trim())
    .filter(Boolean);
}

function numberOrNull(val) {
  const num = parseFloat(val);
  return isNaN(num) ? null : num;
}

function renderAgentSelector() {
  const select = document.getElementById("agent-selector");
  if (!select) return;
  select.innerHTML = "";
  const optDefault = document.createElement("option");
  optDefault.value = "";
  optDefault.textContent = "Default agent";
  select.appendChild(optDefault);
  state.agents.forEach((agent) => {
    const opt = document.createElement("option");
    opt.value = agent.name;
    opt.textContent = agent.name;
    select.appendChild(opt);
  });
  select.value = state.activeAgent || "";
}

// Tabs
function initTabs() {
  document.querySelectorAll(".top-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const target = btn.dataset.tab;
      state.activeTab = target;
      const url = new URL(window.location);
      url.searchParams.set("tab", target);
      window.history.replaceState({}, "", url);
      document.querySelectorAll(".top-tab").forEach((b) => b.classList.remove("active"));
      document.querySelectorAll(".tab-content").forEach((tab) => tab.classList.remove("active"));
      btn.classList.add("active");
      document.getElementById(`tab-${target}`).classList.add("active");

      switch (target) {
        case "plans":
          selectFirstPlan();
          break;
        case "mcp":
          selectFirstMcp();
          break;
        case "agents":
          selectFirstAgent();
          break;
        default:
          selectFirstConversation();
          break;
      }
    });
  });
}

function initTheme() {
  const stored = localStorage.getItem("aixplosion-theme");
  const initial = stored === "light" || stored === "dark" ? stored : "dark";
  applyTheme(initial);
  const toggle = document.getElementById("mode-toggle");
  if (toggle) {
    toggle.addEventListener("click", () => {
      applyTheme(state.theme === "light" ? "dark" : "light");
    });
  }
}

function bindEvents() {
  document.getElementById("send-message").addEventListener("click", sendMessage);
  document.getElementById("message-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  });
  document.getElementById("new-conversation").addEventListener("click", createConversation);

  document.getElementById("save-plan").addEventListener("click", savePlan);
  document.getElementById("create-plan").addEventListener("click", createPlan);
  document.getElementById("create-plan-sidebar").addEventListener("click", createPlan);
  document.getElementById("delete-plan").addEventListener("click", deletePlan);

  document.getElementById("new-mcp").addEventListener("click", resetMcpForm);
  document.getElementById("save-mcp-detail").addEventListener("click", saveMcpServer);
  document.getElementById("connect-mcp-detail").addEventListener("click", async () => {
    const name = document.getElementById("mcp-name").value.trim();
    if (!name) return;
    await api(`/api/mcp/servers/${name}/connect`, { method: "POST" });
    await loadMcp();
  });
  document.getElementById("disconnect-mcp-detail").addEventListener("click", async () => {
    const name = document.getElementById("mcp-name").value.trim();
    if (!name) return;
    await api(`/api/mcp/servers/${name}/disconnect`, { method: "POST" });
    await loadMcp();
  });
  document.getElementById("delete-mcp-detail").addEventListener("click", async () => {
    const name = document.getElementById("mcp-name").value.trim();
    if (!name) return;
    const idx = state.mcpServers.findIndex((s) => s.name === name);
    await api(`/api/mcp/servers/${name}`, { method: "DELETE" });
    await loadMcp();
    if (state.mcpServers.length > 0) {
      const next = state.mcpServers[Math.min(Math.max(idx, 0), state.mcpServers.length - 1)];
      setMcpForm(next);
      renderMcpList();
    } else {
      resetMcpForm();
    }
  });

  document.getElementById("save-agent").addEventListener("click", saveAgent);
  document.getElementById("activate-agent").addEventListener("click", () => {
    const name = document.getElementById("agent-name").value.trim();
    if (name) activateAgent(name);
  });
  document.getElementById("deactivate-agent").addEventListener("click", () => activateAgent(null));
  document.getElementById("delete-agent").addEventListener("click", deleteAgent);
  document.getElementById("new-agent").addEventListener("click", resetAgentForm);
  document.getElementById("agent-selector").addEventListener("change", (e) => {
    const name = e.target.value || null;
    activateAgent(name);
  });
  document.getElementById("show-context").addEventListener("click", showContextModal);
  document.getElementById("close-context").addEventListener("click", closeContextModal);
  document.getElementById("context-modal").addEventListener("click", (e) => {
    if (e.target.id === "context-modal") closeContextModal();
  });
}

async function bootstrap() {
  // Restore tab from URL
  const url = new URL(window.location);
  const tabFromUrl = url.searchParams.get("tab");
  if (tabFromUrl && document.querySelector(`.top-tab[data-tab=\"${tabFromUrl}\"]`)) {
    state.activeTab = tabFromUrl;
    document.querySelectorAll(".top-tab").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".tab-content").forEach((tab) => tab.classList.remove("active"));
    document.querySelector(`.top-tab[data-tab=\"${tabFromUrl}\"]`).classList.add("active");
    document.getElementById(`tab-${tabFromUrl}`).classList.add("active");
  }

  initTabs();
  initTheme();
  bindEvents();
  try {
    await loadConversations();
    await loadPlans();
    await loadMcp();
    await loadAgents();
    switch (state.activeTab) {
      case "plans":
        selectFirstPlan();
        break;
      case "mcp":
        selectFirstMcp();
        break;
      case "agents":
        selectFirstAgent();
        break;
      default:
        selectFirstConversation();
        break;
    }
    setStatus("Ready");
  } catch (err) {
    setStatus(`Startup failed: ${err.message}`);
  }
}

bootstrap();

async function showContextModal() {
  if (!state.activeConversationId) return;
  try {
    setStatus("Loading context...");
    const detail = await api(`/api/conversations/${state.activeConversationId}`);
    const meta = detail.conversation;
    const lines = [];
    lines.push("Current Conversation Context");
    lines.push("-".repeat(50));
    lines.push("");

    const files = detail.context_files || [];
    if (files.length > 0) {
      lines.push("Context files:");
      files.forEach((f) => lines.push(`- ${f}`));
      lines.push("");
    }

    if (meta.system_prompt) {
      lines.push("System Prompt:");
      lines.push(`  ${meta.system_prompt}`);
      lines.push("");
    }

    if (!detail.messages.length) {
      lines.push("No context yet. Start a conversation to see context here.");
    } else {
      detail.messages.forEach((m, idx) => {
        const role = m.role.toUpperCase();
        const preview = m.content.replace(/\n/g, " ");
        const truncated = preview.length > 100 ? `${preview.slice(0, 100)}...` : preview;
        lines.push(`[${idx + 1}] ${role}: (1 content block)`);
        lines.push(`  ▶ Block 1: Text ${truncated}`);
        lines.push("");
      });
    }

    document.getElementById("context-content").textContent = lines.join("\n");
    document.getElementById("context-modal").classList.add("open");
  } catch (err) {
    setStatus(`Failed to load context: ${err.message}`);
  } finally {
    setStatus("Ready");
  }
}

function closeContextModal() {
  document.getElementById("context-modal").classList.remove("open");
}
