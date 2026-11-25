<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { targets } from '@0xdoublesharp/copied-md-converters';
import { browser } from 'wxt/browser';
import { useSettingsStore } from '@/stores/settings.store';
import { CHAT_SITES, chatOrigin, type ChatSite } from '@/lib/constants';

const settings = useSettingsStore();
const chatAccess = ref<ChatSite['id'][]>([]);
const updatingChatAccess = ref(true);
const chatAccessError = ref('');

onMounted(async () => {
  try {
    for (const site of CHAT_SITES) {
      if (await browser.permissions.contains({ origins: [chatOrigin(site)] }))
        chatAccess.value.push(site.id);
    }
  } catch {
    chatAccessError.value = 'Could not check access to chat sites.';
  } finally {
    updatingChatAccess.value = false;
  }
});

async function changeChatAccess(site: ChatSite, input: { checked: boolean }) {
  updatingChatAccess.value = true;
  chatAccessError.value = '';
  const checked = input.checked;
  const permission = { origins: [chatOrigin(site)] };
  try {
    await (checked
      ? browser.permissions.request(permission)
      : browser.permissions.remove(permission));
    const allowed = await browser.permissions.contains(permission);
    chatAccess.value = [
      ...chatAccess.value.filter((id) => id !== site.id),
      ...(allowed ? [site.id] : []),
    ];
  } catch {
    chatAccessError.value = 'Could not change access to ' + site.label + '.';
  } finally {
    input.checked = chatAccess.value.includes(site.id);
    updatingChatAccess.value = false;
  }
}

// Auto-save on changes
watch(
  () => ({
    defaultPlatform: settings.defaultPlatform,
    showNotifications: settings.showNotifications,
    autoClose: settings.autoClose,
    formatVisibility: settings.formatVisibility,
  }),
  () => {
    settings.save();
  },
  { deep: true },
);
</script>

<template>
  <div class="space-y-4 py-2">
    <div class="space-y-2">
      <label class="text-sm font-medium">Default format</label>
      <select
        v-model="settings.defaultPlatform"
        class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
      >
        <option v-for="p in settings.formats" :key="p.id" :value="p.id">
          {{ p.label }}
        </option>
      </select>
    </div>

    <fieldset class="space-y-2">
      <legend class="mb-2 text-sm font-medium">Show formats</legend>
      <div class="grid grid-cols-2 gap-2">
        <label v-for="target in targets" :key="target.id" class="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            class="rounded"
            :checked="settings.formats.some(({ id }) => id === target.id)"
            :disabled="settings.formats.length === 1 && settings.formats[0].id === target.id"
            @change="
              settings.setFormatVisible(target.id, ($event.target as HTMLInputElement).checked)
            "
          />
          {{ target.label }}
        </label>
      </div>
    </fieldset>

    <label class="flex items-center justify-between text-sm">
      Show notifications
      <input v-model="settings.showNotifications" type="checkbox" class="rounded" />
    </label>

    <fieldset class="space-y-2" :disabled="updatingChatAccess">
      <legend class="mb-2 text-sm font-medium">Copy as… in AI chats</legend>
      <label v-for="site in CHAT_SITES" :key="site.id" class="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          :checked="chatAccess.includes(site.id)"
          class="rounded"
          @change="changeChatAccess(site, $event.target as HTMLInputElement)"
        />
        {{ site.label }}
      </label>
      <p v-if="chatAccessError" role="alert" class="text-sm">{{ chatAccessError }}</p>
    </fieldset>

    <label class="flex items-center justify-between text-sm">
      Auto-close after copy
      <input v-model="settings.autoClose" type="checkbox" class="rounded" />
    </label>
  </div>
</template>
