#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { convertMarkdown, resolveTarget, targets } from '@0xdoublesharp/copied-md-converters';

class CliError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly exitCode = 2,
  ) {
    super(message);
  }
}

const help = `Usage
  copied-md targets [--json]
  copied-md convert [--to TARGET] [--file PATH] [--format plain|html] [--json]
  copied-md copy [--to TARGET | --pick] [--file PATH | --clipboard] [--json]
  copied-md mcp

Convert and copy read stdin by default. Copy writes to the local macOS clipboard.
--source-url resolves relative links. --unicode opts into decorative styling.
`;

function helper<T>(command: string, input?: unknown): T {
  const executable = process.env.COPIED_MD_PASTEBOARD || 'copied-md-pasteboard';
  const result = spawnSync(executable, [command], {
    encoding: 'utf8',
    input: input === undefined ? undefined : JSON.stringify(input),
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) {
    throw new CliError(
      'clipboard-helper-unavailable',
      'Install copied-md-pasteboard or set COPIED_MD_PASTEBOARD.',
      1,
    );
  }
  if (result.status !== 0) {
    throw new CliError(
      result.status === 3 ? 'clipboard-changed' : 'clipboard-failed',
      result.stderr.trim() || 'The clipboard operation failed.',
      1,
    );
  }
  try {
    return JSON.parse(result.stdout) as T;
  } catch {
    throw new CliError(
      'clipboard-helper-response',
      'The clipboard helper returned an invalid response.',
      1,
    );
  }
}

const wantsJson = process.argv.includes('--json');

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: {
      to: { type: 'string' },
      file: { type: 'string' },
      format: { type: 'string' },
      'source-url': { type: 'string' },
      json: { type: 'boolean' },
      clipboard: { type: 'boolean' },
      pick: { type: 'boolean' },
      unicode: { type: 'boolean' },
      help: { type: 'boolean', short: 'h' },
    },
  });
  const [command = 'convert', ...extra] = positionals;
  if (values.help) {
    process.stdout.write(help);
    return;
  }
  if (extra.length) throw new CliError('usage', 'Use --file PATH to convert a file.');
  if (command === 'mcp') {
    if (Object.keys(values).length)
      throw new CliError('usage', 'The MCP command takes no options.');
    const { serveMcp } = await import('./mcp');
    await serveMcp();
    return;
  }
  if (command === 'targets') {
    process.stdout.write(
      values.json
        ? JSON.stringify(targets) + '\n'
        : targets.map(({ id, label }) => id + '\t' + label).join('\n') + '\n',
    );
    return;
  }
  if (command !== 'convert' && command !== 'copy')
    throw new CliError('usage', 'Unknown command ' + command + '.');
  if (command === 'convert' && (values.clipboard || values.pick))
    throw new CliError('usage', '--clipboard and --pick require the copy command.');
  if (command === 'copy' && process.platform !== 'darwin')
    throw new CliError('clipboard-unavailable', 'System clipboard copy requires macOS.', 1);
  if (values.clipboard && values.file)
    throw new CliError('usage', 'Choose either --file or --clipboard.');
  if (values.pick && values.to) throw new CliError('usage', 'Choose either --to or --pick.');
  if (values.format !== undefined && !['plain', 'html'].includes(values.format))
    throw new CliError('usage', 'Use --format plain or --format html.');

  let selected = values.to ?? 'rich-text';
  if (!values.pick && !resolveTarget(selected))
    throw new CliError('unknown-target', 'Unknown copy target ' + selected + '.');
  let markdown: string;
  let changeCount: number | undefined;
  if (values.clipboard) {
    const snapshot = helper<{ text: string; changeCount: number }>('read');
    markdown = snapshot.text;
    changeCount = snapshot.changeCount;
  } else {
    if (!values.file && process.stdin.isTTY)
      throw new CliError('missing-input', 'Pipe Markdown into stdin or pass --file.');
    try {
      markdown = readFileSync(values.file ?? 0, 'utf8');
    } catch (error) {
      throw new CliError('read-failed', (error as Error).message, 1);
    }
  }

  if (values.pick) {
    const choice = helper<{ target?: string }>('choose', targets);
    if (!choice.target) return;
    selected = choice.target;
  }
  const target = resolveTarget(selected);
  if (!target) throw new CliError('unknown-target', 'Unknown copy target ' + selected + '.');
  const converted = convertMarkdown(markdown, target, {
    sourceUrl: values['source-url'],
    decorativeUnicode: values.unicode,
  });
  if (command === 'copy') {
    if (!markdown.trim() || !converted.representations['text/plain'].trim()) {
      throw new CliError('empty-output', 'There is no text to copy.');
    }
    const copied = helper('write', {
      representations: converted.representations,
      expectedChangeCount: changeCount,
      target,
    });
    if (values.json)
      process.stdout.write(JSON.stringify({ ...converted, clipboard: copied }) + '\n');
    else for (const warning of converted.diagnostics) process.stderr.write(warning.message + '\n');
    return;
  }
  if (values.json) {
    process.stdout.write(JSON.stringify(converted) + '\n');
    return;
  }
  const type = values.format === 'html' ? 'text/html' : 'text/plain';
  const output = converted.representations[type];
  if (output === undefined)
    throw new CliError('representation-unavailable', 'This target does not provide HTML.');
  process.stdout.write(output);
  for (const warning of converted.diagnostics) process.stderr.write(warning.message + '\n');
}

main().catch((error: unknown) => {
  const problem =
    error instanceof CliError
      ? error
      : new CliError(
          'command-failed',
          error instanceof Error ? error.message : 'The command failed.',
          1,
        );
  process.stderr.write(
    wantsJson
      ? JSON.stringify({ error: { code: problem.code, message: problem.message } }) + '\n'
      : problem.message + '\n',
  );
  process.exitCode = problem.exitCode;
});
