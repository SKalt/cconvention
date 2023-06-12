import type { ExtensionContext } from "vscode";
import { Uri, window } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

const DEFAULT_SERVER = "conventional-commit-language-server";
let client: LanguageClient; // FIXME: avoid global variable
function getServer(context: ExtensionContext): string {
  // TODO: check for a pre-installed binary?
  // TODO: consent for error-reporting
  return Uri.joinPath(context.extensionUri, "dist", DEFAULT_SERVER).fsPath;
}

const log = new (class {
  private readonly output = window.createOutputChannel(
    "Git Conventional Commit LS Client"
  );
  info(msg: any) {
    log.write("INFO", msg);
  }
  write(label: string, msg: string) {
    log.output.appendLine(`${label} [${new Date().toISOString()}] ${msg}`);
  }
})();

/**
 * activate the language client & server
 * @param context
 */
export async function activate(
  context: ExtensionContext
): Promise<LanguageClient> {
  const serverPath = getServer(context);
  log.info(`using server: ${serverPath}`);
  const serverOptions: ServerOptions = {
    command: serverPath,
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "git-commit" },
      { scheme: "file", pattern: "COMMIT_EDITMSG" },
    ],
  };
  client = new LanguageClient(
    "gitConventionalCommitLs",
    "Git Conventional Commit Language Server",
    serverOptions,
    clientOptions
  );
  // TODO: close the client after the file closes -- no need to keep the ext running
  // when there's no .git/COMMIT_EDITMSG open
  client.start();
  context.subscriptions.push(client);
  return client;
}

export async function deactivate() {
  return client?.stop();
}
