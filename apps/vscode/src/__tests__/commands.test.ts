import { afterEach, describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'node:events';
import { activate, deactivate } from '../extension';

const mocks = vi.hoisted(() => ({
  editor: undefined as unknown,
  register: vi.fn(),
  write: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
  picker: vi.fn(),
  remote: undefined as string | undefined,
  uiKind: 1,
  setting: vi.fn(),
  spawn: vi.fn(),
}));

vi.mock('node:child_process', () => ({ spawn: mocks.spawn }));

vi.mock('vscode', () => ({
  commands: {
    registerCommand: (id: string, handler: () => void) => {
      mocks.register(id, handler);
      return { dispose: vi.fn() };
    },
  },
  window: {
    get activeTextEditor() {
      return mocks.editor;
    },
    showInformationMessage: mocks.info,
    showWarningMessage: mocks.warning,
    showErrorMessage: mocks.error,
    showQuickPick: mocks.picker,
  },
  UIKind: { Desktop: 1, Web: 2 },
  workspace: { getConfiguration: () => ({ get: mocks.setting }) },
  env: {
    get remoteName() {
      return mocks.remote;
    },
    get uiKind() {
      return mocks.uiKind;
    },
    clipboard: { writeText: mocks.write },
  },
}));

function selection(offset: number, text: string) {
  return {
    offset,
    text,
    isEmpty: !text,
    start: { compareTo: (other: { offset: number }) => offset - other.offset, offset },
  };
}

function editor(
  selections: ReturnType<typeof selection>[],
  languageId = 'markdown',
  document = '',
) {
  return {
    selections,
    document: {
      languageId,
      getText: (range?: ReturnType<typeof selection>) => range?.text ?? document,
    },
  };
}

function handler(id: string): () => Promise<void> {
  const context = { subscriptions: [] };
  activate(context as never);
  return mocks.register.mock.calls.find(([key]) => key === id)?.[1];
}

describe('VS Code copy commands', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.editor = undefined;
    mocks.remote = undefined;
    mocks.uiKind = 1;
    mocks.setting.mockReturnValue('/path/clipboard');
  });
  afterEach(() => vi.unstubAllGlobals());

  it('retains named commands and registers the picker', () => {
    const context = { subscriptions: [] };
    activate(context as never);
    deactivate();
    expect(context.subscriptions).toHaveLength(9);
    expect(mocks.register.mock.calls.map(([id]) => id)).toEqual(
      expect.arrayContaining([
        'copied-md.copyAs',
        'copied-md.copyTo.slack',
        'copied-md.copyTo.html',
      ]),
    );
  });

  it('shows a warning when no editor is active', async () => {
    await handler('copied-md.copyTo.slack')();
    expect(mocks.warning).toHaveBeenCalledWith('No active editor');
    expect(mocks.write).not.toHaveBeenCalled();
  });

  it('reports a plain-text Slack fallback in remote workspaces', async () => {
    vi.stubGlobal('process', Object.create(process, { platform: { value: 'darwin' } }));
    mocks.remote = 'ssh-remote';
    mocks.editor = editor([selection(0, '**bold** and [link](https://example.com)')]);
    await handler('copied-md.copyTo.slack')();
    expect(mocks.write).toHaveBeenCalledWith('bold and link (https://example.com)');
    expect(mocks.info).toHaveBeenCalledWith(expect.stringContaining('Copied as plain text'));
  });

  it('does not copy a non-Markdown document without a selection', async () => {
    mocks.editor = editor([], 'typescript', 'const a = 1');
    await handler('copied-md.copyAs')();
    expect(mocks.warning).toHaveBeenCalledWith('Select Markdown to copy.');
    expect(mocks.picker).not.toHaveBeenCalled();
  });

  it('does not copy when the picker is dismissed', async () => {
    mocks.editor = editor([], 'markdown', '# Notes');
    await handler('copied-md.copyAs')();
    expect(mocks.picker).toHaveBeenCalled();
    expect(mocks.write).not.toHaveBeenCalled();
  });

  it.each([' ', '[Unused]: https://example.org'])('rejects empty copy output %j', async (text) => {
    mocks.editor = editor([], 'markdown', text);
    await handler('copied-md.copyTo.telegram')();
    expect(mocks.warning).toHaveBeenCalledWith('There is no text to copy.');
    expect(mocks.spawn).not.toHaveBeenCalled();
    expect(mocks.write).not.toHaveBeenCalled();
  });

  it.each([new Error('denied'), 'denied'])('reports a clipboard exception %j', async (error) => {
    mocks.editor = editor([], 'markdown', 'Text');
    mocks.write.mockRejectedValue(error);
    await handler('copied-md.copyTo.sms')();
    expect(mocks.error).toHaveBeenCalledWith('Copy failed: denied');
  });

  it.each([
    ['close', 0, true],
    ['close', 1, false],
    ['error', new Error('missing executable'), false],
    ['stdin-error', new Error('pipe closed'), false],
  ] as const)('handles helper %s (%s)', async (event, value, rich) => {
    vi.stubGlobal('process', Object.create(process, { platform: { value: 'darwin' } }));
    try {
      const child = Object.assign(new EventEmitter(), { stdin: new EventEmitter() });
      const end = vi.fn((_data: string) => {
        if (event === 'stdin-error') child.stdin.emit('error', value);
        else child.emit(event, value);
      });
      mocks.spawn.mockReturnValue(
        Object.assign(child, { stdin: Object.assign(child.stdin, { end }) }),
      );
      mocks.editor = editor([], 'markdown', '**Bold**');
      await handler('copied-md.copyTo.telegram')();
      expect(mocks.spawn).toHaveBeenCalledWith('/path/clipboard', ['write'], {
        stdio: ['pipe', 'ignore', 'ignore'],
        timeout: 5000,
      });
      expect(JSON.parse(end.mock.calls[0][0])).toMatchObject({
        representations: {
          'text/plain': 'Bold',
          'text/html': '<strong>Bold</strong>',
          'text/rtf': expect.any(String),
        },
      });
      expect(mocks.write).toHaveBeenCalledTimes(rich ? 0 : 1);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it.each([
    ['linux', 1, '/path/clipboard'],
    ['darwin', 2, '/path/clipboard'],
    ['darwin', 1, 'relative'],
    ['darwin', 1, ''],
  ] as const)('falls back for an unavailable rich clipboard (%s, %d, %j)', async (os, ui, path) => {
    vi.stubGlobal('process', Object.create(process, { platform: { value: os } }));
    try {
      mocks.uiKind = ui;
      mocks.setting.mockReturnValue(path);
      mocks.editor = editor([], 'markdown', '**Bold**');
      await handler('copied-md.copyTo.telegram')();
      expect(mocks.spawn).not.toHaveBeenCalled();
      expect(mocks.write).toHaveBeenCalledWith('Bold');
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('copies selections in document order', async () => {
    mocks.editor = editor([selection(9, 'Second'), selection(0, 'First')]);
    mocks.picker.mockResolvedValue({ target: 'plain-text' });
    await handler('copied-md.copyAs')();
    expect(mocks.write).toHaveBeenCalledWith('First\n\nSecond');
  });

  it('offers the entire Markdown document when the selection is empty', async () => {
    mocks.editor = editor([selection(0, '')], 'markdown', '# Title');
    mocks.picker.mockResolvedValue({ target: 'html-source' });
    await handler('copied-md.copyAs')();
    expect(mocks.write).toHaveBeenCalledWith('<h1>Title</h1>');
  });

  it('warns about X limits without truncating', async () => {
    const text = '漢'.repeat(150);
    mocks.editor = editor([selection(0, text)]);
    await handler('copied-md.copyTo.twitter')();
    expect(mocks.write).toHaveBeenCalledWith(text);
    expect(mocks.warning).toHaveBeenCalledWith(expect.stringContaining('280 weighted characters'));
  });
});
