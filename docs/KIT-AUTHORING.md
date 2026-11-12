# Kit authoring

A kit decides which tools wugs offers and how the workspace starts. Tongues, the fieldwork kit, is one kit. This guide shows you how to make your own, in the Kit Builder or by hand, and how to share it.

A kit changes only the workspace. Your projects and their data stay the same whichever kit you use.

## What a kit is

A kit is one text file, a `kit.toml` manifest in [TOML](https://toml.io) format. It contains no code. The file names:

- who the kit is: an id, a name, a subtitle, a version, and a description
- which tools, or panels, the kit turns on
- where each panel starts in the workspace, and which tabs start in front
- the kit's wug, if it has one: the mascot in the top bar and on empty panels

The built-in Tongues kit is [`kits/tongues/kit.toml`](../kits/tongues/kit.toml). Read it as a second example.

A kit file may not contain a `[scripts]` section. wugs refuses to install a kit with scripts, so installing a kit file is safe.

## Build a kit in the Kit Builder

The Kit Builder is the **Kits** panel. To open it, open the **Workspace** menu in the top bar and choose **Kits** under **Show a closed panel**.

### Start from a copy

You cannot change the built-in Tongues kit. Start from a copy of it:

1. In the **Kits** panel, click **Duplicate** beside Tongues.
2. Type a name for the new kit, or keep "My Tongues".
3. Click **Make copy**.

wugs makes an id from the name. The copy keeps every part of the original file, comments included.

### Edit the kit

Select your kit in the list to open its editor.

- Edit **Name**, **Subtitle**, **Version**, and **About**. Each field saves when you press **Tab** or **Enter**.
- Under **Tools**, clear a tool's check box to take it out of the kit. wugs also removes that panel from the starting layout and closes it. The first panel that was placed beside it takes over its place.
- Under **Wug**, dress the kit's wug. The preview shows each change as you make it.
  - **Body** sets the wug's colour.
  - **Head**, **Face**, **Holding**, **Suit**, and **Badge** each hold one accessory, or **None**. Choosing an accessory replaces the one in that place.
  - An accessory with a colour of its own, such as the explorer hat or the crown, gets a colour box beside it. Glass lenses, the space helmet's visor, and paper keep their colour.
  - A colour saves when you close the colour picker. Other choices save at once. If the kit is in use, the top bar shows the new wug when the change is saved.
- Under **How the workspace starts**, read the layout in words, one line per panel.

Conflicts, Devices, and Kits are always offered. A kit cannot turn them off.

### Set the layout by arranging panels

To make the workspace you have now into the kit's starting layout:

1. Click **Use** beside your kit, so that the workspace shows its panels.
2. Drag the panels where you want them.
3. In the kit's editor, click **Use the current arrangement**.

Floating and popped-out panel groups are not captured.

### Edit the file directly

1. Click **View code**. The editor shows the kit file.
2. Edit the text.
3. Click **Save code**.

wugs checks the text before it saves anything. If the text has problems, wugs lists every problem with its line and column, and writes nothing.

While the code has unsaved changes, the form above it is locked. Save the code, or click **Discard code changes**.

A code save keeps your text exactly as you typed it. A form edit that changes the tool list or the layout rewrites that list in the file, and comments inside the list are lost.

### Use the kit

Click **Use** beside a kit to switch the workspace to it. The active kit sets the name and the wug in the top bar, the panels listed under **Show a closed panel** in the **Workspace** menu, and where **Reset to how … starts** in that menu puts the panels.

wugs never rebuilds the workspace while you work in it. After you change how the kit in use starts, choose **Workspace › Reset to how … starts** in the top bar to see the new start.

## Write a kit by hand

### Know the format

A kit file has three tables, and an optional fourth for its wug.

#### `[kit]`: the kit's identity

| Key | Required | Rules |
| --- | --- | --- |
| `id` | yes | Lowercase letters, digits, `.`, and `-`. Starts with a letter or digit. No `..`, does not end with `.`, at most 64 characters. |
| `name` | yes | Not empty, at most 40 characters. |
| `subtitle` | no | At most 40 characters. The top bar shows it after the name. |
| `version` | no | At most 40 characters. Any text. |
| `description` | no | At most 500 characters. The Kit Builder shows it as **About**. |

The id names the kit's files on disk, so wugs also refuses these ids:

- the names Windows keeps for devices: `con`, `prn`, `aux`, `nul`, `com0` to `com9`, and `lpt0` to `lpt9`, also before a dot, as in `con.kit`
- `org.wugs.tongues`, the built-in kit's id
- `tongues`, the name of the built-in kit's old layout file

A reverse-domain id such as `org.example.lexicon-desk` avoids clashes with other people's kits.

#### `[tools]`: the panels the kit turns on

`enabled` is a list of panel ids. Each id is lowercase letters, digits, and `-`. The list holds at most 64 ids, and no id twice.

These are the panel ids this version of wugs has:

| Id | Panel |
| --- | --- |
| `timeline` | Timeline |
| `media` | Media |
| `texts` | Texts |
| `transcribe` | Transcribe |
| `gloss` | Gloss |
| `lexicon` | Lexicon |
| `elicit` | Elicit |
| `search` | Search |
| `assist` | Speech assist |
| `people` | People |
| `conflicts` | Conflicts, always offered |
| `devices` | Devices, always offered |
| `kits` | Kits, always offered |

An id this version does not have is kept, so a kit written for a newer version still loads. The Kit Builder notes such ids under the kit.

A tool you turn on but do not place in the layout starts closed. Open it from the **Workspace** menu.

#### `[layout]`: where the panels start

`panels` is a list of placements, at most 64. Each placement is an inline table with these keys only:

| Key | Meaning |
| --- | --- |
| `panel` | The panel id. Each panel is placed at most once. |
| `place` | `left`, `right`, `above`, `below`, or `tab`. |
| `of` | The panel id that this panel goes beside, or joins as a tab. |
| `width` | Starting width in pixels, from 50 to 4000. Used with `left` and `right`. |
| `height` | Starting height in pixels, from 50 to 4000. Used with `above` and `below`. |

The order of the list matters:

- The first placement fills the workspace. It needs only `panel`, with no `place` and no `of` (a starting size is allowed).
- Every later placement needs both `place` and `of`, and `of` must name a panel placed earlier in the list.

A size is only a starting size. The workspace shares the rest of the space among the panels.

A placement of a tool panel that is not in `[tools] enabled` stays closed. The Kit Builder notes it. Conflicts, Devices, and Kits can be placed without being listed in `enabled`.

`active` is a list of panel ids to bring to the front, in order, once the panels are laid out. Each id must be placed in `panels`. Name one tab of each tab group that should not start on its first tab.

You can also write the placements as `[[layout.panels]]` tables. wugs reads both forms the same way.

#### `[wug]`: the kit's mascot

The table is optional. A kit without it shows the blue wugs wug. When the table is there, `body` is required.

| Key | Required | Rules |
| --- | --- | --- |
| `body` | yes | The body colour: `#` and six hex digits, such as `"#f08060"`. |
| `accessories` | no | A list of accessory ids, at most 64, no id twice. Each id is lowercase letters, digits, and `-`. |
| `colours` | no | A table from a drawing part to a colour, at most 64 parts, such as `{ hat-shape = "#7a4a10" }`. Each colour is `#` and six hex digits. |

These are the accessories this version of wugs has. Each place holds one accessory. If a kit lists two for one place, the later one is drawn.

| Id | Accessory | Place | Parts that take a colour |
| --- | --- | --- | --- |
| `hat` | Explorer hat | head | `hat-shape` |
| `crown` | Crown | head | `crown-shape` |
| `antennae` | Antennae | head | `antenna-left-shape`, `antenna-right-shape` |
| `monocle` | Monocle | face | none |
| `glasses` | Glasses | face | none |
| `recorder` | Recorder | holding | `recorder-shape` |
| `scroll` | Notes | holding | none |
| `tablet` | Glyph tablet | holding | none |
| `spacesuit` | Space helmet | suit | `spacesuit-base-shape` |
| `number-one` | Number one | badge | none |

An accessory id or a part that this version does not have is kept, so a kit made with a newer version still loads. wugs leaves that accessory off the drawing. Lines are always drawn in black, so a colour for a part ending in `-line` has no effect. When you change a slot in the Kit Builder, it keeps the colours of the parts still drawn and drops the rest, including colours you typed for accessories no longer worn. A colour change in the Kit Builder writes `colours` as one inline table, so comments inside a hand-written `[wug.colours]` table are lost; comments elsewhere in the file are kept.

The Tongues kit's wug:

```toml
[wug]
body = "#f08060"
accessories = ["hat", "recorder"]
```

#### Tables wugs keeps but does not apply yet

The Tongues kit also has `[templates]` (the default tier schema, gloss conventions, concept lists, and semantic domains) and `[validators]` (a warn or hold level for each check). wugs keeps these tables in the file, and the Kit Builder never removes them. Nothing in wugs applies them yet, so changing them has no effect today.

Other keys in `[kit]`, `[tools]`, `[layout]`, and `[wug]`, and other tables except `[scripts]`, are kept and ignored. A placement is the exception: it may hold only the five keys above.

### Check the size limit

A kit file is at most 256 KiB, and it must be UTF-8 text.

### Look at a worked example

This kit is for a lexicographer who works with a speaker and does not segment recordings. It opens the Lexicon with Elicit beside it, and Search and People as tabs.

```toml
# A desk for building a lexicon with a speaker.

[kit]
id = "org.example.lexicon-desk"
name = "Lexicon desk"
subtitle = "Elicitation"
version = "1.0.0"
description = "Word lists, semantic domains and the lexicon, with no timeline."

[tools]
enabled = ["lexicon", "elicit", "search", "people", "texts"]

[layout]
panels = [
  { panel = "lexicon" },
  { panel = "elicit", place = "left", of = "lexicon", width = 420 },
  { panel = "search", place = "tab", of = "elicit" },
  { panel = "people", place = "tab", of = "elicit" },
  { panel = "conflicts", place = "right", of = "lexicon", width = 300 },
]
active = ["elicit"]

[wug]
body = "#7a6cd8"
accessories = ["crown", "glasses"]
colours = { crown-shape = "#ffd135" }
```

In this kit:

- The Lexicon fills the workspace first.
- Elicit goes to its left, 420 pixels wide. Search and People join Elicit as tabs.
- Conflicts goes to the right of the Lexicon. It is not in `enabled`, because it is always offered.
- `texts` is turned on but not placed, so it starts closed and is listed in the **Workspace** menu.
- `active` brings Elicit to the front of its tab group.
- The wug is purple, with a gold crown and glasses.

## Install and share a kit

To share a kit, send its `.toml` file. There is no kit library yet.

To install a kit file someone sent you:

1. In the **Kits** panel, click **Install a kit file…**.
2. Choose the `.toml` file.
3. Click **Use** beside the new kit when you are ready.

wugs checks the file before it copies it. It never replaces a kit that is already installed. If a kit with the same id exists, delete that kit first, or change `id` in the file.

wugs stores your kits in the `kits` folder of its app data folder, one file per kit, named `<id>.toml`:

| OS | Folder |
| --- | --- |
| Windows | `%APPDATA%\dev.wugs.desktop\kits` |
| macOS | `~/Library/Application Support/dev.wugs.desktop/kits` |
| Linux | `~/.local/share/dev.wugs.desktop/kits` |

A file in that folder must be named after its kit's id. The Kits panel lists files it cannot use under **Files in the kits folder that could not be used**, with the reason, and never deletes them.

To delete a kit, click **Delete** beside it, then click **Delete? Click again**. If it was the kit in use, Tongues becomes the kit in use again.

## Know the limits

- A kit's id cannot change after the kit is made. To use another id, duplicate the kit.
- The kit in use is chosen per computer. It is not stored in the project and does not sync.
- `[templates]` and `[validators]` are kept but not applied.
- wugs saves each kit's rearranged workspace as soon as you use the kit. When a wugs update changes how Tongues starts, choose **Workspace › Reset to how Tongues starts** to see the new start.
- Kits cannot contain scripts.
