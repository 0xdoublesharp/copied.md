import { spawn } from 'node:child_process';
import { isAbsolute } from 'node:path';
import * as vscode from 'vscode';
import {
  convertMarkdown,
  targets,
  type ClipboardPayload,
  type CopyTarget,
} from '@0xdoublesharp/copied-md-converters';

const namedTargets: Record<string, CopyTarget> = {
  facebook: 'facebook',
  twitter: 'twitter',
  slack: 'slack-paste',
  discord: 'discord',
  telegram: 'telegram',
  sms: 'sms',
  reddit: 'reddit',
  html: 'html-source',
};

export function registerCommands(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.commands.registerCommand('copied-md.copyAs', () => copy()));
  for (const [suffix, target] of Object.entries(namedTargets)) {
    context.subscriptions.push(
      vscode.commands.registerCommand('copied-md.copyTo.' + suffix, () => copy(target)),
    );
  }
}

async function copy(target?: CopyTarget): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    void vscode.window.showWarningMessage('No active editor');
    return;
  }

  try {
    const selections = editor.selections
      .filter(({ isEmpty }) => !isEmpty)
      .sort((a, b) => a.start.compareTo(b.start));
    const wholeDocument = selections.length === 0 && editor.document.languageId === 'markdown';
    if (!selections.length && !wholeDocument) {
      void vscode.window.showWarningMessage('Select Markdown to copy.');
      return;
    }
    if (!target) {
      const choice = await vscode.window.showQuickPick(
        targets.map((item) => ({ label: item.label, target: item.id })),
        {
          placeHolder: wholeDocument ? 'Copy the Markdown document as…' : 'Copy the selection as…',
        },
      );
      if (!choice) return;
      target = choice.target;
    }

    const text = wholeDocument
      ? editor.document.getText()
      : selections.map((range) => editor.document.getText(range)).join('\n\n');
    if (!text.trim()) {
      void vscode.window.showWarningMessage('There is no text to copy.');
      return;
    }
    const result = convertMarkdown(text, target);
    const formats = result.representations;
    if (!formats['text/plain'].trim()) {
      void vscode.window.showWarningMessage('There is no text to copy.');
      return;
    }
    const rich = !!formats['text/html'];
    const label = targets.find((item) => item.id === target)!.label;
    const warning = result.diagnostics.find((item) => item.severity === 'warning');
    if (rich && (await writeRich(formats))) {
      void vscode.window.showInformationMessage(`Copied as ${label}.`);
    } else {
      await vscode.env.clipboard.writeText(formats['text/plain']);
      void vscode.window.showInformationMessage(
        rich
          ? 'Copied as plain text. Use the macOS Copy as… action for rich text.'
          : `Copied as ${label}.`,
      );
    }
    if (warning) void vscode.window.showWarningMessage(warning.message);
  } catch (error) {
    void vscode.window.showErrorMessage(
      'Copy failed: ' + (error instanceof Error ? error.message : String(error)),
    );
  }
}

async function writeRich(representations: ClipboardPayload): Promise<boolean> {
  if (
    process.platform !== 'darwin' ||
    vscode.env.remoteName ||
    vscode.env.uiKind !== vscode.UIKind.Desktop
  )
    return false;
  const executable = vscode.workspace.getConfiguration('copied-md').get<string>('pasteboardHelper');
  if (!executable || !isAbsolute(executable)) return false;
  return new Promise((resolve) => {
    const child = spawn(executable, ['write'], {
      stdio: ['pipe', 'ignore', 'ignore'],
      timeout: 5000,
    });
    child.on('error', () => resolve(false));
    child.on('close', (code) => resolve(code === 0));
    child.stdin.on('error', () => resolve(false));
    child.stdin.end(JSON.stringify({ representations }));
  });
}
