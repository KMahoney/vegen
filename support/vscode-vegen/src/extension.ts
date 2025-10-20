import * as vscode from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const serverExecutable = resolveServerExecutable();

  if (!serverExecutable) {
    vscode.window.showErrorMessage(
      "VeGen: unable to locate the language server binary. Set `vegen.serverPath` in your settings."
    );
    return;
  }

  const serverArgs = resolveServerArgs();
  const executable: Executable = {
    command: serverExecutable,
    args: ["lsp", ...serverArgs],
    options: {
      env: {
        ...process.env,
      },
    },
  };

  const serverOptions = {
    run: executable,
    debug: executable,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "vg" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.vg"),
    },
  };

  client = new LanguageClient(
    "vegen-language-server",
    "VeGen Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push({
    dispose: () => {
      if (client) {
        void client.stop();
      }
    },
  });

  client.start().catch((err) => {
    vscode.window.showErrorMessage(
      `VeGen: failed to start language server: ${err}`
    );
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

function resolveServerArgs(): string[] {
  const config = vscode.workspace.getConfiguration("vegen");
  const args = config.get<string[]>("serverArgs") ?? [];
  return Array.isArray(args) ? args : [];
}

function resolveServerExecutable(): string | undefined {
  const config = vscode.workspace.getConfiguration("vegen");
  const explicitPath = config.get<string>("serverPath")?.trim();

  if (explicitPath && explicitPath.length > 0) {
    return explicitPath;
  }

  return "vegen";
}
