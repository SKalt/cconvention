import * as path from "path";
import type { ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient; // FIXME: avoid global variable
/**
 * activate the language client & server
 * @param context
 */
export async function activate(
  context: ExtensionContext
): Promise<LanguageClient> {
  const serverPath = context.asAbsolutePath(
    path.join(
      "..",
      "..",
      "target",
      "debug",
      "conventional-commit-language-server"
    )
  );
  const serverOptions: ServerOptions = {
    command: serverPath,
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "git-commit" },
      { scheme: "file", pattern: "GIT_COMMIT_EDITMSG" },
    ],
    // TODO: add synchronization options when we support config files
  };
  client = new LanguageClient(
    "gitConventionalCommitLs",
    "Git Conventional Commit Language Server",
    serverOptions,
    clientOptions
  );
  client.start();
  context.subscriptions.push(client);
  return client;
}

export async function deactivate() {
  return client.stop();
  // await setContextValue(RUST_PROJECT_CONTEXT_NAME, undefined);
}
