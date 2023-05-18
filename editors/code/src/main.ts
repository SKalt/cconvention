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
      // FIXME: figure out how to bundle the server binary with the ts code
      "..",
      "..",
      "target",
      "debug", // FIXME: use the prod build
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
      { scheme: "file", pattern: "COMMIT_EDITMSG" },
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
}
