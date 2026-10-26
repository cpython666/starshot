<script setup lang="ts">
withDefaults(defineProps<{name:string;size?:number}>(),{size:20})
const paths:Record<string,string>={
 capture:'M8 3H5a2 2 0 0 0-2 2v3m13-5h3a2 2 0 0 1 2 2v3M3 16v3a2 2 0 0 0 2 2h3m13-5v3a2 2 0 0 1-2 2h-3M8 8h8v8H8z',
 image:'M4 4h16v16H4zM4 16l5-5 4 4 3-3 4 4M15 8h.01',
 import:'M12 15V3m-4 4 4-4 4 4M4 14v6h16v-6',
 save:'M12 3v12m-4-4 4 4 4-4M4 16v5h16v-5',
 pin:'m9 3 8 0-1 6 3 3v2h-6v7l-2-3v-4H5v-2l3-3z',
 copy:'M8 8h12v13H8zM16 8V3H3v13h5',
 undo:'M9 5 4 10l5 5M4 10h10a6 6 0 0 1 6 6',
 redo:'m15 5 5 5-5 5m5-5H10a6 6 0 0 0-6 6',
 arrow:'M4 20 20 4m-9 0h9v9', rect:'M4 5h16v14H4z',
 ellipse:'M21 12a9 7 0 1 1-18 0 9 7 0 1 1 18 0',
 pen:'m4 20 4-1L20 7l-3-3L5 16zM14 7l3 3',
 highlighter:'m4 15 9-11 7 6-10 10zM4 15l6 5M3 21h7',
 text:'M4 5h16M12 5v16m-4 0h8',
 number:'M9 3 7 21M17 3l-2 18M4 9h17M3 15h17',
 mosaic:'M3 3h6v6H3zm12 0h6v6h-6zM9 9h6v6H9zm-6 6h6v6H3zm12 0h6v6h-6z',
 crop:'M6 3v15h15M3 6h15v15', cursor:'m5 3 14 9-7 1-3 7z',
 settings:'M12 8a4 4 0 1 0 0 8 4 4 0 1 0 0-8M12 2v3m0 14v3M2 12h3m14 0h3M5 5l2 2m10 10 2 2M5 19l2-2M17 7l2-2',
 history:'M3 11a9 9 0 1 1 2 7M3 4v7h7m2-4v6l4 2',
 close:'m6 6 12 12M6 18 18 6', check:'m5 12 4 4L19 6',
 spark:'m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5z',
 sun:'M12 8a4 4 0 1 0 0 8 4 4 0 1 0 0-8M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1 1m12 12 1 1M5 19l1-1M18 6l1-1',
 moon:'M20 15a9 9 0 0 1-11-11A9 9 0 1 0 20 15',
 help:'M12 17h.01M9 9a3 3 0 1 1 4 3c-1 .5-1 1-1 2M21 12a9 9 0 1 1-18 0 9 9 0 1 1 18 0',
 trash:'M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7',
 chevron:'m9 5 7 7-7 7', plus:'M12 5v14M5 12h14', folder:'M3 6h7l2 2h9v12H3z'
}
</script>
<template><svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path :d="paths[name] || paths.image"/></svg></template>
