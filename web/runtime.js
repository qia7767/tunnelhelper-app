(() => {
  const tauriInvoke = window.__TAURI__?.core?.invoke;
  const isTauri = typeof tauriInvoke === "function";
  document.documentElement.dataset.runtime = isTauri ? "tauri" : "browser";

  async function browserInvoke(command, args = {}, allowTokenPrompt = true) {
    const token = sessionStorage.getItem("tunnelHelper.accessToken") || "";
    const headers = { "Content-Type": "application/json" };
    if (token) headers["X-Tunnel-Token"] = token;

    const response = await fetch(`/api/invoke/${encodeURIComponent(command)}`, {
      method: "POST",
      credentials: "same-origin",
      headers,
      body: JSON.stringify(args ?? {}),
    });

    if (response.status === 401 && allowTokenPrompt) {
      sessionStorage.removeItem("tunnelHelper.accessToken");
      const supplied = window.prompt("Tunnel Helper Access Token");
      if (supplied) {
        sessionStorage.setItem("tunnelHelper.accessToken", supplied.trim());
        return browserInvoke(command, args, false);
      }
    }

    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      throw new Error(body.error || `HTTP ${response.status}`);
    }
    return body;
  }

  window.TunnelBridge = {
    isTauri,
    invoke: isTauri ? tauriInvoke : browserInvoke,
  };
})();
