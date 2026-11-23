import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { readFile, spawn, serve, stdin } = vi.hoisted(() => ({
  readFile: vi.fn(),
  spawn: vi.fn(),
  serve: vi.fn(),
  stdin: vi.fn(),
}));

vi.mock('node:fs', async (importOriginal) => ({
  ...(await importOriginal<typeof import('node:fs')>()),
  readFileSync: readFile,
}));
vi.mock('node:child_process', async (importOriginal) => ({
  ...(await importOriginal<typeof import('node:child_process')>()),
  spawnSync: spawn,
}));
vi.mock('../mcp', () => ({ serveMcp: serve }));

async function run(args: string[], platform: NodeJS.Platform = 'darwin', tty = false) {
  vi.resetModules();
  const realArgv = process.argv;
  const realExit = process.exitCode;
  const platformDescriptor = Object.getOwnPropertyDescriptor(process, 'platform')!;
  const stdinDescriptor = Object.getOwnPropertyDescriptor(process, 'stdin')!;
  process.argv = ['node', 'copied-md', ...args];
  process.exitCode = undefined;
  Object.defineProperty(process, 'platform', { ...platformDescriptor, value: platform });
  Object.defineProperty(process, 'stdin', {
    configurable: true,
    value: { isTTY: tty, [Symbol.asyncIterator]: stdin },
  });
  const stdout = vi.spyOn(process.stdout, 'write').mockReturnValue(true);
  const stderr = vi.spyOn(process.stderr, 'write').mockReturnValue(true);
  try {
    await import('../cli');
    await new Promise<void>((resolve) => setImmediate(resolve));
    return {
      stdout: stdout.mock.calls.map(([chunk]) => chunk).join(''),
      stderr: stderr.mock.calls.map(([chunk]) => chunk).join(''),
      exit: process.exitCode,
    };
  } finally {
    stdout.mockRestore();
    stderr.mockRestore();
    process.argv = realArgv;
    process.exitCode = realExit;
    Object.defineProperty(process, 'platform', platformDescriptor);
    Object.defineProperty(process, 'stdin', stdinDescriptor);
  }
}

function helper(output: unknown, status = 0, stderr = '') {
  return { stdout: JSON.stringify(output), stderr, status };
}

