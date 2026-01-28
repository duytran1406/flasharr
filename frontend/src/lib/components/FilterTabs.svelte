<script lang="ts">
  interface Tab {
    id: string;
    label: string;
    count?: number;
  }

  interface Props {
    tabs: Tab[];
    activeTab: string;
    onTabChange?: (tabId: string) => void;
  }

  let { tabs, activeTab, onTabChange }: Props = $props();
</script>

<div class="flex gap-2 p-1 bg-slate-800/50 rounded-xl">
  {#each tabs as tab}
    <button
      onclick={() => onTabChange?.(tab.id)}
      class="px-4 py-2 rounded-lg font-medium text-sm transition-all flex items-center gap-2"
      class:bg-blue-500={activeTab === tab.id}
      class:text-white={activeTab === tab.id}
      class:shadow-lg={activeTab === tab.id}
      class:text-slate-400={activeTab !== tab.id}
      class:hover:text-white={activeTab !== tab.id}
      class:hover:bg-slate-700={activeTab !== tab.id}
    >
      {tab.label}
      {#if tab.count !== undefined && tab.count > 0}
        <span
          class="px-1.5 py-0.5 rounded-full text-xs {activeTab === tab.id
            ? 'bg-white/20'
            : 'bg-slate-600'}"
        >
          {tab.count}
        </span>
      {/if}
    </button>
  {/each}
</div>
