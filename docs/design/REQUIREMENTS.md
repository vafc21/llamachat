# LlamaChat UI — design requirements

Source of truth for the UI redesign. Derived from Vlad's direct feedback
(2026-07-28, `local-conductor` session `7327ec4a`) plus the three reference
screenshots in `mockups/ref{1,2,3}-*.png`.

Written down because the original brief only existed inside a chat session.

---

## 1. What Vlad asked for, verbatim

> So I feel like you're trying to go in between a coder or like a programmer and a
> regular person, which I understand is hard. Can you literally research and copy
> pretty much how Claude desktop app works? I like the chat thing and I like the
> co-work change. So you can change the co-work and then it uses stuff, but it isn't
> that coder. And then they can switch to the code part and it has that whole code
> with the context window. You should add in the CPU RAM and the GPU percentage
> there. But for the chat, it should be minimalistic exactly how Claude code does.
> I'm not Claude code, just the regular Claude chat and take some don't change.
> Don't take the colors, though. I don't want you to. I don't want to be too direct.
> Definitely look at what open AI is doing to you know, most of those. Kimmy K3
> would be a good one to look at to just do some more research.

> I'ma be honest, this looks awful except the good evening Vlad. That's good. But
> there should be just one way to switch between code and all and co-work. Also,
> code, there shouldn't be any emojis. You should be able to just put glyph icons
> and code shouldn't show any code. It should just get a library and should be
> exactly how llama cli works. Just have a GUI for it. Maybe a different prompt when
> you're prompting to show the AI nodes that it's in the code section that has this
> tools or something like that.

> Yes, take some notes on what Cowork and Clyde have like tools from [...] so you
> can copy those tools [...] and give those to these local agents.

---

## 2. Hard requirements

| # | Requirement | Source |
|---|---|---|
| R1 | **Exactly one** mode switcher. Not two. | "there should be just one way to switch" |
| R2 | Three modes: **Chat**, **Cowork**, **Code**. | "chat thing", "co-work change", "the code part" |
| R3 | Chat is **minimalistic** — regular Claude chat, *not* Claude Code. | "for the chat, it should be minimalistic" |
| R4 | Cowork uses tools but is **not developer-facing**. | "it uses stuff, but it isn't that coder" |
| R5 | Code mode shows the **context window**. | "the code part [...] has that whole code with the context window" |
| R6 | Code mode shows **CPU, RAM and GPU percentage**. | "add in the CPU RAM and the GPU percentage there" |
| R7 | **Zero emoji** in Code mode. Line/glyph icons only. | "there shouldn't be any emojis [...] just put glyph icons" |
| R8 | Code mode does **not** render source code. It is a **GUI for `llama-cli`**. | "code shouldn't show any code [...] exactly how llama cli works" |
| R9 | Code mode surfaces the **model library**. | "It should just get a library" |
| R10 | A **distinct prompt/affordance** in Code mode signalling the model is tool-equipped. | "a different prompt [...] to show the AI [k]nows that it's in the code section that has this tools" |
| R11 | Keep the **time-of-day greeting** ("Good evening, Vlad"). | "this looks awful except the good evening Vlad. That's good." |
| R12 | **Do not copy Claude's colors.** Use LlamaChat's own palette. | "Don't take the colors [...] I don't want to be too direct" |
| R13 | Deliver **three distinct versions**. | "Can you give me like three different versions?" |

## 3. Standing design law (from prior feedback)

- Clean and professional. Must not read as "AI-generated".
- **Banned:** neon, glow, gradient fills, glassmorphism, decorative blur.
- One muted accent, used sparingly. Whitespace does the work. Inter-class sans.
- Icons are single-weight line glyphs. No emoji anywhere in chrome.

## 4. Palette

Use the existing brand tokens from `ui/src/index.css` — these are LlamaChat's,
not Claude's, which satisfies R12:

