# The Thread Workshop

One original pixel-art family for the twelve tools in the Quipu stack. Each tool gets its own object, drawn from simple geometric regions; the frame, palette, typography and composition stay shared. These are hero illustrations, not replacement product logos or screenshots.

## Canvas and placement

The editable canvas is 352 × 176 pixels (2:1). The standard PNG is 704 × 352, rendered at integer scale 2. The @2x PNG is 1408 × 704, rendered at scale 4. Display either at 704 CSS pixels or less, preserving the aspect ratio. Never use smoothing to make another export.

A warm paper rim sits at x8/y8, around a fixed ink panel. The left field carries the tool name and a short purpose label. The right field holds an original object, roughly x230–326/y43–124. A low bench, sparse stitches and a hanging divider make the set feel like one workshop. Keep the rim and lettering crisp. Cards carry no series number or stack-size denominator; each illustration stands alone.

## Palette

| Token | Hex | Use |
|---|---|---|
| ink | #172B3B | stable interior backdrop |
| depth | #10212F | bench and recessed details |
| rim / paper | #F2E6CF / #EEE2C7 | framing, primary lettering, object highlights |
| muted | #95AFB4 | series labels |
| grid | #294554 | quiet guides and stitches |
| teal / teal-dark | #68D4BF / #347C79 | active threads, leaves and connections |
| gold / wood | #EDB96E / #AF7952 | knots, timber and warm accents |
| coral | #E78E7D | reserved secondary accent for future tools |

There are no gradients, photographs, traced logos, imported artwork or downloaded fonts. Original 5×7 uppercase lettering is defined in lettering.json and authored as geometric regions in each source. Tool titles use scale 2; supporting labels use scale 1. The small labels support the title rather than carrying essential instructions. Use sentence-case accessible alt text alongside an image.

## Light and dark backgrounds

The production hero has a transparent margin around an opaque panel. The warm rim remains visible against the #0D1117 dark preview and the #FFFFFF light preview; the typography always sits on the same ink backdrop. The previews are separate sprites in each source, rendered by pixelsrc, not postprocessed PNGs. This avoids theme-specific versions drifting in their interior artwork. They are presentation proofs; use renders/tool.png for embedding, not the preview.

## Motifs

| Tool | Object | Purpose label |
|---|---|---|
| quipu | knotted hanging cords | Knowledge / history |
| seeds | sprout rising from an ordered work card | Work / momentum |
| shantytown | four workshop houses and a shared courtyard | Crews / coordination |
| bobbin | thread spool with a loose working end | Code / context |
| yupana | counting board with distinct counters | Impact / policy |
| camayoc | loose strands passing through a checked gate | Ingress / checks |
| quechua | open book carrying matching terms | Words / shapes |
| chaski | winged envelope and delivery arrow | Events / delivery |
| shuttle | wooden shuttle passing through warp threads | Signed / workflows |
| rikuy | an eye-shaped instrument and status indicators | Status / decisions |
| desire-path | stepping stones crossing an orderly garden grid | Friction / discovery |
| tambo | toolkit and connecting component tiles | Install / configure |

## Reproduce

Run `PXL=/path/to/pxl ./render.sh` from any directory. The script strictly validates each source, then produces the standard and @2x transparent heroes and both theme previews. All geometry, letterforms and colours are inside the .pxl files. No network or input image is needed.

The delivered renderer was built from pixelsrc commit 48023226cacf81ce5ba5e493cc98d94fea5d7e23; its SHA256 is recorded in verification.json. Pin that source revision for exact reproduction. The bundle does not contain or install the renderer. SHA256SUMS covers every delivered file except itself. Verification includes exact nearest-neighbour pixel replication and equality of the panel between both theme variants.

## Extension and integration

A new tool gets a new object and purpose caption, keeping the frame, canvas, shared palette, title size and motif field. Do not add a series number or stack-size denominator: adding another tool must not change any existing card. Source regions use named semantic tokens and unique explicit z layers (no tie-order dependency); prefer unions of simple shapes over complex polygons.

Use each tool’s own card at the top of its README. Pair it with an embedded, reproducible terminal demonstration near the first-success section. Keep explanatory mechanism diagrams where they help a reader follow the tool’s behavior. The illustrations contain no claims that a system is installed, healthy or running.
