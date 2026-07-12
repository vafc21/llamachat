# Privacy Policy

_Last updated: 2026-07-12_

**LlamaChat** is a local-first desktop application. It is designed so that your
data stays on your device. LlamaChat does **not** collect, transmit, sell, or
share any personal information, and it contains **no analytics, tracking, or
telemetry**.

## What LlamaChat stores (locally, on your device)

LlamaChat stores the following only on your own computer, in a local database
and settings files:

- your hardware profile (CPU, GPU, RAM, storage, OS) detected on your machine;
- benchmark results measured on your machine;
- your app settings, and any chat history or notes you create.

None of this is sent anywhere. You can delete it at any time by removing the
app's data directory.

## When data leaves your device (only by your action)

LlamaChat only makes network connections as a direct result of something you
choose to do:

- **Downloading a model** — when you ask LlamaChat to download an open model, it
  uses your local [Ollama](https://ollama.com) installation, which fetches the
  model from its model registry. That request is governed by Ollama's own
  privacy policy, not LlamaChat's. LlamaChat sends no personal data as part of
  it.
- **Cloud model comparison (optional)** — if a future version lets you compare
  against a hosted/cloud model, it will contact that provider's API **only if
  you explicitly enable it**, and only send the prompt you choose to compare.
  This is off by default.

LlamaChat does not require an account, does not phone home, and shows no ads.

## Agent mode

If you use Agent mode, LlamaChat can read your screen (via accessibility APIs or
screenshots) and control the mouse/keyboard to operate other apps on your
computer. This information is used **locally** — by a model running on your own
machine — to perform the task you requested. It is not transmitted by LlamaChat.

## Children's privacy

Because LlamaChat collects no personal information, it collects none from
children either.

## Changes to this policy

If this policy changes, the updated version will be published in this repository
with a new "Last updated" date.

## Contact

Questions about privacy? Email **vladimirpetrosyanjr@gmail.com** or open an issue
at <https://github.com/vafc21/llamachat/issues>.
