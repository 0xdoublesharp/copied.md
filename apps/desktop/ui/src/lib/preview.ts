import { shallowRef, watch } from 'vue';
import type { CopyTarget, DesktopApi, MarkdownConversion } from './ipc';

export function useMarkdownPreview(
  api: DesktopApi,
  source: () => string | undefined,
  target: () => CopyTarget | undefined,
  operation?: () => number,
) {
  const result = shallowRef<MarkdownConversion>();
  const failed = shallowRef(false);
  let generation = 0;

  watch(
    [source, target, () => operation?.()],
    ([markdown, target], _, onCleanup) => {
      const request = ++generation;
      result.value = undefined;
      failed.value = false;
      if (markdown == null || !target) return;

      const timer = window.setTimeout(async () => {
        try {
          const conversion = await api.preview(markdown, target, null, false);
          if (request === generation) result.value = conversion;
        } catch {
          if (request === generation) failed.value = true;
        }
      }, 50);
      onCleanup(() => {
        ++generation;
        window.clearTimeout(timer);
      });
    },
    { immediate: true },
  );

  return { result, failed };
}