```
--color-bg          #0d0d12
--color-surface     #111118
--color-sidebar     #0b0b10
--color-text        #e1e1e6
--color-text-secondary #8b8b9e
--color-text-muted  #5c5c6e
--color-accent      #4d7cff   ← sparingly. no glow.
--color-success     #3fb950
--color-warning     #d29922
--color-error       #f85149
```

Brand mark: white Ollama-derived llama on `#0d0d12` squircle
(`llamachat-brand/svg/logo-mono.svg`).

## 5. What the reference screenshots actually show

Read from `ref1-chat.png`, `ref2-cowork.png`, `ref3-code.png`.

**Shell (all three):** ~36px chromeless title bar — hamburger, sidebar toggle,
search, back/forward at left; window controls at right. Sidebar ~280px, one tone
darker than the main pane, no divider border. Sidebar footer is avatar + name +
plan. No emoji anywhere; thin line icons throughout.

**Chat (ref1):** Main pane is empty apart from a vertically-centred greeting —
mark glyph + one **serif** line — with the composer directly beneath it. Composer
is a rounded rect (~16px radius) on a lighter surface, ~670px wide. Inside it:
placeholder text, then a control row — `+`, a **Chat | Cowork segmented control**,
and at the right the model name, effort level, mic, waveform. Accent colour
appears **once**, on the mark.

**Cowork (ref2):** Identical shell and composer, Cowork segment active. Composer
grows a second row: scope picker (`Project or folder`), a permission control
(`Skip`), and a usage note. Below the composer sits an **Active** list — task
rows with a status dot, bold title, relative timestamp, thin dividers, and a
`Show more` at the end.

**Code (ref3):** Sidebar nav collapses to a shorter set. Greeting switches to
**sans-serif** (`What's up next, Vlad?`). Main pane holds a stats card — segmented
`Overview | Models`, a range switch (`All / 30d / 7d`), a 2×4 grid of stat tiles
(Sessions, Messages, Total tokens, Active days, Current streak, Longest streak,
Peak hour, Favorite model) and a contribution heatmap. **The composer is docked to
the bottom**, not centred. Above the input is a context chip row (host, project,
branch, worktree). Below it: a permission-mode chip in amber — the only warning
colour on screen — plus model, effort, and **a small ring at the far right, which
is the context-window meter**.

> Note: real Claude has **two** switchers — `Home | Code` at the sidebar top *and*
> `Chat | Cowork` in the composer. R1 says LlamaChat gets **one**. This is the
> single deliberate divergence from the reference.

## 6. Mapping to existing LlamaChat features

Do not invent product surface. Wire the modes to what exists:

- Views today (`ui/src/App.tsx`): `chat | library | settings | skills | memory`.
- Auto-download tiers (`models.ts`): **Quick** / **Smart** / **Best**.
- Per-model hardware rating (`types.ts`): `wont_run | slow | okay | great | blazing`.
- Download states: `pending | downloading | ready | error`.
- Agent mode already has permission modes (`ask` / bypass) — this is the chip
  that belongs in the Code composer.

**Emoji currently in the source that R7 removes:** `🔧` and `✓` in
`MessageBubble.tsx`, `🤖` in `InputBar.tsx`, `🧠`/`🔧`/`⚠` in `App.tsx`,
`✅`/`❌` in `AgentSetup.tsx`, `✓` in `Settings.tsx`, `WelcomeSteps.tsx`,
`MemorySeed.tsx`, `⚠` in `ModelLibrary.tsx`, and the tier icons `⚡ ✦ ★` in
`models.ts`.

## 7. Deliverable

Three standalone HTML mockups, each a different structural answer to R1 — not
three colour skins of one layout. Each must render all three modes.

---

# Round 2 — persona split (2026-08-02)

Vlad picked **Version B (segmented)**. New requirements on top of R1–R13.

## Hard requirements

