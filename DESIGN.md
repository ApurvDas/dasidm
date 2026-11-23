---
name: DasIDM
description: A personal download manager that reads like a departures board on smoked glass.
colors:
  alert: "#ffd400"
  alert-ink: "#1a1600"
  alert-hover: "#ffe14d"
  alert-on-paper: "#c9a300"
  ink: "#0b0d10"
  steel: "#6b7178"
  fog: "#9aa1a9"
  ivory: "#f2f2ee"
  glass-board: "rgb(11 13 16 / 0.6)"
  glass-hall: "rgb(16 19 23 / 0.64)"
  glass-pass: "rgb(13 16 19 / 0.56)"
  line: "rgb(255 255 255 / 0.08)"
  line-strong: "rgb(255 255 255 / 0.14)"
  hover: "rgb(255 255 255 / 0.045)"
  selected: "rgb(242 242 238 / 0.08)"
  sheet: "#14171c"
  map-hole: "#15191e"
  pass-band: "#e4e4dd"
  paper-ink: "#15181c"
  paper-label: "#565d65"
  tear-line: "#a7aca4"
  paper-hole: "#d6d6cf"
  ok: "#4cc38a"
  bad: "#ff6b6b"
  bad-paper-bg: "#ffe3e3"
  bad-paper-ink: "#8a1c1c"
  close-red: "#c42b1c"
typography:
  display:
    fontFamily: "Barlow Condensed, Segoe UI Variable Display, sans-serif"
    fontSize: "34px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "0.03em"
  numeral-xl:
    fontFamily: "Barlow Condensed, Segoe UI Variable Display, sans-serif"
    fontSize: "46px"
    fontWeight: 600
    lineHeight: 0.9
    letterSpacing: "0em"
  route-code:
    fontFamily: "Barlow Condensed, Segoe UI Variable Display, sans-serif"
    fontSize: "24px"
    fontWeight: 700
    lineHeight: 1.05
    letterSpacing: "0.02em"
  numeral-row:
    fontFamily: "Barlow Condensed, Segoe UI Variable Display, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    letterSpacing: "0.02em"
  board-row:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "16px"
    fontWeight: 500
    letterSpacing: "0.01em"
    fontVariation: "'wdth' 75"
    fontFeature: "'tnum' 1"
  pass-title:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "17px"
    fontWeight: 700
    lineHeight: 1.25
    fontVariation: "'wdth' 75"
  pass-value:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "17px"
    fontWeight: 600
    fontVariation: "'wdth' 75"
    fontFeature: "'tnum' 1"
  parts-row:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "14px"
    fontWeight: 400
    fontVariation: "'wdth' 75"
    fontFeature: "'tnum' 1"
  wordmark:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "15px"
    fontWeight: 700
    letterSpacing: "0.08em"
    fontVariation: "'wdth' 75"
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.4
    fontFeature: "'tnum' 1"
  body-strong:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: 1.4
  caption:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.4
  label:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "11px"
    fontWeight: 500
    letterSpacing: "0.08em"
    fontVariation: "'wdth' 75"
  label-paper:
    fontFamily: "Martian Mono, ui-monospace, monospace"
    fontSize: "10px"
    fontWeight: 500
    letterSpacing: "0.08em"
    fontVariation: "'wdth' 75"
rounded:
  hair: "2px"
  badge: "3px"
  map: "4px"
  control: "6px"
  panel: "8px"
  pass: "12px"
spacing:
  tight: "6px"
  gutter: "10px"
  panel: "14px"
  panel-x: "16px"
  column: "16px"
  figure: "28px"
