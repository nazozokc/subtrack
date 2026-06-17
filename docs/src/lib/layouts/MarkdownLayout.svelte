<script lang="ts">
  let {
    children,
    title = "subtrack",
    description = "",
  }: {
    children: import("svelte").Snippet
    title?: string
    description?: string
  } = $props()
</script>

<svelte:head>
  <title>{title === "subtrack" ? "subtrack" : `${title} — subtrack`}</title>
  {#if description}
    <meta name="description" content={description} />
  {/if}
</svelte:head>

<div class="docs">
  {#if title !== "subtrack"}
    <h1>{title}</h1>
  {/if}
  {@render children()}
</div>