| # | Requirement |
|---|---|
| R14 | Ask once at **startup**: developer or not. Default is the simple side. |
| R15 | The same choice is changeable **any time in Settings**. |
| R16 | **Developer:** can change the model, sees the runtime insights (the bottom status bar from Version C), and gets the **Code** workspace. |
| R17 | **Simple:** **no model is ever shown.** No context, no token counts, no tok/s, no CPU/GPU/VRAM, no model library in the sidebar, no Code tab. |
| R18 | Simple mode **auto-routes**: a router picks which local model handles the message and how much effort it spends. Modelled on ChatGPT. |
| R19 | Everything is **transparent to the developer** — tokens/sec, usage, context, and *which model the router chose and why*. |
| R20 | Use the **real brand mark**, not a stand-in. |

## The router (research)

OpenAI describes GPT‑5 as a **unified system**: a fast model for most questions, a deeper
reasoning model for hard ones, and a **real-time router** choosing per message on
**conversation type, complexity, tool needs, and explicit intent** ("think hard about this"
forces the reasoning path). The router is trained on real usage signals — model-switching
behaviour, preference rates, correctness — not hand-written rules. Users *can* override
(Auto / Instant / Thinking / Pro) but Auto is default and most never touch it.

**LlamaChat's local equivalent:** the existing benchmark tiers are the routing targets, and the
per-machine ratings bound what the router may pick.

| Signal | Routes to |
|---|---|
| Short, factual, single-step | `Quick` tier, low effort |
| Everyday work, some reasoning | `Smart` tier, medium effort |
| Multi-step, technical, tool-using | `Best` tier, high effort |
| Explicit "think hard" | forced high effort |
| Model rated `wont_run` / `slow` | never selected |

Simple mode shows only the outcome in plain words — `Thought for 6 seconds`. Developer mode
prints the decision: `Router → Qwen3 30B A3B · high effort · reason: technical, multi-step`.

Context in simple mode is managed silently (trim + summarize); the user is never asked to
care about it.

## Logo

All earlier mockups used an invented llama path. The correct mark is the canonical
5,351-character `fill-rule="evenodd"` path in `llamachat-brand/svg/logo-mono.svg` — the
traced Ollama llama with the eye as a true cut-out, filled with `currentColor`. It is the
same path shipped in `Logo.tsx` and every SVG in the brand kit. Never redraw it.

## Deliverables

- `mockups/v6.html` — the app. `?persona=simple|dev`, `?mode=`, `?view=empty|convo`,
  `?screen=setup`, `?chrome=0` to hide the preview bar.
- `mockups/v6-compare.html` — side-by-side diff, embeds the above live.

---

# CLI — decided (2026-08-02)

Vlad asked me to choose. Locked:

| Decision | Choice |
|---|---|
| Shell | **B · Flat full-screen** — borderless, dim letterspaced section labels, column alignment |
| Treatment | **D** — background bands, eighth-block gauges, sparklines, filled badges |
| Rating column | **Option 4 — word + speed** (`blazing    94 tok/s`). No `▰▰▰▰▱`. |
| Row grammar | One shape everywhere: `marker · name · meter · detail · state` |
| Character / mascot | **None.** Dropped — the art can't be made to render consistently across fonts. |
| Palette | Tints/shades of `#4d7cff` only, plus neutrals. Amber reserved for permission state. |

**Known trade:** B keeps `ratatui::init()`'s alternate screen, so the session still
vanishes from scrollback on exit. Claude Code and Codex both avoid this. A `--no-alt`
inline mode is the follow-up, not a blocker.

**Craft rules taken from Codex CLI, to apply in the rewrite:**
- Segment-cached incremental markdown — never re-render completed turns.
- Bounded tool output: head + `… N more lines` + tail, full text kept in the transcript.
- OSC 8 hyperlinks with capability probing and plain-text fallback.
- Honour `NO_COLOR` and non-tty stdout (already enforced in `design.rs`).

**Preview:** `llamachat design [--variant a|b|c|d|meters|rows]`, rendered to
`mockups/cli.html`. Delete `design.rs` once `tui/render.rs` is rewritten.
