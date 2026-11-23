<script setup lang="ts">
import { targets } from '@0xdoublesharp/copied-md-converters';
import { useUiStore } from '@/stores/ui.store';
import { Button } from '@/components/ui/button';

const ui = useUiStore();
</script>

<template>
  <div class="grid grid-cols-3 gap-2">
    <Button
      v-for="target in targets"
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