components:
  button-primary:
    backgroundColor: "{colors.alert}"
    textColor: "{colors.alert-ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "30px"
  button-primary-hover:
    backgroundColor: "{colors.alert-hover}"
  button-default:
    backgroundColor: "rgb(255 255 255 / 0.04)"
    textColor: "{colors.ivory}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "30px"
  button-default-hover:
    backgroundColor: "rgb(255 255 255 / 0.09)"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ivory}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "30px"
  button-ghost-hover:
    backgroundColor: "{colors.hover}"
  button-danger:
    textColor: "{colors.bad}"
    rounded: "{rounded.control}"
    height: "30px"
  field:
    backgroundColor: "rgb(0 0 0 / 0.35)"
    textColor: "{colors.ivory}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "32px"
  filter-tab:
    textColor: "{colors.fog}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "30px"
  filter-tab-active:
    backgroundColor: "rgb(255 255 255 / 0.09)"
    textColor: "{colors.ivory}"
  panel-board:
    backgroundColor: "{colors.glass-board}"
    rounded: "{rounded.panel}"
    padding: "14px 16px 10px"
  panel-hall:
    backgroundColor: "{colors.glass-hall}"
    rounded: "{rounded.panel}"
  panel-pass:
    backgroundColor: "{colors.glass-pass}"
    rounded: "{rounded.panel}"
    padding: "14px"
  board-row:
    textColor: "{colors.ivory}"
    typography: "{typography.board-row}"
    padding: "0 10px"
    height: "46px"
  board-row-selected:
    backgroundColor: "{colors.selected}"
  board-row-lit:
    backgroundColor: "rgb(255 212 0 / 0.1)"
  hall-row:
    textColor: "{colors.ivory}"
    typography: "{typography.body}"
    padding: "0 14px"
    height: "34px"
  hall-row-selected:
    backgroundColor: "{colors.selected}"
  parts-row:
    typography: "{typography.parts-row}"
    padding: "0 10px"
    height: "38px"
  pass:
    backgroundColor: "{colors.ivory}"
    textColor: "{colors.paper-ink}"
    rounded: "{rounded.pass}"
  pass-header:
    backgroundColor: "{colors.pass-band}"
    typography: "{typography.pass-title}"
    padding: "14px 16px"
  change-badge:
    backgroundColor: "{colors.alert}"
    textColor: "{colors.alert-ink}"
    rounded: "{rounded.badge}"
    padding: "2px 6px"
  status-mark:
    rounded: "{rounded.hair}"
    size: "10px"
  add-sheet:
    backgroundColor: "{colors.sheet}"
    rounded: "{rounded.pass}"
    padding: "16px"
    width: "560px"
  kbd:
    textColor: "{colors.fog}"
    rounded: "{rounded.badge}"
    padding: "1px 4px"
---

# Design System: DasIDM

## Overview

**Creative North Star: "The Departures Board"**

DasIDM is a gate board hung on smoked glass. What is loading is ranked by when it will finish, the totals read in big condensed numerals across the top, and anything that changes stays lit in alert yellow until you have seen it or it fades. The window's ground is Windows 11 Mica supplied by the OS compositor; every panel is a tinted alpha layer over it with a single line of light along its top edge. Nothing is blurred in CSS: the glass is the operating system's, and the page stays light enough to run in software rendering.

The density is an operator's, not a dashboard's. Board rows are 46px of condensed mono, hall rows are 34px of plain Segoe at one size, and the selected item gets an ivory boarding pass on the right with dashed tear lines, a route line (source code to folder) and the file's live parts map as its scannable block. One key (P) turns the left side into a departures board for a single file: one row per live connection in file order. Motion is mechanical and honest: numbers glide for exactly one progress tick, rows open from zero height, a split drops a yellow cut and flash, and all of it switches off under reduced motion.

The category default this system refuses is a flat table with thin blue progress bars where every row looks the same. Here the order means something, change is visible, and state is a mark in a fixed cell.

**Key Characteristics:**
- Smoked-glass panels (55–75% ink alpha) over OS Mica, top-edge highlight, no CSS blur.
- One alert yellow, reserved for change, live write heads, the primary action and keyboard focus.
- Ivory is the selection colour and the paper of the pass; never yellow.
- Three faces with fixed jobs: condensed gothic numerals, condensed mono board data and labels, Segoe UI Variable for interface prose.
- Tabular figures everywhere, so live numbers never jitter.
- State as a 10px mark in a fixed cell, the same shape wherever it appears.
- anime.js motion grammar: one linear tick tween, one exponential entrance, one split flash.

