const cp = require("node:child_process");
const vscode = require("vscode");

function workspaceCwd() {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    throw new Error("Open a workspace folder before running Autoresearch commands.");
  }
  return folders[0].uri.fsPath;
}

function binaryPath() {
  return vscode.workspace
    .getConfiguration("autoresearch")
    .get("binaryPath", "autoresearch");
}

function runAutoresearch(args) {
  return new Promise((resolve, reject) => {
    cp.execFile(binaryPath(), args, { cwd: workspaceCwd() }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(stderr || error.message));
        return;
      }
      resolve(stdout || stderr);
    });
  });
}

async function showDocument(title, language, content) {
  const doc = await vscode.workspace.openTextDocument({
    content,
    language
  });
  await vscode.window.showTextDocument(doc, {
    preview: true,
    viewColumn: vscode.ViewColumn.Beside
  });
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  vscode.window.setStatusBarMessage(title, 3000);
}

async function showStatus() {
  const cwd = workspaceCwd();
  const output = await runAutoresearch(["status", "--summary", "--cwd", cwd]);
  await showDocument("Autoresearch status", "json", output);
}

async function showDashboard() {
  const cwd = workspaceCwd();
  const output = await runAutoresearch(["dashboard", "--once", "--cwd", cwd]);
  await showDocument("Autoresearch dashboard", "plaintext", output);
}

function watchResults() {
  const terminal = vscode.window.createTerminal("Autoresearch Watch");
  const cwd = workspaceCwd().replace(/"/g, '\\"');
  terminal.sendText(`"${binaryPath()}" watch --format jsonl --cwd "${cwd}"`);
  terminal.show();
}

function register(context, command, handler) {
  context.subscriptions.push(
    vscode.commands.registerCommand(command, async () => {
      try {
        await handler();
      } catch (error) {
        vscode.window.showErrorMessage(error.message || String(error));
      }
    })
  );
}

function activate(context) {
  register(context, "autoresearch.showStatus", showStatus);
  register(context, "autoresearch.showDashboard", showDashboard);
  register(context, "autoresearch.watchResults", watchResults);
}

function deactivate() {}

module.exports = {
  activate,
  deactivate
};
