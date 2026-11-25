<script setup lang="ts">
import { useSettingsStore } from '@/stores/settings.store';
import { useUiStore } from '@/stores/ui.store';
import { Button } from '@/components/ui/button';

const ui = useUiStore();
const settings = useSettingsStore();
</script>

<template>
  <div class="grid grid-cols-3 gap-2">
    <Button
      v-for="target in settings.formats"
      :key="target.id"
      :variant="ui.selectedPlatform === target.id ? 'default' : 'outline'"
      :aria-pressed="ui.selectedPlatform === target.id"
      size="sm"
      class="h-auto min-h-9 whitespace-normal px-2 text-xs"
      @click="ui.selectPlatform(target.id)"
    >
      {{ target.label }}
    </Button>
  </div>
</template>