## Colors

A near-black smoked palette lit by one hard sodium yellow, with ivory paper for the thing you are holding and green and red reserved for finished and failed.

### Primary
- **Departure Yellow** (#ffd400): the only accent. Lit rows after a real overtake ("MOVED UP" badge), the "SPLIT FROM #NN" badge and the split flash and cut, live write heads in every map and strip, the primary action ("Add URL", "Download", "Resume", "Open", "Save"), the keyboard focus ring, the offline caption and the toast. Text on it is **Yellow Ink** (#1a1600); hover lifts it to **Bright Yellow** (#ffe14d).
- **Burnt Yellow** (#c9a300): the write head printed on the ivory pass, where full yellow would vanish against paper.

### Neutral
- **Board Ink** (#0b0d10): the darkest tone; the base hue of every glass tint and the wordmark tile.
- **Smoked Glass, board** (rgb(11 13 16 / 0.6)), **hall** (rgb(16 19 23 / 0.64)) and **pass column** (rgb(13 16 19 / 0.56)): the three panel tints over Mica. The hall is the most opaque because it carries the longest reading.
- **Panel Ivory** (#f2f2ee): primary text on glass, the loaded fill of every map and strip (at 78–82% alpha), the selection tint (selected, 8% alpha, with a 28% ivory inset ring) and the paper of the pass.
- **Fog** (#9aa1a9): secondary text, column and figure labels, waiting rows, units; 7:1 on the glass panels.
- **Steel** (#6b7178): the fill of a paused or waiting strip; a quieter ivory.
- **Hairline** (rgb(255 255 255 / 0.08)) and **Strong Hairline** (rgb(255 255 255 / 0.14)): row dividers and panel borders, and column-header rules and control borders respectively. **Hover** (rgb(255 255 255 / 0.045)) is the row and ghost-control hover wash.
- **Sheet** (#14171c): the opaque ground of the Add sheet and select menus. **Map Hole** (#15191e): missing bytes in the glass parts map and the empty part track.
- **Pass paper family**: **Pass Band** (#e4e4dd) behind the pass header, **Paper Ink** (#15181c) for text, the barcode print and the "Open parts" button, **Paper Label** (#565d65) for labels and sub-values, **Tear Line** (#a7aca4) for the dashed rules, **Paper Hole** (#d6d6cf) for missing bytes on paper.

### Status
- **Arrived Green** (#4cc38a): the finished mark, finished status text, the "Resumable" probe tag.
- **Failed Red** (#ff6b6b): the failed mark, the failed count, failed status text, danger buttons and the remove confirmation. On the pass, failure prints as **Failure Slip** (#ffe3e3 ground, #8a1c1c text).
- **Windows Close Red** (#c42b1c): only the close button's hover, matching the OS.

### Named Rules
**The Yellow Means Change Rule.** Departure Yellow marks something that changed, something writing right now, the one thing to press, or where the keyboard is. It never marks selection, category, decoration or a resting state.

**The Ivory Selection Rule.** Selection is an ivory tint with an ivory inset ring. A lit (changed) row and a selected row must never look alike.

**The Lit Until Seen Rule.** A changed row holds its yellow until the pointer passes over it or it is selected; an overtake on the board fades after 8 seconds.

## Typography

**Display Font:** Barlow Condensed 600/700 (self-hosted, with Segoe UI Variable Display)
**Board Font:** Martian Mono variable, always at 75% width (self-hosted, with ui-monospace)
**Body Font:** Segoe UI Variable Text (with Segoe UI, system-ui)

**Character:** A gate board's condensed gothic numerals over a condensed mono that prints like a split-flap row, with the Windows system face carrying everything a person reads as a sentence. All figures are tabular (set on the root), so a column of speeds or times never shifts as it updates.

### Hierarchy
- **Display** (Barlow Condensed 700, 34px, line-height 1, 0.03em, uppercase): panel titles only, "DEPARTURES" and "PARTS". Drops to 28px in a narrow parts view.
- **Numeral XL** (Barlow Condensed 600, 46px, line-height 0.9): the board's header figures, total speed and "all done at", with the unit set beside it in 12px fog. The parts header uses the same face at 34px.
- **Route Code** (Barlow Condensed 700, 24px, uppercase, 0.02em): the pass's source code and destination folder, like airport codes.
- **Row Numeral** (Barlow Condensed 600, 22px): the finish time in a board row; part numbers ("#07") at 20px.
- **Board Row** (Martian Mono 500 at 75% width, 16px, 0.01em): every cell of a board row; 14px in a narrow board. Status text in the row sits at 13px, 0.06em, fog.
- **Pass Title / Value** (Martian Mono at 75% width, 17px, 700 for the file name clamped to two lines, 600 for values; 15px in the three-cell segment).
- **Parts Row** (Martian Mono at 75% width, 14px): one row per live connection.
- **Body** (Segoe UI Variable Text 400, 13px, line-height 1.4): the hall list, explanations, form text; 600 for buttons, tabs and emphasis. Prose explanations cap at 75ch.
- **Caption** (Segoe 12px): status bar, pass sub-values, hints.
- **Label** (Martian Mono 500 at 75% width, 11px, 0.08em, uppercase, fog): column headers, figure labels, form field labels, counts. On the ivory pass it is 10px in Paper Label.

### Named Rules
**The Gothic Numeral Rule.** Times, totals and part numbers are set in Barlow Condensed. Prose and buttons never are.

**The One Size In The List Rule.** Every cell of a hall row is 13px. Weight (600 for loading) and colour (fog for waiting and finished) carry the difference, never size.

**The Label Sits With Its Datum Rule.** The uppercase tracked label names a column or a figure and sits beside or directly above that datum. It is never a kicker above a heading.

## Layout

A fixed three-band window: a 44px glass title bar (wordmark, status-filter tabs, actions, then 46px-wide window controls), the main area, and a 28px status bar. The main area is a grid of a flexible left column and a 360px pass column (300px when the window is under 1100px wide), with a 10px gutter between panels and 10px side padding. The left column stacks the board (auto height, at most six rows; three when the window is 760px tall or less) over the hall (minimum 170px, fills the rest). The parts view covers the left column in place; board and hall stay mounted beneath it.

The default window is 1280×820; the minimum is 920×580. Width adapts through container queries on each panel rather than page breakpoints: under 820px the board tightens its columns and drops the speed trace; under 720px the hall drops the Added column and its mini strips; under 760px the parts view drops "Parts so far", the legend and the long split wording. In the title bar, tab labels give way under 1000px (All, Failed and the active tab keep theirs) and action labels hide under 1100px.

Rows are fixed-height so lists virtualise: board 46px, parts 38px, hall 34px. Column gaps are 16px (10px when narrow); panel padding is 14px, 16px on the horizontal for board and parts; figures in a header sit 26–28px apart; buttons in a row sit 6px apart.

### Named Rules
**The Fixed Cell Rule.** Every row is a grid of fixed columns, so a value always appears in the same place and the eye can scan straight down.

## Elevation & Depth

Depth is tonal, not cast. Each panel is a translucent ink tint over OS Mica with a 1px hairline border and a top-edge highlight (`inset 0 1px 0 rgb(255 255 255 / 0.07)`), as if light catches the upper edge of a glass pane. Only two objects lift off the glass with a real shadow: the ivory pass, which is a physical ticket, and the Add sheet, which floats over a 35% black scrim. Selection and change are drawn as inset rings, never as lift.

### Shadow Vocabulary
- **Glass Edge** (`box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.07)`): the top-edge highlight on board, hall, pass column and parts panels.
- **Ticket Drop** (`box-shadow: 0 10px 30px rgb(0 0 0 / 0.35)`): the ivory pass only.
- **Sheet Drop** (`box-shadow: 0 18px 48px rgb(0 0 0 / 0.5)`): the Add sheet only.
- **Selected Ring** (`box-shadow: inset 0 0 0 1px rgb(242 242 238 / 0.28)`): a selected row.
- **Lit Ring** (`box-shadow: inset 0 0 0 1px rgb(255 212 0 / 0.45)`): a changed row (0.4 in the hall).

### Named Rules
**The OS Glass Rule.** The glass is Windows Mica from the compositor. No CSS `backdrop-filter`, ever: it costs GPU memory, and idle RAM is a binding product constraint.

## Shapes

Mostly rectilinear with small, consistent corners that step with the size of the object: 2px for marks, strips and part tracks; 3px for change badges and key caps; 4px for the parts map and the focus ring; 6px for buttons, fields and tabs; 8px for glass panels; 12px for the pass and the Add sheet. The pass is the one object with a printed-paper silhouette: dashed 1px tear lines between its segments, a darker header band and a barcode-like stripe pattern where bytes are on disk. Status marks are 10px squares whose shape changes with state (solid ivory loading, outlined fog waiting, two fog bars paused, solid green finished, red with an ink slash failed). Icons are one authored family: 16px grid, 1.5px round-capped strokes, no fills.

## Components

### Buttons
Squared-off, dense and keyboard-labelled: the key that triggers an action is shown in a key cap inside the button.
- **Shape:** gently squared corners (6px), 30px tall, 12px horizontal padding, 8px between icon and label, Segoe 600.
- **Primary:** Departure Yellow ground, Yellow Ink text, Bright Yellow on hover. One per view: "Add URL" in the title bar, "Download" in the sheet, "Resume"/"Retry"/"Open" on the pass, "Save" in settings.
- **Default:** a 4% white wash with a Strong Hairline border; 9% on hover, 13% pressed. Disabled at 40% opacity.
- **Ghost:** no border or ground; Hover wash on hover. Used for Pause all, settings, Cancel, Back to list, Remove.
- **Danger:** Failed Red text with a 40% red border.
- **Focus:** a 2px Departure Yellow outline, offset 1px.
- **Key cap:** 10px condensed mono, 3px corners, 1px currentColor border at 55% opacity.

### Filter Tabs (title bar)
- **Style:** 30px, 6px corners, Segoe 600 in fog; each non-"All" tab carries its state's mark and a condensed-mono count (the Failed count in Failed Red).
- **States:** hover ivory on the Hover wash; active ivory on a 9% white wash.

### Panels (board, hall, pass column, settings)
- **Corner Style:** 8px.
- **Background:** the three smoked-glass tints.
- **Shadow Strategy:** Glass Edge only (see Elevation & Depth).
- **Border:** 1px Hairline (Strong Hairline on the parts view).
- **Internal Padding:** 14px, or 14px 16px 10px on board and parts.

### Inputs / Fields
- **Style:** 32px, 6px corners, 35% black ground, Strong Hairline border, fog placeholder. Selects use the board's own chevron and the Sheet ground for options.
- **Focus:** the border turns Departure Yellow.
- **Checkboxes:** native, tinted Departure Yellow.

### Departures Board (signature)
The top panel: a Display title with a fog caption beside it, the 60-second speed trace (ivory 1.5px line over a 7% ivory area), and the Numeral XL figures. Rows are ranked by finish time on a 2-second clock with hysteresis (a row overtakes only when clearly sooner, by 15 seconds or 20%), rerank with a 520ms quartic ease-out move, and freeze while the pointer is over the board. A real overtake lights the row (10% yellow ground, Lit Ring) and swaps the status for a "MOVED UP" yellow badge. Each row carries a 3px live strip along its bottom: ivory where bytes are on disk, holes where they are missing, yellow ticks at each write head.

### Hall List
The quieter full list: search, category select and count, sortable condensed-mono column headers, then virtualised 34px rows of mark, name, size, status (with a 72px mini strip while loading or paused) and added time. Finished and failed rows hold a yellow Lit Ring until seen.

### The Pass (signature)
The selected item as an ivory boarding pass on the glass: header band with the file name and a sequence number, the route line (source code → folder in Route Code), a two-cell and a three-cell segment of label-over-value data separated by dashed tear lines, then the parts map printed as the scannable block (barcode stripes where loaded, Paper Hole where missing, Burnt Yellow heads) with an ink "Open parts" button. A failure prints a Failure Slip at the foot. Actions sit below the pass on the glass.

### Parts View (signature)
A departures board for one file: header figures, a 64px file map (ivory loaded, Map Hole missing, each live part a framed band with its number and a yellow head), and one 38px row per live connection in file order (part numeral, byte range in one unit, an 8px track with an ivory fill ending in a yellow edge, done %, speed). When a fast line takes half of a slow part, the map drops a yellow cut and flash and the new row opens reading "SPLIT FROM #NN" in a yellow badge; finished rows hold one beat at 100%, then close up.

### Status Mark
A 10px square with 2px corners in a fixed cell, labelled for assistive tech; the same mark appears in hall rows and filter tabs.

### Add Sheet
A 560px opaque sheet (12px corners, Sheet ground, Strong Hairline border, Sheet Drop) dropped from under the title bar's right edge over a scrim, focus-trapped. It probes the link as you type and shows size and resumability before you commit.

### Motion
One grammar, implemented in anime.js for the live parts motion:
- **Tick glide:** live widths and positions tween linearly for 260ms, one progress tick, so bars glide rather than jump.
- **Entrance:** opacity and a short rise, 560ms exponential ease-out; rows open from zero height in 520ms.
- **Exit:** a row fades in the first half, then collapses to zero height (340ms total), never a cut-through slice.
- **Split:** the flash burns off from 95% over 1600ms; the cut drops over 900ms.
- **Chrome:** hovers and state washes use 120–160ms on `cubic-bezier(0.16, 1, 0.3, 1)`; the window fades in over 180ms; the sheet drops 6px in 220ms.
- **Reduced motion:** every tween, transition and keyframe goes to zero; values snap.

## Do's and Don'ts

### Do:
- **Do** keep Departure Yellow (#ffd400) for change, live write heads, the one primary action and keyboard focus.
- **Do** show selection as the ivory tint with the 28% ivory inset ring.
- **Do** set times, totals and part numbers in Barlow Condensed, and every changing number with tabular figures.
- **Do** show state as the 10px status mark in a fixed cell, the same shape everywhere.
- **Do** keep every cell of a hall row at 13px and let weight and colour carry emphasis.
- **Do** build panels as smoked-glass tints over OS Mica with a 1px hairline and the top-edge highlight.
- **Do** use the authored 16px, 1.5px-stroke icon set for every icon.
- **Do** keep rows fixed-height (46 / 38 / 34px) so lists stay virtualised.
- **Do** run live motion through the one anime.js grammar and switch it off under prefers-reduced-motion.
- **Do** use real media art as the only imagery.

### Don't:
- **Don't** use CSS `backdrop-filter`; the glass comes from Windows Mica.
- **Don't** use yellow for selection, categories, decoration or a resting state.
- **Don't** build a flat table with thin blue progress bars where every row looks the same.
- **Don't** rerank the board on a speed wobble; only a clear overtake moves a row, and never under the pointer.
- **Don't** mix type sizes inside a hall row.
- **Don't** put an uppercase tracked label above a heading as a kicker; the label names a column or a figure.
- **Don't** use text glyphs or emoji in place of icons.
- **Don't** use hard offset shadows; only the pass and the Add sheet cast soft shadows.
