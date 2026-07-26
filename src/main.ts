import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import QRCode from "qrcode";
import "./style.css";

interface SharedFolder {
  name: string;
  path: string;
  active: boolean;
  allow_rename: boolean;
  allow_upload: boolean;
  allow_delete: boolean;
}

// ── Window controls ──
const win = getCurrentWindow();
document.getElementById("btn-minimize")?.addEventListener("click", () => win.minimize());
document.getElementById("btn-maximize")?.addEventListener("click", async () => {
  (await win.isMaximized()) ? win.unmaximize() : win.maximize();
});
document.getElementById("btn-close")?.addEventListener("click", () => win.close());

// ── State ──
let folders: SharedFolder[] = [];

// ── DOM refs ──
const folderList = document.getElementById("folder-list")!;
const btnAddFolder = document.getElementById("btn-add-folder")!;
const btnToggleServer = document.getElementById("btn-toggle-server")!;
const serverStatus = document.getElementById("server-status")!;
const urlDisplay = document.getElementById("url-display")!;
const qrCanvas = document.getElementById("qr-canvas") as HTMLCanvasElement;
const btnToggleLog = document.getElementById("btn-toggle-log")!;
const logPanel = document.getElementById("log-panel")!;
const logEntries = document.getElementById("log-entries")!;
const btnSettings = document.getElementById("btn-settings")!;
const settingsPopup = document.getElementById("settings-popup")!;
const inputPassword = document.getElementById("input-password") as HTMLInputElement;
const btnSetPwd = document.getElementById("btn-set-pwd")!;

// ── Settings gear toggle ──
let settingsOpen = false;
btnSettings.addEventListener("click", () => {
  settingsOpen = !settingsOpen;
  settingsPopup.style.display = settingsOpen ? "flex" : "none";
});

// ── Set password ──
btnSetPwd.addEventListener("click", async () => {
  const pwd = inputPassword.value.trim();
  try {
    await invoke("set_password", { password: pwd });
  } catch (e) { console.error(e); }
});

// ── Add folder ──
btnAddFolder.addEventListener("click", async () => {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  const path = selected as string;
  const name = path.split(/[\\/]/).pop() || path;
  try {
    folders = await invoke<SharedFolder[]>("add_folder", { name, path });
    renderFolders();
  } catch (e) { console.error(e); }
});

// ── Toggle server ──
btnToggleServer.addEventListener("click", async () => {
  try {
    const running = await invoke<boolean>("is_server_running");
    if (running) {
      await invoke("stop_server");
      btnToggleServer.textContent = "启动服务器";
      btnToggleServer.classList.remove("running");
      serverStatus.textContent = "● 未启动";
      serverStatus.className = "status-off";
    } else {
      await invoke("start_server");
      btnToggleServer.textContent = "停止服务器";
      btnToggleServer.classList.add("running");
      serverStatus.textContent = "● 运行中 :9726";
      serverStatus.className = "status-on";
    }
  } catch (e) { console.error(e); }
});

// ── Toggle folder permission ──
async function togglePerm(name: string, perm: string) {
  try {
    folders = await invoke<SharedFolder[]>("toggle_folder_perm", { name, perm });
    renderFolders();
  } catch (e) { console.error(e); }
}

// ── Render folder list ──
function renderFolders() {
  folderList.innerHTML = "";
  for (const f of folders) {
    const li = document.createElement("li");
    li.className = "folder-item";

    // Play/pause
    const playBtn = document.createElement("button");
    playBtn.className = `play-btn${f.active ? " active" : ""}`;
    playBtn.title = f.active ? "停止共享" : "开始共享";
    const playSvg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    playSvg.setAttribute("width", "14");
    playSvg.setAttribute("height", "14");
    playSvg.setAttribute("viewBox", "0 0 14 14");
    playSvg.setAttribute("fill", "none");
    if (f.active) {
      playSvg.innerHTML = '<rect x="2" y="2" width="3.5" height="10" rx="0.8" fill="currentColor"/><rect x="8.5" y="2" width="3.5" height="10" rx="0.8" fill="currentColor"/>';
    } else {
      playSvg.innerHTML = '<polygon points="3,1 13,7 3,13" fill="currentColor"/>';
    }
    playBtn.appendChild(playSvg);
    playBtn.addEventListener("click", async () => {
      folders = await invoke<SharedFolder[]>("toggle_folder", { name: f.name });
      renderFolders();
    });

    // Folder info
    const infoDiv = document.createElement("div");
    infoDiv.className = "folder-info";

    const nameSpan = document.createElement("span");
    nameSpan.className = "folder-name";
    nameSpan.textContent = f.name;
    nameSpan.title = f.path;
    infoDiv.appendChild(nameSpan);

    // Permission toggles
    const permsDiv = document.createElement("div");
    permsDiv.className = "perms";

    const permLabels = [
      { key: "rename", text: "重命名" },
      { key: "upload", text: "上传" },
      { key: "delete", text: "删除" },
    ];
    for (const pl of permLabels) {
      const btn = document.createElement("button");
      btn.className = `perm-btn${(f as any)["allow_" + pl.key] ? " on" : ""}`;
      btn.textContent = pl.text;
      btn.addEventListener("click", () => togglePerm(f.name, pl.key));
      permsDiv.appendChild(btn);
    }
    infoDiv.appendChild(permsDiv);

    // Remove button
    const removeBtn = document.createElement("button");
    removeBtn.className = "remove-btn";
    removeBtn.innerHTML = "&times;";
    removeBtn.title = "移除";
    removeBtn.addEventListener("click", async () => {
      folders = await invoke<SharedFolder[]>("remove_folder", { name: f.name });
      renderFolders();
    });

    li.appendChild(playBtn);
    li.appendChild(infoDiv);
    li.appendChild(removeBtn);
    folderList.appendChild(li);
  }
}

// ── QR code ──
async function updateQR() {
  try {
    const ip = await invoke<string>("get_local_ip");
    const url = `http://${ip}:9726`;
    urlDisplay.textContent = url;
    await QRCode.toCanvas(qrCanvas, url, { width: 200, margin: 2, color: { dark: "#1b4332", light: "#ffffff" } });
  } catch {
    urlDisplay.textContent = "http://0.0.0.0:9726";
    await QRCode.toCanvas(qrCanvas, "http://0.0.0.0:9726", { width: 200, margin: 2, color: { dark: "#1b4332", light: "#ffffff" } });
  }
}

// ── Log toggle ──
let logVisible = false;
btnToggleLog.addEventListener("click", async () => {
  logVisible = !logVisible;
  if (logVisible) {
    btnToggleLog.textContent = "隐藏日志";
    logPanel.style.display = "block";
    const logs = await invoke<string[]>("get_logs");
    logEntries.textContent = logs.join("\n") || "暂无日志";
  } else {
    btnToggleLog.textContent = "显示日志";
    logPanel.style.display = "none";
  }
});

// ── Init ──
updateQR();
(async () => {
  try {
    folders = await invoke<SharedFolder[]>("get_folders");
    renderFolders();
    const running = await invoke<boolean>("is_server_running");
    if (running) {
      btnToggleServer.textContent = "停止服务器";
      btnToggleServer.classList.add("running");
      serverStatus.textContent = "● 运行中 :9726";
      serverStatus.className = "status-on";
    }
    const pwd = await invoke<string | null>("get_password");
    if (pwd) inputPassword.value = pwd;
  } catch (_) {}
})();
