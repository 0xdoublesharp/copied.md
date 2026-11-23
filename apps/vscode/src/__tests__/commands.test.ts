import { describe, it, expect, vi, beforeEach } from 'vitest';
import { registerCommands } from '../commands';

const mocks = vi.hoisted(() => ({
  editor: undefined as unknown,
  register: vi.fn(),
  write: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
  picker: vi.fn(),
}));

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
  env: { remoteName: 'ssh-remote', clipboard: { writeText: mocks.write } },
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
  registerCommands(context as never);
  return mocks.register.mock.calls.find(([key]) => key === id)?.[1];
}

describe('VS Code copy commands', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.editor = undefined;
  });

  it('retains named commands and registers the picker', () => {
    const context = { subscriptions: [] };
    registerCommands(context as never);
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
    mocks.editor = editor([selection(0, '**bold** and [link](https://example.com)')]);
    await handler('copied-md.copyTo.slack')();
    expect(mocks.write).toHaveBeenCalledWith('bold and link (https://example.com)');
    expect(mocks.info).toHaveBeenCalledWith(expect.stringContaining('Copied as plain text'));
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