describe('copied-md command line', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    stdin.mockImplementation(async function* () {
      yield Buffer.from(readFile());
    });
    readFile.mockReturnValue('**Bold**');
    vi.stubEnv('COPIED_MD_PASTEBOARD', '');
  });

  afterEach(() => vi.unstubAllEnvs());

  it('prints help and both forms of the target directory', async () => {
    expect((await run(['-h'])).stdout).toContain('copied-md mcp');
    expect((await run(['targets'])).stdout).toContain('telegram\tTelegram');
    const { stdout } = await run(['targets', '--json']);
    expect(JSON.parse(stdout)).toContainEqual({ id: 'telegram', label: 'Telegram', rich: true });
  });

  it('converts stdin by default and emits diagnostics on stderr', async () => {
    readFile.mockReturnValue('[Notes](./notes)');
    expect(await run([])).toMatchObject({
      stdout: 'Notes',
      stderr: expect.stringContaining('original page URL'),
    });
    expect(stdin).toHaveBeenCalledOnce();
  });

  it('decodes Unicode intact when UTF-8 code points span pipe reads', async () => {
    const encoded = Buffer.from('**😀**');
    stdin.mockImplementation(async function* () {
      yield encoded.subarray(0, 4);
      await Promise.resolve();
      yield encoded.subarray(4);
    });
    expect((await run(['convert', '--to', 'plain-text'])).stdout).toBe('😀');
  });

  it('accepts stdin streams already decoding UTF-8', async () => {
    stdin.mockImplementation(async function* () {
      yield '**Bold**';
    });
    expect((await run(['convert', '--to', 'plain-text'])).stdout).toBe('Bold');
  });

  it('converts a file with HTML and resolves against the source URL', async () => {
    readFile.mockReturnValue('[Notes](./notes)');
    const result = await run([
      'convert',
      '--file',
      'document.md',
      '--source-url',
      'https://example.org/posts/a',
      '--format',
      'html',
    ]);
    expect(result.stdout).toContain('href="https://example.org/posts/notes"');
    expect(readFile).toHaveBeenCalledWith('document.md', 'utf8');
  });

  it('returns the schema and original source length in JSON', async () => {
    const { stdout } = await run(['convert', '--to', 'slack', '--json']);
    expect(JSON.parse(stdout)).toMatchObject({
      target: 'slack-paste',
      metrics: { sourceCodeUnits: 8 },
      representations: { 'text/plain': 'Bold', 'text/html': '<p><strong>Bold</strong></p>' },
    });
  });

  it.each([
    [['convert', 'document.md'], 'usage'],
    [['unknown'], 'usage'],
    [['mcp', '--to', 'sms'], 'usage'],
    [['convert', '--clipboard'], 'usage'],
    [['convert', '--pick'], 'usage'],
    [['copy', '--file', 'document.md', '--clipboard'], 'usage'],
    [['copy', '--to', 'sms', '--pick'], 'usage'],
    [['convert', '--format', 'xml'], 'usage'],
    [['convert', '--to', 'missing'], 'unknown-target'],
  ])('rejects invalid arguments %j', async (args, code) => {
    const result = await run([...args, '--json']);
    expect(result.exit).toBe(2);
    expect(JSON.parse(result.stderr).error.code).toBe(code);
    expect(result.stdout).toBe('');
  });

  it('rejects an HTML-only request for a plain text target', async () => {
    expect(await run(['convert', '--to', 'plain-text', '--format', 'html'])).toMatchObject({
      exit: 2,
      stderr: 'This target does not provide HTML.\n',
      stdout: '',
    });
  });

  it('requires stdin or a file in an interactive terminal', async () => {
    const result = await run(['convert'], 'darwin', true);
    expect(result.stderr).toContain('Pipe Markdown');
    expect(result.exit).toBe(2);
  });

  it('reports invalid flags and unreadable files', async () => {
    expect(JSON.parse((await run(['--unknown', '--json'])).stderr).error.code).toBe(
      'command-failed',
    );
    readFile.mockImplementation(() => {
      throw new Error('file unavailable');
    });
    expect(await run(['convert', '--file', 'missing'])).toMatchObject({
      stderr: 'file unavailable\n',
      exit: 1,
    });
  });

  it('connects to the MCP entry point only with no options', async () => {
    expect(await run(['mcp'])).toMatchObject({ stdout: '', stderr: '' });
    await vi.waitFor(() => expect(serve).toHaveBeenCalledOnce());
  });

  it('reports a general command error for a non-Error server rejection', async () => {
    serve.mockRejectedValueOnce('unexpected rejection');
    expect(await run(['mcp'])).toMatchObject({ exit: 1, stderr: 'The command failed.\n' });
  });

  it('does not offer system clipboard copy on Linux', async () => {
    const result = await run(['copy', '--json'], 'linux');
    expect(result.exit).toBe(1);
    expect(JSON.parse(result.stderr).error.code).toBe('clipboard-unavailable');
    expect(spawn).not.toHaveBeenCalled();
  });

  it('copies the chosen target and guards the original clipboard change count', async () => {
    vi.stubEnv('COPIED_MD_PASTEBOARD', '/custom/helper');
    spawn
      .mockReturnValueOnce(helper({ text: '**Source**', changeCount: 12 }))
      .mockReturnValueOnce(helper({ target: 'telegram' }))
      .mockReturnValueOnce(helper({ types: ['text/plain', 'text/html', 'text/rtf'] }));
    const { stdout } = await run(['copy', '--clipboard', '--pick', '--json']);
    expect(JSON.parse(stdout)).toMatchObject({
      target: 'telegram',
      clipboard: { types: ['text/plain', 'text/html', 'text/rtf'] },
    });
    expect(spawn.mock.calls.map(([command, args]) => [command, args[0]])).toEqual([
      ['/custom/helper', 'read'],
      ['/custom/helper', 'choose'],
      ['/custom/helper', 'write'],
    ]);
    expect(JSON.parse(spawn.mock.calls[2][2].input)).toMatchObject({
      expectedChangeCount: 12,
      target: 'telegram',
      representations: { 'text/plain': 'Source' },
    });
  });

  it('does not write after a cancelled picker or invalid choice', async () => {
    spawn.mockReturnValueOnce(helper({}));
    expect(await run(['copy', '--pick'])).toMatchObject({ stdout: '', stderr: '' });
    spawn.mockReturnValueOnce(helper({ target: 'invalid' }));
    expect((await run(['copy', '--pick'])).stderr).toContain('Unknown copy target invalid');
    expect(spawn.mock.calls.map(([, args]) => args[0])).toEqual(['choose', 'choose']);
  });

  it('rejects empty input and rendered whitespace before touching the clipboard', async () => {
    for (const markdown of ['  ', '[x]: https://example.org']) {
      readFile.mockReturnValue(markdown);
      expect((await run(['copy'])).stderr).toContain('There is no text to copy');
    }
    expect(spawn).not.toHaveBeenCalled();
  });

  it('writes warnings without corrupting stdout in plain copy mode', async () => {
    readFile.mockReturnValue('[Link](./relative)');
    spawn.mockReturnValueOnce(helper({ types: ['text/plain'] }));
    expect(await run(['copy', '--to', 'plain-text'])).toMatchObject({
      stdout: '',
      stderr: expect.stringContaining('original page URL'),
    });
  });

  it.each([
    [{ error: new Error('ENOENT') }, 'clipboard-helper-unavailable', 'Install copied-md'],
    [helper({}, 3, 'stale'), 'clipboard-changed', 'stale'],
    [helper({}, 1), 'clipboard-failed', 'The clipboard operation failed.'],
    [{ status: 0, stdout: 'broken' }, 'clipboard-helper-response', 'invalid response'],
  ])('handles clipboard helper failure %#', async (response, code, message) => {
    spawn.mockReturnValue(response);
    const result = await run(['copy', '--json']);
    expect(result.exit).toBe(1);
    expect(JSON.parse(result.stderr).error).toEqual({
      code,
      message: expect.stringContaining(message),
    });
  });
});
